use std::sync::Arc;
use async_trait::async_trait;
use serde_json::json;

use claw10_domain::{
    ChildSpawnPolicy, ChildSpec, SpawnRequest, SpawnRequestId, SpawnState,
    SwarmTeamSpec, TerminationPolicy, SideEffectClass
};
use claw10_tool::{registry::Tool, context::ToolContext, error::ToolError, result::ToolOutput};
use claw10_store::{Store, StoreExt};

pub struct SpawnTool {
    kv_store: Arc<dyn Store>,
}

impl SpawnTool {
    pub fn new(kv_store: Arc<dyn Store>) -> Self {
        Self { kv_store }
    }
}

#[async_trait]
impl Tool for SpawnTool {
    fn name(&self) -> &'static str {
        "spawn"
    }

    fn description(&self) -> &'static str {
        "Request spawning a new child agent to perform a specific sub-task/objective. Useful for dividing work and recursive delegation."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "role": {
                    "type": "string",
                    "description": "The specific role/specialty of the child agent (e.g., 'Security Engineer', 'DB Designer')"
                },
                "objective": {
                    "type": "string",
                    "description": "The detailed objective or task the child agent must accomplish"
                },
                "budget_usd": {
                    "type": "number",
                    "description": "Max budget in USD allowed for this child agent. Default: 1.0"
                },
                "model_profile": {
                    "type": "string",
                    "description": "Model profile name to run the child agent. Default: default"
                }
            },
            "required": ["role", "objective"]
        })
    }


    fn categories(&self) -> Vec<&str> {
        vec!["swarm", "orchestration"]
    }

    fn side_effect_class(&self) -> SideEffectClass {
        SideEffectClass::ControlledWrite
    }

    async fn execute(
        &self,
        context: &ToolContext,
        args: serde_json::Value,
    ) -> Result<ToolOutput, ToolError> {
        let role = args.get("role")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("missing role".to_string()))?;

        let objective = args.get("objective")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("missing objective".to_string()))?;

        let budget_usd = args.get("budget_usd")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(1.0);

        let model_profile = args.get("model_profile")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        let request_id = SpawnRequestId(uuid::Uuid::now_v7());
        let spawn_request = SpawnRequest {
            id: request_id.clone(),
            mission_id: context.mission_id.clone(),
            task_id: Some(context.task_id.0.to_string()),
            requested_by: context.agent_id.clone(),
            reason: format!("LLM spawn call for child role '{role}'"),
            team: SwarmTeamSpec {
                name: format!("{role}-team"),
                lifecycle_mode: claw10_domain::LifecycleMode::Ephemeral,
                ttl_seconds: Some(3600),
                idle_timeout_seconds: Some(300),
            },
            children: vec![ChildSpec {
                role: role.to_string(),
                objective: objective.to_string(),
                budget_usd,
                model_profile: model_profile.to_string(),
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
            state: SpawnState::Pending,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let key = format!("spawnreq:{}", request_id.0);
        self.kv_store.set(&key, &spawn_request).await
            .map_err(|e| ToolError::Other(format!("gagal simpan spawn request: {e}")))?;

        // Loop memantau database untuk menunggu keputusan persetujuan user di TUI
        let timeout = std::time::Duration::from_secs(300);
        let start = std::time::Instant::now();
        let mut interval_ms = 100_u64;
        const MAX_INTERVAL_MS: u64 = 2000;

        loop {
            if start.elapsed() > timeout {
                return Ok(ToolOutput::fail(format!(
                    "Spawn request untuk child '{role}' timeout setelah 300 detik."
                )));
            }

            if let Ok(Some(current_req)) = self.kv_store.get::<SpawnRequest>(&key).await {
                match current_req.state {
                    SpawnState::Completed => {
                        return Ok(ToolOutput::ok(json!({
                            "spawn_request_id": request_id.0.to_string(),
                            "status": "Approved",
                            "message": format!(
                                "Spawn request untuk child '{}' disetujui oleh user dan agen berhasil diluncurkan.",
                                role
                            )
                        })));
                    }
                    SpawnState::Denied => {
                        return Ok(ToolOutput::fail(format!(
                            "Spawn request untuk child '{role}' ditolak oleh user."
                        )));
                    }
                    _ => {}
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(interval_ms)).await;
            interval_ms = (interval_ms * 2).min(MAX_INTERVAL_MS);
        }
    }
}
