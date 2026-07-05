use claw10_control_api::store::{AGENT_PREFIX, MISSION_PREFIX, SPAWNREQ_PREFIX};
use claw10_domain::{
    Agent, AgentId, AgentState, ChildSpawnPolicy, ChildSpec, Mission, SpawnRequest, SpawnRequestId,
    SpawnState, SwarmTeamSpec, TerminationPolicy, ToolApprovalRequest, ToolApprovalState,
};
use claw10_store::StoreExt;

use crate::app::{Tab, TuiApp};

impl TuiApp {
    pub(crate) async fn execute_command(&mut self, cmd: &str) {
        let trimmed = cmd.trim();
        if trimmed.is_empty() {
            return;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        let command = parts[0].to_lowercase();

        let result = match command.as_str() {
            "help" => {
                let providers_list = claw10_model_router::providers::provider_configs();
                let names: Vec<&str> = providers_list.iter().map(|c| c.name).collect();
                let help = format!(
"\
Commands:
  :help                              Show this help
  :refresh                           Refresh all data
  :apikey <provider> <key>           Set API key for a provider (alias: :connect)
  :pause <agent_id|name>             Pause agent
  :terminate <agent_id|name>         Terminate agent
  :approve <spawn_id>                Approve spawn request
  :approve tool <id>                 Approve tool execution request
  :approve always [tool] [id]        Always approve tool request
  :approve                           Approve the currently pending tool request
  :deny <spawn_id>                   Deny spawn request
  :deny tool <id>                    Deny tool execution request
  :deny                              Deny the currently pending tool request
  :spawn <mission> <role> <objective> <budget>  Create spawn request
  :goto <agents|workers|spawn>       Switch tab
  :save <filename.md>                Save last assistant response to a markdown file
  :workspace / :home / :ws           Kembali ke Workspace Selector
  :q                                 Quit TUI

Available providers ({} total):
  {}
Use Ctrl+P palette to set API keys.
Type any message to start a chat with the active model.",
                    names.len(),
                    names.chunks(5).map(|chunk| chunk.join(", ")).collect::<Vec<_>>().join(",\n  ")
                );
                help
            }
            "apikey" | "connect" => {
                if parts.len() < 2 {
                    "Usage: :apikey <provider> <your_api_key>".into()
                } else if parts.len() < 3 {
                    format!("Missing API key for '{}'. Usage: :apikey {} <key>", parts[1], parts[1])
                } else {
                    let provider_type = parts[1].to_lowercase();
                    let api_key = parts[2..].join(" ");
                    self.persist_api_key(&provider_type, &api_key).await;
                    self.register_all_providers().await;
                    format!("{} API key set & saved.", provider_type)
                }
            }
            "refresh" => {
                self.status_message = "Refreshing...".into();
                return;
            }
            "pause" => {
                if parts.len() < 2 {
                    "Usage: :pause <agent_id|name>".into()
                } else {
                    let id_str = parts[1];
                    let agents = self
                        .state
                        .kv_store
                        .scan_prefix::<Agent>(AGENT_PREFIX)
                        .await
                        .unwrap_or_default();
                    match agents.into_iter().find(|(_, a)| {
                        a.id.0.to_string().starts_with(id_str) || a.name == id_str
                    }) {
                        Some((key, mut agent))
                            if matches!(
                                agent.state,
                                AgentState::Active | AgentState::Hibernating
                            ) =>
                        {
                            agent.state = AgentState::Paused;
                            agent.updated_at = chrono::Utc::now();
                            let _ = self.state.kv_store.set(&key, &agent).await;
                            format!("Paused agent {}", agent.name)
                        }
                        Some((_, agent)) => format!(
                            "Agent {} is in state {:?} (cannot pause)",
                            agent.name, agent.state
                        ),
                        None => format!("Agent not found: {id_str}"),
                    }
                }
            }
            "clear" => {
                self.clear_app_data().await;
                "Semua cache, history, dan context window berhasil dibersihkan.".into()
            }
            "model" => {
                if parts.len() < 2 {
                    if let Some(router) = &self.state.model_router {
                        let profiles = router.registry().list_profiles();
                        if profiles.is_empty() {
                            "Tidak ada model yang terkonfigurasi. Set API key terlebih dahulu.".into()
                        } else {
                            let mut msg = "Model terkonfigurasi yang tersedia:\n".to_string();
                            for p in profiles {
                                msg.push_str(&format!("  - {} ({})\n", p.id, p.provider));
                            }
                            msg.push_str("\nGunakan ':model <model_id>' untuk beralih.");
                            msg
                        }
                    } else {
                        "Tidak ada model yang terkonfigurasi. Set API key terlebih dahulu.".into()
                    }
                } else {
                    let target_model = parts[1];
                    let mut matched = false;
                    if let Some(router) = &self.state.model_router {
                        let profiles = router.registry().list_profiles();
                        if profiles.iter().any(|p| p.id == target_model || p.model_name == target_model) {
                            matched = true;
                        }
                    }
                    if matched {
                        self.set_active_model(target_model.to_string());
                        self.init_agent_runtime().await;
                        format!("Model berhasil dialihkan ke: {}", target_model)
                    } else {
                        format!("Model '{}' tidak terkonfigurasi. Gunakan ':model' untuk melihat daftar.", target_model)
                    }
                }
            }
            "terminate" => {
                if parts.len() < 2 {
                    "Usage: :terminate <agent_id|name>".into()
                } else {
                    let id_str = parts[1];
                    let agents = self
                        .state
                        .kv_store
                        .scan_prefix::<Agent>(AGENT_PREFIX)
                        .await
                        .unwrap_or_default();
                    match agents.into_iter().find(|(_, a)| {
                        a.id.0.to_string().starts_with(id_str) || a.name == id_str
                    }) {
                        Some((key, mut agent)) => {
                            agent.state = AgentState::Terminated;
                            agent.updated_at = chrono::Utc::now();
                            agent.terminated_at = Some(chrono::Utc::now());
                            let _ = self.state.kv_store.set(&key, &agent).await;
                            format!("Terminated agent {}", agent.name)
                        }
                        None => format!("Agent not found: {id_str}"),
                    }
                }
            }
            "approve" => self.handle_approve_command(&parts).await,
            "deny" => self.handle_deny_command(&parts).await,
            "spawn" => {
                if parts.len() < 4 {
                    "Usage: :spawn <mission_id> <role> <objective> <budget>".into()
                } else {
                    let mission_id_str = parts[1];
                    let role = parts[2];
                    let objective = parts[3];
                    let budget: f64 = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(1.0);

                    let missions = self
                        .state
                        .kv_store
                        .scan_prefix::<Mission>(MISSION_PREFIX)
                        .await
                        .unwrap_or_default();
                    let mission_id_uuid = uuid::Uuid::parse_str(mission_id_str).ok();
                    let mission = missions
                        .into_iter()
                        .find(|(_, m)| {
                            Some(m.id.0) == mission_id_uuid
                                || m.objective.contains(mission_id_str)
                        })
                        .map(|(_, m)| m);

                    match mission {
                        Some(m) => {
                            let request = SpawnRequest {
                                id: SpawnRequestId(uuid::Uuid::now_v7()),
                                mission_id: m.id,
                                task_id: None,
                                requested_by: AgentId(uuid::Uuid::now_v7()),
                                reason: format!("TUI spawn for {role}"),
                                team: SwarmTeamSpec {
                                    name: format!("{role}-team"),
                                    lifecycle_mode: claw10_domain::LifecycleMode::Ephemeral,
                                    ttl_seconds: Some(3600),
                                    idle_timeout_seconds: Some(300),
                                },
                                children: vec![ChildSpec {
                                    role: role.to_string(),
                                    objective: objective.to_string(),
                                    budget_usd: budget,
                                    model_profile: "default".into(),
                                    max_turns: 100,
                                    custom_permissions: None,
                                }],
                                child_spawn_policy: ChildSpawnPolicy {
                                    allowed: true,
                                    max_depth: Some(3),
                                    max_children: Some(5),
                                },
                                termination: TerminationPolicy {
                                    on_task_complete: true,
                                    on_parent_terminated: true,
                                    on_budget_exhausted: true,
                                },
                                state: SpawnState::Approved,
                                created_at: chrono::Utc::now(),
                                updated_at: chrono::Utc::now(),
                            };
                            let spawn_key = format!("{SPAWNREQ_PREFIX}{}", request.id.0);
                            let _ = self.state.kv_store.set(&spawn_key, &request).await;
                            format!("Created spawn request for {role}")
                        }
                        None => format!("Mission not found: {mission_id_str}"),
                    }
                }
            }
            "goto" => {
                if parts.len() < 2 {
                    "Usage: :goto <agents|workers|spawn|missions|tasks|memory|approvals|costs|policies|skills|artifacts|logs|incidents>".into()
                } else {
                    let target = parts[1];
                    let maybe_tab = match target {
                        "agents" => Some((Tab::Agents, "Agents")),
                        "workers" => Some((Tab::Workers, "Workers")),
                        "spawn" => Some((Tab::SpawnRequests, "Spawn Requests")),
                        "missions" => Some((Tab::Missions, "Missions")),
                        "tasks" => Some((Tab::Tasks, "Tasks")),
                        "memory" => Some((Tab::Memory, "Memory")),
                        "approvals" => Some((Tab::Approvals, "Approvals")),
                        "costs" => Some((Tab::Costs, "Costs")),
                        "policies" => Some((Tab::Policies, "Policies")),
                        "skills" => Some((Tab::Skills, "Skills")),
                        "artifacts" => Some((Tab::Artifacts, "Artifacts")),
                        "incidents" => Some((Tab::Incidents, "Incidents")),
                        _ => None,
                    };
                    match maybe_tab {
                        Some((tab, label)) => {
                            self.selected_tab = tab;
                            self.selected_index = 0;
                            self.active_screen = crate::app::TuiApp::screen_for_tab(tab);
                            format!("Switched to {label}")
                        }
                        None => format!("Unknown tab: {target}"),
                    }
                }
            }
            "save" => {
                if parts.len() < 2 {
                    "Usage: :save <filename.md>".into()
                } else {
                    let filename = parts[1];
                    let last_assistant_msg = self.chat_history.iter().rev().find(|(sender, _, _)| {
                        sender.to_lowercase() == "agent" || sender.to_lowercase() == "assistant"
                    });
                    match last_assistant_msg {
                        Some((_, _, content)) => {
                            if content.is_empty() {
                                "Pesan terakhir kosong, tidak ada yang bisa disimpan.".into()
                            } else {
                                match tokio::fs::write(filename, content).await {
                                    Ok(_) => format!("Berhasil menyimpan jawaban terakhir ke file '{}'", filename),
                                    Err(e) => format!("Gagal menulis file: {}", e),
                                }
                            }
                        }
                        None => "Tidak ada jawaban asisten yang bisa disimpan.".into()
                    }
                }
            }
            "home" | "workspace" | "ws" => {
                // Kembali ke Home screen (Workspace Selector) dan reset workspace aktif
                self.active_workspace = None;
                self.chat_history.clear();
                self.active_screen = crate::app::Screen::Home;
                self.load_workspaces().await;
                "Kembali ke Workspace Selector.".into()
            }
            "q" => {
                self.should_quit = true;
                "Goodbye!".into()
            }
            other => format!("Unknown command: {other}. Type :help for commands"),
        };

        self.status_message = result;
    }

    pub(crate) fn interrupt_agent(&mut self) {
        if self.is_streaming {
            if let Some(task) = self.agent_task.take() {
                task.abort();
            }
            self.is_streaming = false;
            self.stream_status = None;
            self.agent_rx = None;
            self.stream_rx = None;
            self.active_agent_id = None;
            self.chat_history.push((
                "System".to_string(),
                String::new(),
                "Agent diinterupsi oleh user (proses dibatalkan).".to_string(),
            ));
            self.status_message = "Agent execution interrupted.".into();
        }
    }

    async fn handle_approve_command(&mut self, parts: &[&str]) -> String {
        if parts.len() == 1 {
            // Bare :approve → act on the currently pending tool approval.
            if let Some(req) = self.pending_tool_approval.clone() {
                self.handle_tool_approval(ToolApprovalState::Approved).await;
                return format!("Approved tool request {}", req.id);
            }
            return "Usage: :approve <spawn_id> | :approve tool <id> | :approve always [tool] [id]".into();
        }

        if parts[1].eq_ignore_ascii_case("tool") {
            if parts.len() < 3 {
                return "Usage: :approve tool <tool_approval_id>".into();
            }
            let id_str = parts[2];
            return self
                .update_tool_approval_state(id_str, ToolApprovalState::Approved)
                .await;
        }

        if parts[1].eq_ignore_ascii_case("always") {
            if parts.len() >= 3 && parts[2].eq_ignore_ascii_case("tool") {
                if parts.len() < 4 {
                    return "Usage: :approve always tool <tool_approval_id>".into();
                }
                let id_str = parts[3];
                return self
                    .update_tool_approval_state(id_str, ToolApprovalState::AlwaysApproved)
                    .await;
            }
            // :approve always → act on pending tool approval
            if let Some(req) = self.pending_tool_approval.clone() {
                self.handle_tool_approval(ToolApprovalState::AlwaysApproved).await;
                return format!("Always approved tool request {}", req.id);
            }
            return "Usage: :approve always [tool] [id]".into();
        }

        // Default: treat as spawn request id
        let id_str = parts[1];
        let requests = self
            .state
            .kv_store
            .scan_prefix::<SpawnRequest>(SPAWNREQ_PREFIX)
            .await
            .unwrap_or_default();
        match requests.into_iter().find(|(_, r)| r.id.0.to_string().starts_with(id_str)) {
            Some((key, mut req)) if req.state == SpawnState::Pending => {
                req.state = SpawnState::Approved;
                req.updated_at = chrono::Utc::now();
                let _ = self.state.kv_store.set(&key, &req).await;
                format!("Approved spawn request {}", req.id.0)
            }
            Some((_, req)) => format!("Spawn request is {:?} (not pending)", req.state),
            None => format!("Spawn request not found: {id_str}"),
        }
    }

    async fn handle_deny_command(&mut self, parts: &[&str]) -> String {
        if parts.len() == 1 {
            // Bare :deny → act on the currently pending tool approval.
            if let Some(req) = self.pending_tool_approval.clone() {
                self.handle_tool_approval(ToolApprovalState::Denied).await;
                return format!("Denied tool request {}", req.id);
            }
            return "Usage: :deny <spawn_id> | :deny tool <id>".into();
        }

        if parts[1].eq_ignore_ascii_case("tool") {
            if parts.len() < 3 {
                return "Usage: :deny tool <tool_approval_id>".into();
            }
            let id_str = parts[2];
            return self
                .update_tool_approval_state(id_str, ToolApprovalState::Denied)
                .await;
        }

        // Default: treat as spawn request id
        let id_str = parts[1];
        let requests = self
            .state
            .kv_store
            .scan_prefix::<SpawnRequest>(SPAWNREQ_PREFIX)
            .await
            .unwrap_or_default();
        match requests.into_iter().find(|(_, r)| r.id.0.to_string().starts_with(id_str)) {
            Some((key, mut req)) if req.state == SpawnState::Pending => {
                req.state = SpawnState::Denied;
                req.updated_at = chrono::Utc::now();
                let _ = self.state.kv_store.set(&key, &req).await;
                format!("Denied spawn request {}", req.id.0)
            }
            Some((_, req)) => format!("Spawn request is {:?} (not pending)", req.state),
            None => format!("Spawn request not found: {id_str}"),
        }
    }

    async fn update_tool_approval_state(&mut self, id_str: &str, state: ToolApprovalState) -> String {
        use claw10_store::StoreExt;
        let requests = self
            .state
            .kv_store
            .scan_prefix::<ToolApprovalRequest>("tool_approval:")
            .await
            .unwrap_or_default();
        match requests.into_iter().find(|(_, r)| r.id.starts_with(id_str)) {
            Some((key, mut req)) if req.state == ToolApprovalState::Pending => {
                req.state = state.clone();
                if self.state.kv_store.set(&key, &req).await.is_ok() {
                    // If the acted request is the pending one, clear it.
                    if self
                        .pending_tool_approval
                        .as_ref()
                        .map(|p| p.id == req.id)
                        .unwrap_or(false)
                    {
                        self.pending_tool_approval = None;
                    }
                    format!("Tool request {} is now {:?}", req.id, state)
                } else {
                    "Failed to save tool approval state".into()
                }
            }
            Some((_, req)) => format!("Tool request is {:?} (not pending)", req.state),
            None => format!("Tool approval not found: {id_str}"),
        }
    }
}
