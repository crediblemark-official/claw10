use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc;

use claw10_budget::BudgetService;
use claw10_domain::{Agent, CostCategory, PolicySubject};
use claw10_model_router::router::ModelRouter;
use claw10_policy::PolicyService;
use claw10_model_router::types::{ChatRequest, FinishReason, MessageRole, ModelMessage};
use claw10_tool::context::ToolContext;
use claw10_tool::registry::ToolRegistry;
use claw10_tool::result::ToolOutput;
use claw10_store::StoreExt;


use crate::context::ContextBuilder;
use crate::error::AgentError;
use crate::events::AgentEvent;
use crate::self_correct::{RetryTracker, VerificationEngine, VerificationResult};
use crate::session::{AgentSession, SessionState};

pub type EventSender = mpsc::UnboundedSender<AgentEvent>;

pub struct AgentExecutor {
    model_router: Arc<ModelRouter>,
    tool_registry: Arc<ToolRegistry>,
    budget_service: Arc<BudgetService>,
    kv_store: Arc<dyn claw10_store::Store>,
    verifier: VerificationEngine,
}

impl AgentExecutor {
    #[must_use]
    pub fn new(
        model_router: Arc<ModelRouter>,
        tool_registry: Arc<ToolRegistry>,
        budget_service: Arc<BudgetService>,
        kv_store: Arc<dyn claw10_store::Store>,
    ) -> Self {
        Self {
            model_router: Arc::clone(&model_router),
            tool_registry: Arc::clone(&tool_registry),
            budget_service,
            kv_store,
            verifier: VerificationEngine::with_model_router(Arc::clone(&tool_registry), Arc::clone(&model_router)),
        }
    }

    pub async fn execute(
        &self,
        agent: &mut Agent,
        objective: &str,
        context: HashMap<String, String>,
        tool_context: ToolContext,
        max_turns: u32,
    ) -> Result<(AgentSession, Vec<AgentEvent>), AgentError> {
        let mut session = AgentSession::with_context_limit(
            agent.id.clone(),
            Some(agent.genome.model_policy.max_context_tokens),
        );
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let mut events = Vec::new();

        if event_tx
            .send(AgentEvent::SessionStarted {
                agent_id: agent.id.0.to_string(),
                session_id: session.id.0.to_string(),
                objective: objective.to_string(),
            })
            .is_err()
        {
            tracing::warn!("SessionStarted event send failed (receiver may be dropped)");
        }

        let messages = ContextBuilder::build_initial_messages(agent, objective, &context, &self.tool_registry);
        for msg in messages {
            session.add_message(msg);
        }

        let tool_defs = ContextBuilder::tool_definitions(&self.tool_registry);

        // ── Policy pre-flight check ─────────────────────────────────
        let policy_subject = PolicySubject::Agent(agent.id.0.to_string());
        let policy_result = PolicyService::evaluate(
            &agent.policy_bundle,
            &policy_subject,
            "agent:execute",
            &format!("agent:{}", agent.id.0),
            None,
        );
        match policy_result {
            Ok(r) if !r.allowed => {
                if event_tx.send(AgentEvent::Error {
                    message: r.reason.clone(),
                }).is_err() {
                    tracing::warn!("PolicyDenied event send failed (receiver may be dropped)");
                }
                return Err(AgentError::PolicyDenied(r.reason));
            }
            Err(e) => {
                if event_tx.send(AgentEvent::Error {
                    message: e.to_string(),
                }).is_err() {
                    tracing::warn!("PolicyDenied event send failed (receiver may be dropped)");
                }
                return Err(AgentError::PolicyDenied(e.to_string()));
            }
            _ => {}
        }

        for turn in 0..max_turns {
            if session.state != SessionState::Active {
                break;
            }

            if agent.budget.is_soft_limit_reached() {
                if event_tx
                    .send(AgentEvent::BudgetWarning {
                        remaining: agent.budget.remaining(),
                    })
                    .is_err()
                {
                    tracing::warn!("BudgetWarning event send failed (receiver may be dropped)");
                }
            }

            // ── Budget circuit breaker: hard limit check BEFORE each turn ──
            if agent.budget.is_exhausted() {
                let spent = agent.budget.spent_usd;
                let hard_limit = agent.budget.hard_limit_usd.unwrap_or(agent.budget.allocated_usd);
                if event_tx
                    .send(AgentEvent::BudgetExceeded {
                        spent,
                        hard_limit,
                    })
                    .is_err()
                {
                    tracing::warn!("BudgetExceeded event send failed (receiver may be dropped)");
                }
                return Err(AgentError::BudgetExhausted);
            }

            let profile_id = &agent.genome.model_policy.preferred_profile;
            let fallbacks = &agent.genome.model_policy.fallback_profiles;

            let chat_request = ChatRequest {
                model: profile_id.clone(),
                messages: session.messages.clone(),
                max_tokens: Some(4096),
                temperature: Some(0.7),
                tools: Some(tool_defs.clone()),
                stop: None,
            };

            if event_tx
                .send(AgentEvent::ModelCall {
                    turn,
                    tokens: 0,
                    cost: 0.0,
                })
                .is_err()
            {
                tracing::warn!("ModelCall event send failed (receiver may be dropped)");
            }

            tracing::info!("[Agent Loop] Memulai Turn {}/{} untuk Agent {}", turn + 1, max_turns, agent.id.0);

            let response = self
                .model_router
                .route_with_fallback(profile_id, fallbacks, chat_request)
                .await?;

            session.record_turn(response.usage.total_tokens, response.usage.cost_usd);

            // Enforce budget per turn (post-facto check via reserve)
            if let Err(e) = self.budget_service.reserve(&mut agent.budget, response.usage.cost_usd) {
                if event_tx
                    .send(AgentEvent::BudgetWarning {
                        remaining: agent.budget.remaining(),
                    })
                    .is_err()
                {
                    tracing::warn!("BudgetWarning (reserve) event send failed (receiver may be dropped)");
                }
                return Err(AgentError::from(e));
            }

            let msg = response.message.clone();
            let has_tool_calls = response.finish_reason == FinishReason::ToolCalls
                || response.message.tool_calls.as_ref().map_or(false, |tc| !tc.is_empty());

            tracing::info!("[Agent Loop] Respon LLM diterima untuk Turn {}. Content: '{}', has_tool_calls: {}", turn + 1, response.message.content.trim(), has_tool_calls);

            if !response.message.content.is_empty() {
                if event_tx
                    .send(AgentEvent::Thought {
                        turn,
                        content: response.message.content.clone(),
                    })
                    .is_err()
                {
                    tracing::warn!("Thought event send failed (receiver may be dropped)");
                }
            }

            session.add_message(msg);

            if has_tool_calls {
                if let Some(tool_calls) = &response.message.tool_calls {
                    for tc in tool_calls {
                        let tool_name = &tc.name;
                        let args = &tc.arguments;

                        // ── Policy per-tool check ──────────────────────────
                        let tool_policy_result = PolicyService::evaluate(
                            &agent.policy_bundle,
                            &policy_subject,
                            "tool:invoke",
                            tool_name,
                            None,
                        );
                        match tool_policy_result {
                            Ok(r) if !r.allowed => {
                                if event_tx.send(AgentEvent::Error {
                                    message: r.reason.clone(),
                                }).is_err() {
                                    tracing::warn!("Tool policy denied event send failed (receiver may be dropped)");
                                }
                                return Err(AgentError::PolicyDenied(r.reason));
                            }
                            Err(e) => {
                                if event_tx.send(AgentEvent::Error {
                                    message: e.to_string(),
                                }).is_err() {
                                    tracing::warn!("Tool policy error event send failed (receiver may be dropped)");
                                }
                                return Err(AgentError::PolicyDenied(e.to_string()));
                            }
                            _ => {}
                        }

                        let tool_result = match self.tool_registry.get(tool_name) {
                            Ok(tool) => {
                                if event_tx
                                    .send(AgentEvent::ToolCall {
                                        tool: tool_name.clone(),
                                        args: args.clone(),
                                        result: serde_json::Value::Null,
                                    })
                                    .is_err()
                                {
                                    tracing::warn!("ToolCall event send failed (receiver may be dropped)");
                                }

                                tracing::info!("[Agent Loop] Agent memanggil tool '{}' dengan argumen: {}", tool_name, args);

                                let approved = self.wait_for_tool_approval(agent, tool_name, args, &tc.id).await?;
                                if approved {
                                    match tool.execute(&tool_context, args.clone()).await {
                                        Ok(output) => {
                                            if event_tx
                                                .send(AgentEvent::ToolCall {
                                                    tool: tool_name.clone(),
                                                    args: args.clone(),
                                                    result: output.data.clone(),
                                                })
                                                .is_err()
                                            {
                                                tracing::warn!("ToolCall result event send failed (receiver may be dropped)");
                                            }
                                            output
                                        }
                                        Err(e) => ToolOutput::fail(e.to_string()),
                                    }
                                } else {
                                    let output = ToolOutput::fail("Tool invocation denied by operator".to_string());
                                    if event_tx
                                        .send(AgentEvent::ToolCall {
                                            tool: tool_name.clone(),
                                            args: args.clone(),
                                            result: output.data.clone(),
                                        })
                                        .is_err()
                                    {
                                        tracing::warn!("ToolCall denied event send failed (receiver may be dropped)");
                                    }
                                    output
                                }
                            }
                            Err(e) => {
                                ToolOutput::fail(format!("tool '{tool_name}' not found: {e}"))
                            }
                        };

                        tracing::info!("[Agent Loop] Hasil eksekusi tool '{}': {}", tool_name, serde_json::to_string(&tool_result.data).unwrap_or_default());

                        // ── Self-correction: verify result ──────────────────
                        let enriched_data = self
                            .verify_tool_result(tool_name, args, &tool_result, &tool_context, &event_tx)
                            .await;

                        session.add_message(ModelMessage {
                            role: MessageRole::Tool,
                            content: serde_json::to_string(&enriched_data)
                                .unwrap_or_else(|_| "{}".into()),
                            content_parts: None,
                            tool_calls: None,
                            tool_call_id: Some(tc.id.clone()),
                            name: Some(tool_name.clone()),
                        });
                    }
                }
                continue;
            }

            // LLM responded without tool calls = final answer
            tracing::info!("[Agent Loop] Agent berhasil menyelesaikan objektif. Final answer: '{}'", response.message.content.trim());

            events.extend(self.drain_events(&mut event_rx).await);
            if event_tx
                .send(AgentEvent::ObjectiveComplete {
                    summary: response.message.content.clone(),
                    evidence: vec![],
                })
                .is_err()
            {
                tracing::warn!("ObjectiveComplete event send failed (receiver may be dropped)");
            }
            session.state = SessionState::Completed;
            events.extend(self.drain_events(&mut event_rx).await);
            return Ok((session, events));
        }

        // Max turns reached without completion
        Err(AgentError::MaxTurnsReached(max_turns))
    }

    async fn drain_events(&self, rx: &mut mpsc::UnboundedReceiver<AgentEvent>) -> Vec<AgentEvent> {
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        events
    }

    /// Versi streaming dari execute — menggunakan route_chat_stream sehingga:
    /// - TextDelta dikirim per-karakter ke TUI (real-time)
    /// - ToolCallDelta di-assemble dari stream, lalu dieksekusi
    /// - AgentEvent::Thought berisi teks lengkap setelah turn selesai
    pub async fn execute_streaming(
        &self,
        agent: &mut Agent,
        objective: &str,
        context: HashMap<String, String>,
        tool_context: ToolContext,
        max_turns: u32,
        event_tx: EventSender,
    ) -> Result<AgentSession, AgentError> {
        use claw10_model_router::types::StreamEvent;
        use std::collections::HashMap as ToolMap;


        let mut session = AgentSession::with_context_limit(
            agent.id.clone(),
            Some(agent.genome.model_policy.max_context_tokens),
        );

        if event_tx
            .send(AgentEvent::SessionStarted {
                agent_id: agent.id.0.to_string(),
                session_id: session.id.0.to_string(),
                objective: objective.to_string(),
            })
            .is_err()
        {
            tracing::warn!("SessionStarted event send failed (receiver may be dropped)");
        }

        let messages = ContextBuilder::build_initial_messages(agent, objective, &context, &self.tool_registry);
        for msg in messages {
            session.add_message(msg);
        }

        let tool_defs = ContextBuilder::tool_definitions(&self.tool_registry);
        let has_tools = !tool_defs.is_empty();

        for turn in 0..max_turns {
            if session.state != SessionState::Active {
                break;
            }

            if agent.budget.is_soft_limit_reached() {
                if event_tx
                    .send(AgentEvent::BudgetWarning {
                        remaining: agent.budget.remaining(),
                    })
                    .is_err()
                {
                    tracing::warn!("BudgetWarning event send failed (receiver may be dropped)");
                }
            }

            let profile_id = &agent.genome.model_policy.preferred_profile;

            let chat_request = ChatRequest {
                model: profile_id.clone(),
                messages: session.messages.clone(),
                max_tokens: Some(4096),
                temperature: Some(0.7),
                tools: if has_tools { Some(tool_defs.clone()) } else { None },
                stop: None,
            };

            // ── Budget circuit breaker: hard limit check BEFORE sending ModelCall event ──
            if agent.budget.is_exhausted() {
                let spent = agent.budget.spent_usd;
                let hard_limit = agent.budget.hard_limit_usd.unwrap_or(agent.budget.allocated_usd);
                if event_tx
                    .send(AgentEvent::BudgetExceeded {
                        spent,
                        hard_limit,
                    })
                    .is_err()
                {
                    tracing::warn!("BudgetExceeded event send failed (receiver may be dropped)");
                }
                return Err(AgentError::BudgetExhausted);
            }

            if event_tx
                .send(AgentEvent::ModelCall {
                    turn,
                    tokens: 0,
                    cost: 0.0,
                })
                .is_err()
            {
                tracing::warn!("ModelCall event send failed (receiver may be dropped)");
            }

            // ── Coba streaming terlebih dulu ─────────────────────────────
            let stream_handle = self.model_router.route_chat_stream(profile_id, chat_request.clone()).await;

            match stream_handle {
                Ok(handle) => {
                    // Kumpulkan stream: text deltas + tool call deltas
                    let mut full_text = String::new();
                    // key: index → (id, name, args_so_far)
                    let mut tool_chunks: ToolMap<usize, (String, String, String)> = ToolMap::new();
                    let mut total_tokens = 0u32;
                    let mut total_cost = 0.0f64;
                    let mut stream_had_error = false;

                    loop {
                        match handle.recv().await {
                            Some(StreamEvent::TextDelta(delta)) => {
                                full_text.push_str(&delta);
                                // Emit ke TUI per-karakter
                                if event_tx
                                    .send(AgentEvent::TextDelta {
                                        turn,
                                        delta: delta.clone(),
                                    })
                                    .is_err()
                                {
                                    tracing::warn!("TextDelta event send failed (receiver may be dropped)");
                                }
                            }
                            Some(StreamEvent::ToolCallDelta { index, id, name, arguments }) => {
                                let entry = tool_chunks.entry(index).or_insert_with(|| {
                                    (String::new(), String::new(), String::new())
                                });
                                if let Some(id_val) = id {
                                    entry.0 = id_val;
                                }
                                if let Some(name_val) = name {
                                    entry.1 = name_val;
                                }
                                entry.2.push_str(&arguments);
                            }
                            Some(StreamEvent::Usage(usage)) => {
                                total_tokens = usage.total_tokens;
                                total_cost = usage.cost_usd;
                            }
                            Some(StreamEvent::Done) | None => break,
                            Some(StreamEvent::Error(e)) => {
                                stream_had_error = true;
                                if event_tx
                                    .send(AgentEvent::Error {
                                        message: format!("Stream error turn {turn}: {e}"),
                                    })
                                    .is_err()
                                {
                                    tracing::warn!("StreamError event send failed (receiver may be dropped)");
                                }
                                break;
                            }
                        }
                    }

                    if stream_had_error {
                        return Err(AgentError::Other(format!("stream error at turn {turn}")));
                    }

                    session.record_turn(total_tokens, total_cost);

                    // Enforce budget per turn
                    if let Err(e) = self.budget_service.reserve(&mut agent.budget, total_cost) {
                        if event_tx
                            .send(AgentEvent::BudgetWarning {
                                remaining: agent.budget.remaining(),
                            })
                            .is_err()
                        {
                            tracing::warn!("BudgetWarning (stream) event send failed (receiver may be dropped)");
                        }
                        return Err(AgentError::from(e));
                    }
                    self.record_cost(
                        &agent.id.0.to_string(),
                        &agent.mission_id.0.to_string(),
                        total_cost,
                        CostCategory::ModelCall,
                    )
                    .await;


                    // Tentukan apakah ada tool calls
                    let has_tool_calls = !tool_chunks.is_empty();

                    if has_tool_calls {
                        // Emit Thought jika ada teks sebelum tool calls
                        if !full_text.is_empty() {
                            if event_tx
                                .send(AgentEvent::Thought {
                                    turn,
                                    content: full_text.clone(),
                                })
                                .is_err()
                            {
                                tracing::warn!("Thought event send failed (receiver may be dropped)");
                            }
                        }

                        // Tambah message assistant dengan tool calls ke session
                        let assembled_tool_calls: Vec<claw10_model_router::types::ToolCall> = {
                            let mut sorted: Vec<(usize, (String, String, String))> =
                                tool_chunks.into_iter().collect();
                            sorted.sort_by_key(|(idx, _)| *idx);
                            sorted
                                .into_iter()
                                .map(|(_, (id, name, args_str))| {
                                    let cleaned_args = {
                                        let mut s = args_str.trim();
                                        if s.starts_with("```json") {
                                            s = s.strip_prefix("```json").unwrap_or(s).trim();
                                        } else if s.starts_with("```") {
                                            s = s.strip_prefix("```").unwrap_or(s).trim();
                                        }
                                        if s.ends_with("```") {
                                            s = s.strip_suffix("```").unwrap_or(s).trim();
                                        }
                                        s
                                    };
                                    let args_val = match serde_json::from_str(cleaned_args) {
                                        Ok(val) => val,
                                        Err(e) => {
                                            tracing::warn!("Gagal parse tool arguments: '{}'. Error: {:?}", args_str, e);
                                            serde_json::Value::Null
                                        }
                                    };
                                    claw10_model_router::types::ToolCall {
                                        id,
                                        name,
                                        arguments: args_val,
                                    }
                                })
                                .collect()
                        };

                        // Simpan assistant message dengan tool calls
                        session.add_message(ModelMessage {
                            role: MessageRole::Assistant,
                            content: full_text.clone(),
                            content_parts: None,
                            tool_calls: Some(assembled_tool_calls.clone()),
                            tool_call_id: None,
                            name: None,
                        });

                        // Eksekusi setiap tool call
                        for tc in &assembled_tool_calls {
                            let tool_name = &tc.name;
                            let tool_id = &tc.id;
                            let args = &tc.arguments;

                            // Emit "memanggil tool" sebelum eksekusi
                            if event_tx
                                .send(AgentEvent::ToolCall {
                                    tool: tool_name.to_string(),
                                    args: args.clone(),
                                    result: serde_json::Value::Null,
                                })
                                .is_err()
                            {
                                tracing::warn!("ToolCall event send failed (receiver may be dropped)");
                            }

                            let tool_result = match self.tool_registry.get(tool_name) {
                                Ok(tool) => {
                                    let approved = self.wait_for_tool_approval(agent, tool_name, args, tool_id).await?;
                                    if approved {
                                        match tool.execute(&tool_context, args.clone()).await {
                                            Ok(output) => {
                                                if event_tx
                                                    .send(AgentEvent::ToolCall {
                                                        tool: tool_name.to_string(),
                                                        args: args.clone(),
                                                        result: output.data.clone(),
                                                    })
                                                    .is_err()
                                                {
                                                    tracing::warn!("ToolCall result event send failed (receiver may be dropped)");
                                                }
                                                output
                                            }
                                            Err(e) => ToolOutput::fail(e.to_string()),
                                        }
                                    } else {
                                        let output = ToolOutput::fail("Tool invocation denied by operator".to_string());
                                        if event_tx
                                            .send(AgentEvent::ToolCall {
                                                tool: tool_name.to_string(),
                                                args: args.clone(),
                                                result: output.data.clone(),
                                            })
                                            .is_err()
                                        {
                                            tracing::warn!("ToolCall denied event send failed (receiver may be dropped)");
                                        }
                                        output
                                    }
                                }
                                Err(e) => {
                                    ToolOutput::fail(format!("tool '{tool_name}' not found: {e}"))
                                }
                            };

                            // ── Self-correction: verify result ──────────────────
                            let enriched_data = self
                                .verify_tool_result(tool_name, args, &tool_result, &tool_context, &event_tx)
                                .await;                        session.add_message(ModelMessage {
                            role: MessageRole::Tool,
                            content: serde_json::to_string(&enriched_data)
                                .unwrap_or_else(|_| "{}".into()),
                            content_parts: None,
                            tool_calls: None,
                            tool_call_id: Some(tool_id.to_string()),
                            name: Some(tool_name.to_string()),
                        });
                        }
                        continue; // lanjut ke turn berikutnya
                    } else {
                        // Tidak ada tool calls = jawaban final
                        if !full_text.is_empty() {
                            if event_tx
                                .send(AgentEvent::Thought {
                                    turn,
                                    content: full_text.clone(),
                                })
                                .is_err()
                            {
                                tracing::warn!("Thought event send failed (receiver may be dropped)");
                            }
                        }

                        session.add_message(ModelMessage {
                            role: MessageRole::Assistant,
                            content: full_text.clone(),
                            content_parts: None,
                            tool_calls: None,
                            tool_call_id: None,
                            name: None,
                        });

                        if event_tx
                            .send(AgentEvent::ObjectiveComplete {
                                summary: full_text,
                                evidence: vec![],
                            })
                            .is_err()
                        {
                            tracing::warn!("ObjectiveComplete event send failed (receiver may be dropped)");
                        }
                        session.state = SessionState::Completed;
                        return Ok(session);
                    }
                }
                Err(_stream_err) => {
                    // Fallback ke non-streaming jika provider tidak support stream
                    let fallbacks = &agent.genome.model_policy.fallback_profiles;
                    let response = self
                        .model_router
                        .route_with_fallback(profile_id, fallbacks, chat_request)
                        .await?;

                    session.record_turn(response.usage.total_tokens, response.usage.cost_usd);

                    // Enforce budget per turn
                    if let Err(e) = self.budget_service.reserve(&mut agent.budget, response.usage.cost_usd) {
                        if event_tx
                            .send(AgentEvent::BudgetWarning {
                                remaining: agent.budget.remaining(),
                            })
                            .is_err()
                        {
                            tracing::warn!("BudgetWarning (fallback) event send failed (receiver may be dropped)");
                        }
                        return Err(AgentError::from(e));
                    }
                    self.record_cost(
                        &agent.id.0.to_string(),
                        &agent.mission_id.0.to_string(),
                        response.usage.cost_usd,
                        CostCategory::ModelCall,
                    )
                    .await;


                    let has_tool_calls = response.finish_reason == FinishReason::ToolCalls
                        || response.message.tool_calls.as_ref().map_or(false, |tc| !tc.is_empty());

                    if !response.message.content.is_empty() {
                        if event_tx
                            .send(AgentEvent::Thought {
                                turn,
                                content: response.message.content.clone(),
                            })
                            .is_err()
                        {
                            tracing::warn!("Thought event send failed (receiver may be dropped)");
                        }
                    }

                    let msg = response.message.clone();
                    session.add_message(msg);

                    if has_tool_calls {
                        if let Some(tool_calls) = &response.message.tool_calls {
                            for tc in tool_calls {
                                let tool_name = &tc.name;
                                let args = &tc.arguments;

                                if event_tx
                                    .send(AgentEvent::ToolCall {
                                        tool: tool_name.clone(),
                                        args: args.clone(),
                                        result: serde_json::Value::Null,
                                    })
                                    .is_err()
                                {
                                    tracing::warn!("ToolCall event send failed (receiver may be dropped)");
                                }

                                let tool_result = match self.tool_registry.get(tool_name) {
                                    Ok(tool) => {
                                        let approved = self.wait_for_tool_approval(agent, tool_name, args, &tc.id).await?;
                                        if approved {
                                            match tool.execute(&tool_context, args.clone()).await {
                                                Ok(output) => {
                                                    if event_tx
                                                        .send(AgentEvent::ToolCall {
                                                            tool: tool_name.clone(),
                                                            args: args.clone(),
                                                            result: output.data.clone(),
                                                        })
                                                        .is_err()
                                                    {
                                                        tracing::warn!("ToolCall result event send failed (receiver may be dropped)");
                                                    }
                                                    output
                                                }
                                                Err(e) => ToolOutput::fail(e.to_string()),
                                            }
                                        } else {
                                            let output = ToolOutput::fail("Tool invocation denied by operator".to_string());
                                            if event_tx
                                                .send(AgentEvent::ToolCall {
                                                    tool: tool_name.clone(),
                                                    args: args.clone(),
                                                    result: output.data.clone(),
                                                })
                                                .is_err()
                                            {
                                                tracing::warn!("ToolCall denied event send failed (receiver may be dropped)");
                                            }
                                            output
                                        }
                                    }
                                    Err(e) => {
                                        ToolOutput::fail(format!("tool '{tool_name}' not found: {e}"))
                                    }
                                };

                                // ── Self-correction: verify result ──────────────────
                                let enriched_data = self
                                    .verify_tool_result(tool_name, args, &tool_result, &tool_context, &event_tx)
                                    .await;

                                session.add_message(ModelMessage {
                                    role: MessageRole::Tool,
                                    content: serde_json::to_string(&enriched_data)
                                        .unwrap_or_else(|_| "{}".into()),
                                    content_parts: None,
                                    tool_calls: None,
                                    tool_call_id: Some(tc.id.clone()),
                                    name: Some(tool_name.clone()),
                                });
                            }
                        }
                        continue;
                    }

                    if event_tx
                        .send(AgentEvent::ObjectiveComplete {
                            summary: response.message.content.clone(),
                            evidence: vec![],
                        })
                        .is_err()
                    {
                        tracing::warn!("ObjectiveComplete event send failed (receiver may be dropped)");
                    }
                    session.state = SessionState::Completed;
                    return Ok(session);
                }
            }
        }

        // Batas giliran tercapai tanpa penyelesaian
        Err(AgentError::MaxTurnsReached(max_turns))
    }

    async fn wait_for_tool_approval(
        &self,
        agent: &Agent,
        tool_name: &str,
        args: &serde_json::Value,
        tool_call_id: &str,
    ) -> Result<bool, AgentError> {
        use claw10_domain::approval::{ToolApprovalRequest, ToolApprovalState};
        use claw10_domain::SideEffectClass;
        use claw10_store::StoreExt;

        // Tentukan apakah tool ini memerlukan approval berdasarkan SideEffectClass
        let needs_approval = match self.tool_registry.get(tool_name) {
            Ok(tool) => matches!(
                tool.side_effect_class(),
                SideEffectClass::ControlledWrite
                    | SideEffectClass::ExternalCommunication
                    | SideEffectClass::ProductionMutation
                    | SideEffectClass::Destructive
                    | SideEffectClass::Physical
            ),
            // Jika tool tidak ditemukan, biarkan lolos (akan error saat eksekusi)
            Err(_) => false,
        };

        if !needs_approval {
            return Ok(true);
        }

        // Cek always_allow di database (per-agent per-tool)
        let always_allow_key = format!("always_allow:{}:{}", agent.id.0, tool_name);
        if let Ok(Some(always)) = self.kv_store.get::<bool>(&always_allow_key).await {
            if always {
                return Ok(true);
            }
        }

        // Ambil command/detail dari args untuk ditampilkan ke user
        let command = args
            .get("command")
            .or_else(|| args.get("url"))
            .or_else(|| args.get("path"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Buat ToolApprovalRequest di database
        let approval_key = format!("tool_approval:{}", tool_call_id);
        let req = ToolApprovalRequest {
            id: tool_call_id.to_string(),
            agent_id: agent.id.clone(),
            tool_name: tool_name.to_string(),
            command,
            state: ToolApprovalState::Pending,
            created_at: chrono::Utc::now(),
        };

        if let Err(e) = self.kv_store.set(&approval_key, &req).await {
            return Err(AgentError::Other(format!("gagal simpan request approval: {e}")));
        }

        // Loop menunggu approval dengan exponential backoff dan timeout
        let timeout = std::time::Duration::from_secs(300);
        let start = std::time::Instant::now();
        let mut interval_ms = 100_u64;
        const MAX_INTERVAL_MS: u64 = 2000;

        loop {
            if start.elapsed() > timeout {
                return Err(AgentError::Other(
                    "timeout menunggu approval tool (300 detik)".into(),
                ));
            }

            // Cek status approval
            if let Ok(Some(current_req)) =
                self.kv_store.get::<ToolApprovalRequest>(&approval_key).await
            {
                match current_req.state {
                    ToolApprovalState::Approved => {
                        return Ok(true);
                    }
                    ToolApprovalState::AlwaysApproved => {
                        // Set always allow ke true
                        if self.kv_store.set(&always_allow_key, &true).await.is_err() {
                            tracing::warn!("Gagal menyimpan always_allow key");
                        }
                        return Ok(true);
                    }
                    ToolApprovalState::Denied => {
                        return Ok(false);
                    }
                    ToolApprovalState::Pending => {}
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(interval_ms)).await;
            interval_ms = (interval_ms * 2).min(MAX_INTERVAL_MS);
        }
    }

    /// Verify a tool result and enrich it with self-correction context.
    ///
    /// Runs text-based verification checks based on the tool type.
    /// If verification fails and retries are available, auto-retries.
    /// Returns the enriched tool data with `_verification` field.
    async fn verify_tool_result(
        &self,
        tool_name: &str,
        args: &serde_json::Value,
        result: &ToolOutput,
        context: &ToolContext,
        event_tx: &mpsc::UnboundedSender<AgentEvent>,
    ) -> serde_json::Value {
        // Run verification
        let verification = self.verifier.verify(tool_name, args, result, context).await;

        match verification {
            VerificationResult::Success => {
                // Enrich with success marker for LLM context
                VerificationEngine::enrich_success(result)
            }
            VerificationResult::Failed { ref reason, ref suggestion } => {
                let mut tracker = RetryTracker::new();
                tracker.record_failure(reason.clone());

                // Emit retry event
                if event_tx
                    .send(AgentEvent::ToolRetry {
                        tool: tool_name.to_string(),
                        attempt: tracker.attempt,
                        max_retries: 3,
                        reason: reason.clone(),
                    })
                    .is_err()
                {
                    tracing::warn!("ToolRetry event send failed");
                }

                // Emit verification failure event
                if event_tx
                    .send(AgentEvent::VerificationFailed {
                        tool: tool_name.to_string(),
                        reason: reason.clone(),
                        suggestion: suggestion.clone(),
                    })
                    .is_err()
                {
                    tracing::warn!("VerificationFailed event send failed");
                }

                tracing::warn!(
                    "[Self-Correct] Tool '{}' failed verification: {}. Retries exhausted.",
                    tool_name, reason
                );

                // Auto-retry for shell commands (up to 2 more times)
                if tool_name == "shell" && tracker.can_retry() {
                    // Re-execute the same command as auto-retry
                    let retry_cmd = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
                    let retry_timeout = args.get("timeout_seconds").and_then(|v| v.as_u64()).unwrap_or(30);
                    let retry_args = serde_json::json!({
                        "action": "exec",
                        "command": retry_cmd,
                        "timeout_seconds": retry_timeout,
                    });

                    if event_tx
                        .send(AgentEvent::ToolRetry {
                            tool: tool_name.to_string(),
                            attempt: 2,
                            max_retries: 3,
                            reason: format!("Auto-retry after: {reason}"),
                        })
                        .is_err()
                    {
                        tracing::warn!("ToolRetry event send failed");
                    }

                    // Retry the command
                    if let Ok(tool) = self.tool_registry.get(tool_name) {
                        if let Ok(retry_result) = tool.execute(context, retry_args).await {
                            let retry_verification = self.verifier.verify(tool_name, args, &retry_result, context).await;
                            if retry_verification.is_success() {
                                return VerificationEngine::enrich_success(&retry_result);
                            }
                        }
                    }
                }

                // Enrich with failure context for LLM
                VerificationEngine::enrich_failure(result, &verification, &tracker)
            }
            VerificationResult::RequiresScreenshot { ref reason } => {
                // For now, just mark as ambiguous and let LLM decide
                let mut enriched = result.data.clone();
                enriched["_verification"] = serde_json::json!({
                    "status": "ambiguous",
                    "reason": reason,
                    "suggestion": "Screenshot verification recommended",
                });
                enriched
            }
            VerificationResult::OcrMismatch { ref expected, ref actual, confidence } => {
                let mut tracker = RetryTracker::new();
                tracker.record_failure(format!("OCR mismatch: expected '{}' but got '{}' (confidence: {})", expected, actual, confidence));

                // Emit retry event (consistency with Failed arm)
                if event_tx
                    .send(AgentEvent::ToolRetry {
                        tool: tool_name.to_string(),
                        attempt: tracker.attempt,
                        max_retries: 3,
                        reason: format!("OCR mismatch: expected '{}' not found", expected),
                    })
                    .is_err()
                {
                    tracing::warn!("ToolRetry event send failed");
                }

                // Emit verification failure event
                if event_tx
                    .send(AgentEvent::VerificationFailed {
                        tool: tool_name.to_string(),
                        reason: format!("OCR text mismatch — expected '{}' not found in OCR output", expected),
                        suggestion: Some(format!("The screen shows '{}' but expected '{}'. Try a different approach or adjust the target area.", actual, expected)),
                    })
                    .is_err()
                {
                    tracing::warn!("VerificationFailed event send failed");
                }

                tracing::warn!(
                    "[Self-Correct] Tool '{}' OCR mismatch: expected='{}' actual='{}' confidence={}",
                    tool_name, expected, actual, confidence
                );

                VerificationEngine::enrich_failure(result, &verification, &tracker)
            }
        }
    }

    /// Menyimpan CostRecord secara persisten ke KV store.
    async fn record_cost(
        &self,
        agent_id: &str,
        mission_id: &str,
        amount: f64,
        category: CostCategory,
    ) {
        if amount <= 0.0 {
            return;
        }
        let record = BudgetService::create_cost_record(
            mission_id.to_string(),
            agent_id.to_string(),
            amount,
            category,
        );
        let key = format!("cost_record:{}", uuid::Uuid::now_v7());
        if let Err(e) = self.kv_store.set(&key, &record).await {
            tracing::warn!("Gagal menyimpan cost record secara persisten: {e}");
        }
    }
}
