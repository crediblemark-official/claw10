//! Helper untuk inisialisasi `AgentRuntime` dan default agent di sesi TUI.
//!
//! Setiap sesi TUI memiliki satu "tui-agent" yang berjalan sebagai
//! root agent. Semua pesan user diteruskan sebagai objective ke agent ini.

use std::sync::Arc;

use claw10_agent::runtime::AgentRuntime;
use claw10_agent::store::AgentStore;
use claw10_budget::BudgetService;
use claw10_domain::{
    Agent, AgentId, AgentState, LifecycleMode, MissionId, PolicyBundle, PolicyBundleId, WorkerId,
    WorkerCapability, WorkerType,
    agent::{AgentGenome, AutonomyConfig, MemoryConfig, ModelPolicy, NetworkPolicy, RuntimeConfig},
    budget::Budget,
    identity::IdentityId,
    lineage::LineageId,
};
use claw10_model_router::router::ModelRouter;
use claw10_tool::registry::ToolRegistry;
use claw10_worker::WorkerService;

/// Bangun `AgentRuntime` dengan `BudgetService` dan default worker TUI.
///
/// Mengembalikan `(runtime, worker_id)` untuk disimpan di `TuiApp`.
pub async fn build_tui_runtime(
    kv_store: Arc<dyn claw10_store::Store>,
    model_router: Arc<ModelRouter>,
    _ignored_tool_registry: Arc<ToolRegistry>,
    worker_service: Arc<WorkerService>,
) -> Result<(AgentRuntime, WorkerId), String> {
    // Daftarkan default worker TUI
    let worker = worker_service
        .register(
            "tui-local".to_string(),
            WorkerType::Local,
            vec![WorkerCapability {
                name: "llm".to_string(),
                version: None,
            }],
            "0.1.0".to_string(),
        )
        .await;
    let worker_id = worker.id.clone();

    // Daftarkan semua builtin tools agar agent dapat berinteraksi dengan sistem nyata
    let mut tool_registry = ToolRegistry::new();
    tool_registry.register(Box::new(claw10_tool::builtin::ShellTool));
    tool_registry.register(Box::new(claw10_tool::builtin::ReadFileTool));
    tool_registry.register(Box::new(claw10_tool::builtin::WriteFileTool));
    tool_registry.register(Box::new(claw10_tool::builtin::HttpTool));
    tool_registry.register(Box::new(claw10_tool::builtin::DeclareArtifactTool::new(Arc::clone(&kv_store))));
    tool_registry.register(Box::new(crate::spawn_tool::SpawnTool::new(Arc::clone(&kv_store))));
    let tool_registry_arc = Arc::new(tool_registry);



    let agent_store = AgentStore::new(Arc::clone(&kv_store));
    // BudgetService adalah unit struct, tidak punya Default
    let budget_service = Arc::new(BudgetService);

    let runtime = AgentRuntime::new(
        agent_store,
        model_router,
        tool_registry_arc,
        budget_service,
        worker_service,
        Some(worker_id.clone()),
    );

    Ok((runtime, worker_id))
}


/// Buat agent default ephemeral untuk sesi TUI dengan model yang dipilih.
pub fn make_default_agent(agent_id: AgentId, model_id: &str, mission_id: MissionId) -> Agent {
    Agent {
        id: agent_id,
        identity_id: IdentityId(uuid::Uuid::nil()),
        mission_id,
        parent_agent_id: None,
        lineage_id: LineageId(uuid::Uuid::now_v7()),
        name: "TUI Root Agent".to_string(),
        role: "assistant".to_string(),
        genome: AgentGenome {
            id: "tui-default".to_string(),
            version: "1.0".to_string(),
            role: "assistant".to_string(),
            lifecycle_modes: vec![LifecycleMode::Ephemeral],
            model_policy: ModelPolicy {
                preferred_profile: model_id.to_string(),
                fallback_profiles: vec![],
                max_context_tokens: 8192,
            },
            autonomy: AutonomyConfig {
                can_spawn: true,
                max_spawn_depth: 3,
                max_children: 5,
            },
            delegable_permissions: vec![],
            non_delegable_permissions: vec![],
            memory: MemoryConfig {
                default_read_scopes: vec!["session".to_string()],
                default_write_scope: Some("session".to_string()),
            },
            runtime: RuntimeConfig {
                preferred_class: "local".to_string(),
                network: NetworkPolicy::AllowByDefault,
            },
            verification_required: false,
        },
        state: AgentState::Active,
        lifecycle_mode: LifecycleMode::Ephemeral,
        persistent_pattern: None,
        budget: Budget {
            allocated_usd: 10.0,
            spent_usd: 0.0,
            soft_limit_usd: Some(8.0), // peringatan di $8
            hard_limit_usd: Some(10.0),
            recurring_monthly_usd: None,
        },
        delegable_permissions: vec![],
        non_delegable_permissions: vec![],
        current_runtime: None,
        checkpoints: vec![],
        subscriptions: vec![],
        schedules: vec![],
        policy_bundle: PolicyBundle {
            id: PolicyBundleId(uuid::Uuid::nil()),
            name: "default".to_string(),
            version: "1.0".to_string(),
            rules: vec![],
            is_active: true,
            signed_by: None,
            signature: None,
            activated_at: None,
            created_at: chrono::Utc::now(),
        },
        turn_count: 0,
        total_cost_usd: 0.0,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        terminated_at: None,
    }
}
