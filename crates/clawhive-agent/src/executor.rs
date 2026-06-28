use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc;

use clawhive_budget::BudgetService;
use clawhive_domain::Agent;
use clawhive_model_router::router::ModelRouter;
use clawhive_model_router::types::{ChatRequest, FinishReason, MessageRole, ModelMessage};
use clawhive_tool::context::ToolContext;
use clawhive_tool::registry::ToolRegistry;
use clawhive_tool::result::ToolOutput;

use crate::context::ContextBuilder;
use crate::error::AgentError;
use crate::events::AgentEvent;
use crate::session::{AgentSession, SessionState};

pub type EventSender = mpsc::UnboundedSender<AgentEvent>;

pub struct AgentExecutor {
    model_router: Arc<ModelRouter>,
    tool_registry: Arc<ToolRegistry>,
    _budget_service: Arc<BudgetService>,
}

impl AgentExecutor {
    #[must_use]
    pub fn new(
        model_router: Arc<ModelRouter>,
        tool_registry: Arc<ToolRegistry>,
        budget_service: Arc<BudgetService>,
    ) -> Self {
        Self {
            model_router,
            tool_registry,
            _budget_service: budget_service,
        }
    }

    pub async fn execute(
        &self,
        agent: &Agent,
        objective: &str,
        context: HashMap<String, String>,
        tool_context: ToolContext,
        max_turns: u32,
    ) -> Result<(AgentSession, Vec<AgentEvent>), AgentError> {
        let mut session = AgentSession::new(agent.id.clone());
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let mut events = Vec::new();

        event_tx
            .send(AgentEvent::SessionStarted {
                agent_id: agent.id.0.to_string(),
                session_id: session.id.0.to_string(),
                objective: objective.to_string(),
            })
            .ok();

        let messages = ContextBuilder::build_initial_messages(agent, objective, &context);
        for msg in messages {
            session.add_message(msg);
        }

        let tool_defs = ContextBuilder::tool_definitions(&self.tool_registry);

        for turn in 0..max_turns {
            if session.state != SessionState::Active {
                break;
            }

            if agent.budget.is_soft_limit_reached() {
                event_tx
                    .send(AgentEvent::BudgetWarning {
                        remaining: agent.budget.remaining(),
                    })
                    .ok();
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

            event_tx
                .send(AgentEvent::ModelCall {
                    turn,
                    tokens: 0,
                    cost: 0.0,
                })
                .ok();

            let response = self
                .model_router
                .route_with_fallback(profile_id, fallbacks, chat_request)
                .await?;

            session.record_turn(response.usage.total_tokens, response.usage.cost_usd);

            let msg = response.message.clone();
            let has_tool_calls = response.finish_reason == FinishReason::ToolCalls
                || response.message.tool_calls.is_some();

            if !response.message.content.is_empty() {
                event_tx
                    .send(AgentEvent::Thought {
                        turn,
                        content: response.message.content.clone(),
                    })
                    .ok();
            }

            session.add_message(msg);

            if has_tool_calls {
                if let Some(tool_calls) = &response.message.tool_calls {
                    for tc in tool_calls {
                        let tool_name = &tc.name;
                        let args = &tc.arguments;

                        let tool_result = match self.tool_registry.get(tool_name) {
                            Ok(tool) => {
                                event_tx
                                    .send(AgentEvent::ToolCall {
                                        tool: tool_name.clone(),
                                        args: args.clone(),
                                        result: serde_json::Value::Null,
                                    })
                                    .ok();

                                match tool.execute(&tool_context, args.clone()).await {
                                    Ok(output) => {
                                        event_tx
                                            .send(AgentEvent::ToolCall {
                                                tool: tool_name.clone(),
                                                args: args.clone(),
                                                result: output.data.clone(),
                                            })
                                            .ok();
                                        output
                                    }
                                    Err(e) => ToolOutput::fail(e.to_string()),
                                }
                            }
                            Err(e) => {
                                ToolOutput::fail(format!("tool '{tool_name}' not found: {e}"))
                            }
                        };

                        session.add_message(ModelMessage {
                            role: MessageRole::Tool,
                            content: serde_json::to_string(&tool_result.data)
                                .unwrap_or_else(|_| "{}".into()),
                            tool_calls: None,
                            tool_call_id: Some(tc.id.clone()),
                            name: Some(tool_name.clone()),
                        });
                    }
                }
                continue;
            }

            // LLM responded without tool calls = final answer
            events.extend(self.drain_events(&mut event_rx).await);
            event_tx
                .send(AgentEvent::ObjectiveComplete {
                    summary: response.message.content.clone(),
                    evidence: vec![],
                })
                .ok();
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

    /// Versi streaming dari execute — emit setiap AgentEvent langsung ke `event_tx`
    /// sehingga TUI bisa menampilkan progres real-time tanpa menunggu seluruh loop selesai.
    pub async fn execute_streaming(
        &self,
        agent: &Agent,
        objective: &str,
        context: HashMap<String, String>,
        tool_context: ToolContext,
        max_turns: u32,
        event_tx: EventSender,
    ) -> Result<AgentSession, AgentError> {
        let mut session = AgentSession::new(agent.id.clone());

        event_tx
            .send(AgentEvent::SessionStarted {
                agent_id: agent.id.0.to_string(),
                session_id: session.id.0.to_string(),
                objective: objective.to_string(),
            })
            .ok();

        let messages = ContextBuilder::build_initial_messages(agent, objective, &context);
        for msg in messages {
            session.add_message(msg);
        }

        let tool_defs = ContextBuilder::tool_definitions(&self.tool_registry);

        for turn in 0..max_turns {
            if session.state != SessionState::Active {
                break;
            }

            if agent.budget.is_soft_limit_reached() {
                event_tx
                    .send(AgentEvent::BudgetWarning {
                        remaining: agent.budget.remaining(),
                    })
                    .ok();
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

            event_tx
                .send(AgentEvent::ModelCall {
                    turn,
                    tokens: 0,
                    cost: 0.0,
                })
                .ok();

            let response = self
                .model_router
                .route_with_fallback(profile_id, fallbacks, chat_request)
                .await?;

            session.record_turn(response.usage.total_tokens, response.usage.cost_usd);

            let msg = response.message.clone();
            let has_tool_calls = response.finish_reason == FinishReason::ToolCalls
                || response.message.tool_calls.is_some();

            if !response.message.content.is_empty() {
                event_tx
                    .send(AgentEvent::Thought {
                        turn,
                        content: response.message.content.clone(),
                    })
                    .ok();
            }

            session.add_message(msg);

            if has_tool_calls {
                if let Some(tool_calls) = &response.message.tool_calls {
                    for tc in tool_calls {
                        let tool_name = &tc.name;
                        let args = &tc.arguments;

                        // Emit event "sedang memanggil tool" sebelum eksekusi
                        event_tx
                            .send(AgentEvent::ToolCall {
                                tool: tool_name.clone(),
                                args: args.clone(),
                                result: serde_json::Value::Null,
                            })
                            .ok();

                        let tool_result = match self.tool_registry.get(tool_name) {
                            Ok(tool) => match tool.execute(&tool_context, args.clone()).await {
                                Ok(output) => {
                                    // Emit event "tool selesai" dengan hasil
                                    event_tx
                                        .send(AgentEvent::ToolCall {
                                            tool: tool_name.clone(),
                                            args: args.clone(),
                                            result: output.data.clone(),
                                        })
                                        .ok();
                                    output
                                }
                                Err(e) => ToolOutput::fail(e.to_string()),
                            },
                            Err(e) => {
                                ToolOutput::fail(format!("tool '{tool_name}' not found: {e}"))
                            }
                        };

                        session.add_message(ModelMessage {
                            role: MessageRole::Tool,
                            content: serde_json::to_string(&tool_result.data)
                                .unwrap_or_else(|_| "{}".into()),
                            tool_calls: None,
                            tool_call_id: Some(tc.id.clone()),
                            name: Some(tool_name.clone()),
                        });
                    }
                }
                continue;
            }

            // LLM menjawab tanpa tool call = jawaban final
            event_tx
                .send(AgentEvent::ObjectiveComplete {
                    summary: response.message.content.clone(),
                    evidence: vec![],
                })
                .ok();
            session.state = SessionState::Completed;
            return Ok(session);
        }

        // Batas giliran tercapai tanpa penyelesaian
        Err(AgentError::MaxTurnsReached(max_turns))
    }
}
