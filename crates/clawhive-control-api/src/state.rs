use std::sync::Arc;

use clawhive_agent::AgentStore;
use clawhive_auth::credential::CredentialService;
use clawhive_auth::identity::IdentityService;
use clawhive_auth::rbac::RbacService;
use clawhive_domain::SwarmLimitsConfig;
use clawhive_event::InMemoryEventBus;
use clawhive_gateway::GatewayService;
use clawhive_memory::MemoryService;
use clawhive_model_router::router::ModelRouter;
use clawhive_scheduler::ScheduleService;
use clawhive_spawn::broker::SpawnBroker;
use clawhive_store::{InMemoryStore as KvInMemory, Store};
use clawhive_telemetry::TelemetryService;
use clawhive_tool::registry::ToolRegistry;
use clawhive_worker::WorkerService;

pub use crate::store::*;

#[derive(Clone)]
pub struct AppState {
    pub identity_service: Arc<IdentityService>,
    pub rbac_service: Arc<std::sync::Mutex<RbacService>>,
    pub credential_service: Arc<CredentialService>,
    pub scheduler_service: Arc<ScheduleService>,
    pub worker_service: Arc<WorkerService>,
    pub memory_service: Arc<MemoryService>,
    pub gateway_service: Arc<GatewayService>,
    pub spawn_broker: Arc<SpawnBroker>,
    pub telemetry: TelemetryService,
    pub kv_store: Arc<dyn Store>,
    pub model_router: Option<Arc<ModelRouter>>,
    pub tool_registry: Option<Arc<ToolRegistry>>,
}

impl AppState {
    /// Create AppState dengan in-memory KV store.
    #[must_use]
    pub fn new() -> Self {
        Self::new_with_store(Arc::new(KvInMemory::new()))
    }

    /// Create AppState dengan shared KV store (untuk prod dengan sled).
    #[must_use]
    pub fn new_with_store(kv_store: Arc<dyn Store>) -> Self {
        let limits = SwarmLimitsConfig {
            max_spawn_depth: 5,
            max_children_per_agent: 10,
            max_agents_per_mission: 100,
            max_concurrent_agents: 50,
            max_persistent_children_per_agent: 5,
            max_turns_per_ephemeral_agent: 100,
            max_idle_seconds_ephemeral: 600,
        };

        // AgentStore menggunakan KV store yang sama
        let agent_store = Arc::new(AgentStore::new(Arc::clone(&kv_store)));

        // Event bus: in-memory untuk development, nanti bisa diganti NatsEventBus
        let event_bus = Arc::new(InMemoryEventBus::new());

        Self {
            identity_service: Arc::new(IdentityService),
            rbac_service: Arc::new(std::sync::Mutex::new(RbacService::new())),
            credential_service: Arc::new(CredentialService),
            scheduler_service: Arc::new(ScheduleService::new(Arc::clone(&kv_store))),
            worker_service: Arc::new(WorkerService::new(Arc::clone(&kv_store))),
            memory_service: Arc::new(MemoryService::new(Arc::clone(&kv_store))),
            gateway_service: Arc::new(GatewayService::new(Arc::clone(&kv_store))),
            spawn_broker: Arc::new(SpawnBroker::new(limits, agent_store, event_bus)),
            telemetry: TelemetryService::default(),
            kv_store,
            model_router: None,
            tool_registry: None,
        }
    }

    /// Create AppState dengan model router dan tool registry untuk agent execution.
    #[must_use]
    pub fn new_with_services(
        kv_store: Arc<dyn Store>,
        model_router: Arc<ModelRouter>,
        tool_registry: Arc<ToolRegistry>,
    ) -> Self {
        let mut state = Self::new_with_store(kv_store);
        state.model_router = Some(model_router);
        state.tool_registry = Some(tool_registry);
        state
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

