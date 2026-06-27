use axum::{
    routing::{get, post},
    Router,
};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::handlers::{
    agent, approval, health, lineage, mission, policy, spawn, task, worker,
};
use crate::state::AppState;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .route("/health", get(health::health_check))
        .route("/v1/missions", get(mission::list_missions).post(mission::create_mission))
        .route("/v1/missions/{id}", get(mission::get_mission))
        .route("/v1/tasks", get(task::list_tasks).post(task::create_task))
        .route("/v1/tasks/{id}", get(task::get_task))
        .route("/v1/agents", get(agent::list_agents))
        .route("/v1/agents/{id}", get(agent::get_agent))
        .route("/v1/agents/{id}/pause", post(agent::pause_agent))
        .route("/v1/agents/{id}/terminate", post(agent::terminate_agent))
        .route("/v1/spawn-requests", get(spawn::list_spawn_requests).post(spawn::create_spawn_request))
        .route("/v1/spawn-requests/{id}", get(spawn::get_spawn_request))
        .route("/v1/spawn-requests/{id}/approve", post(spawn::approve_spawn))
        .route("/v1/spawn-requests/{id}/deny", post(spawn::deny_spawn))
        .route("/v1/lineages/{id}", get(lineage::get_lineage))
        .route("/v1/agents/{id}/legacy", get(lineage::get_agent_legacy))
        .route("/v1/policies/compile", post(policy::compile_policy))
        .route("/v1/policies/simulate", post(policy::simulate_policy))
        .route("/v1/approvals", get(approval::list_approvals))
        .route("/v1/approvals/{id}/approve", post(approval::approve_request))
        .route("/v1/approvals/{id}/deny", post(approval::deny_request))
        .route("/v1/workers", get(worker::list_workers))
        .route("/v1/workers/{id}/drain", post(worker::drain_worker))
        .with_state(state)
}
