use axum::{
    Router,
    routing::{delete, get, post},
};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::handlers::{
    agent, approval, gateway, health, lifecycle, lineage, memory, mission, policy, scheduler,
    spawn, task, worker,
};
use crate::state::AppState;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .route("/health", get(health::health_check))
        .route(
            "/v1/missions",
            get(mission::list_missions).post(mission::create_mission),
        )
        .route("/v1/missions/{id}", get(mission::get_mission))
        .route("/v1/tasks", get(task::list_tasks).post(task::create_task))
        .route("/v1/tasks/{id}", get(task::get_task))
        .route("/v1/agents", get(agent::list_agents))
        .route("/v1/agents/{id}", get(agent::get_agent))
        .route("/v1/agents/{id}/pause", post(agent::pause_agent))
        .route("/v1/agents/{id}/execute", post(agent::execute_agent))
        .route("/v1/agents/{id}/terminate", post(agent::terminate_agent))
        .route(
            "/v1/spawn-requests",
            get(spawn::list_spawn_requests).post(spawn::create_spawn_request),
        )
        .route("/v1/spawn-requests/{id}", get(spawn::get_spawn_request))
        .route(
            "/v1/spawn-requests/{id}/approve",
            post(spawn::approve_spawn),
        )
        .route("/v1/spawn-requests/{id}/deny", post(spawn::deny_spawn))
        .route("/v1/lineages/{id}", get(lineage::get_lineage))
        .route("/v1/agents/{id}/legacy", get(lineage::get_agent_legacy))
        .route("/v1/policies/compile", post(policy::compile_policy))
        .route("/v1/policies/simulate", post(policy::simulate_policy))
        .route("/v1/policies/evaluate", post(policy::evaluate_policy))
        .route("/v1/approvals", get(approval::list_approvals))
        .route(
            "/v1/approvals/{id}/approve",
            post(approval::approve_request),
        )
        .route("/v1/approvals/{id}/deny", post(approval::deny_request))
        // Worker endpoints
        .route(
            "/v1/workers",
            get(worker::list_workers).post(worker::register_worker),
        )
        .route("/v1/workers/{id}", get(worker::get_worker))
        .route("/v1/workers/{id}/heartbeat", post(worker::worker_heartbeat))
        .route("/v1/workers/{id}/drain", post(worker::drain_worker))
        .route("/v1/workers/{id}/offline", post(worker::mark_offline))
        .route(
            "/v1/workers/{id}/quarantine",
            post(worker::quarantine_worker),
        )
        .route("/v1/workers/stale", get(worker::stale_workers))
        .route("/v1/workers/counts", get(worker::worker_counts))
        // Lifecycle endpoints
        .route(
            "/v1/agents/{id}/checkpoints",
            get(lifecycle::list_checkpoints).post(lifecycle::create_checkpoint),
        )
        .route(
            "/v1/agents/{id}/hibernate",
            post(lifecycle::hibernate_agent),
        )
        .route("/v1/agents/{id}/wake", post(lifecycle::wake_agent))
        .route(
            "/v1/agents/{id}/heartbeat",
            post(lifecycle::heartbeat_agent),
        )
        .route("/v1/agents/stale", get(lifecycle::list_stale_agents))
        .route("/v1/agents/{id}/migrate", post(lifecycle::migrate_agent))
        .route("/v1/agents/{id}/lease", post(lifecycle::assign_lease))
        // Scheduler endpoints
        .route(
            "/v1/agents/{agent_id}/schedules",
            get(scheduler::list_schedules).post(scheduler::add_schedule),
        )
        .route(
            "/v1/agents/{agent_id}/schedules/{index}",
            delete(scheduler::remove_schedule),
        )
        .route("/v1/schedules/due", get(scheduler::get_due_schedules))
        // Memory endpoints
        .route(
            "/v1/memories",
            get(memory::query_memories).post(memory::store_memory),
        )
        .route(
            "/v1/memories/{id}",
            get(memory::get_memory)
                .put(memory::update_memory)
                .delete(memory::delete_memory),
        )
        .route("/v1/memories/{id}/verify", post(memory::verify_memory))
        .route(
            "/v1/memories/{id}/transition",
            post(memory::transition_memory),
        )
        .route("/v1/memories/counts", get(memory::memory_counts))
        // Gateway endpoints
        .route(
            "/v1/gateway/channels",
            get(gateway::list_channels).post(gateway::register_channel),
        )
        .route("/v1/gateway/channels/{id}", get(gateway::get_channel))
        .route(
            "/v1/gateway/channels/{id}/activate",
            post(gateway::activate_channel),
        )
        .route(
            "/v1/gateway/channels/{id}/deactivate",
            post(gateway::deactivate_channel),
        )
        .route(
            "/v1/gateway/channels/{id}/dispatch",
            post(gateway::dispatch_message),
        )
        .route("/v1/gateway/sessions", post(gateway::create_session))
        .route("/v1/gateway/sessions/{id}", get(gateway::get_session))
        .route(
            "/v1/gateway/sessions/{id}/terminate",
            post(gateway::terminate_session),
        )
        .route(
            "/v1/identities/{identity_id}/sessions",
            get(gateway::list_identity_sessions),
        )
        .with_state(state)
}
