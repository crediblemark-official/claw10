use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use clawhive_agent::{
    AgentRuntime, AgentStore,
    error::AgentError,
};
use clawhive_domain::{Agent, AgentState, WorkerId};
use clawhive_lifecycle::LifecycleService;
use clawhive_store::StoreExt;

use crate::error::ApiError;
use crate::state::AppState;
use crate::store::AGENT_PREFIX;

#[derive(Serialize)]
pub struct AgentResponse {
    pub id: String,
    pub name: String,
    pub role: String,
    pub state: String,
    pub lifecycle_mode: String,
    pub parent_agent_id: Option<String>,
}

#[derive(Deserialize)]
pub struct AgentsQuery {
    pub state: Option<String>,
    pub mission_id: Option<String>,
}

/// GET /v1/agents
pub async fn list_agents(
    State(state): State<AppState>,
    Query(query): Query<AgentsQuery>,
) -> Result<Json<Vec<AgentResponse>>, ApiError> {
    let agents = state
        .kv_store
        .scan_prefix::<Agent>(AGENT_PREFIX)
        .await?
        .into_iter()
        .map(|(_, a)| a)
        .filter(|a| {
            if let Some(ref s) = query.state {
                let q = s.to_lowercase();
                let state_str = format!("{:?}", a.state).to_lowercase();
                if state_str != q {
                    return false;
                }
            }
            if let Some(ref m) = query.mission_id
                && a.mission_id.0.to_string() != *m
            {
                return false;
            }
            true
        })
        .map(|a| AgentResponse {
            id: a.id.0.to_string(),
            name: a.name,
            role: a.role,
            state: format!("{:?}", a.state),
            lifecycle_mode: format!("{:?}", a.lifecycle_mode),
            parent_agent_id: a.parent_agent_id.map(|id| id.0.to_string()),
        })
        .collect();

    Ok(Json(agents))
}

/// POST /v1/agents/{id}/execute
#[derive(Deserialize)]
pub struct ExecuteRequest {
    pub objective: String,
    pub context: Option<HashMap<String, String>>,
    pub worker_id: Option<String>,
}

/// Response from execute_agent.
#[derive(Serialize)]
pub struct ExecuteResponse {
    pub session_id: String,
    pub turn_count: u32,
    pub total_cost_usd: f64,
    pub total_tokens: u32,
    pub state: String,
    pub events: Vec<String>,
}

/// GET /v1/agents/{id}
pub async fn get_agent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<AgentResponse>, ApiError> {
    let key = format!("{AGENT_PREFIX}{id}");
    let agent = state
        .kv_store
        .get::<Agent>(&key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("agent {id}")))?;

    Ok(Json(AgentResponse {
        id: agent.id.0.to_string(),
        name: agent.name,
        role: agent.role,
        state: format!("{:?}", agent.state),
        lifecycle_mode: format!("{:?}", agent.lifecycle_mode),
        parent_agent_id: agent.parent_agent_id.map(|id| id.0.to_string()),
    }))
}

/// POST /v1/agents/{id}/pause
pub async fn pause_agent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<AgentResponse>, ApiError> {
    let key = format!("{AGENT_PREFIX}{id}");
    let mut agent = state
        .kv_store
        .get::<Agent>(&key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("agent {id}")))?;

    if !matches!(agent.state, AgentState::Active | AgentState::Hibernating) {
        return Err(ApiError::Validation(format!(
            "cannot pause agent in state {:?}",
            agent.state
        )));
    }

    agent.state = AgentState::Paused;
    agent.updated_at = chrono::Utc::now();
    state.kv_store.set(&key, &agent).await?;

    let _ = state.telemetry.record("agent.paused", "success", |e| {
        e.with_agent_id(agent.id.0.to_string())
            .with_mission_id(agent.mission_id.0.to_string())
    });

    Ok(Json(AgentResponse {
        id: agent.id.0.to_string(),
        name: agent.name,
        role: agent.role,
        state: format!("{:?}", agent.state),
        lifecycle_mode: format!("{:?}", agent.lifecycle_mode),
        parent_agent_id: agent.parent_agent_id.map(|id| id.0.to_string()),
    }))
}

/// POST /v1/agents/{id}/execute
pub async fn execute_agent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<ExecuteRequest>,
) -> Result<Json<ExecuteResponse>, ApiError> {
    let model_router = state
        .model_router
        .clone()
        .ok_or_else(|| ApiError::Internal("model router not configured".into()))?;
    let tool_registry = state
        .tool_registry
        .clone()
        .ok_or_else(|| ApiError::Internal("tool registry not configured".into()))?;

    let agent_store = AgentStore::new(Arc::clone(&state.kv_store));
    let budget_service = Arc::new(clawhive_budget::BudgetService);

    let default_worker_id = body
        .worker_id
        .map(|w| {
            w.parse::<Uuid>()
                .map(WorkerId)
                .map_err(|e| ApiError::Validation(format!("invalid worker_id: {e}")))
        })
        .transpose()?;

    let runtime = AgentRuntime::new(
        agent_store,
        model_router,
        tool_registry,
        budget_service,
        Arc::clone(&state.worker_service),
        default_worker_id,
    );

    let agent_id = clawhive_domain::AgentId(id);
    let (session, events) = runtime
        .execute_agent(&agent_id, body.objective, body.context.unwrap_or_default(), None)
        .await
        .map_err(|e| match &e {
            AgentError::AgentNotFound(_) => ApiError::NotFound(format!("agent {id}")),
            AgentError::BudgetExhausted => ApiError::Validation("budget exhausted".into()),
            _ => ApiError::Internal(e.to_string()),
        })?;

    let _ = state.telemetry.record("agent.executed", "success", |e| {
        e.with_agent_id(id.to_string())
            .with_mission_id(format!("{}", uuid::Uuid::nil()))
    });

    Ok(Json(ExecuteResponse {
        session_id: session.id.0.to_string(),
        turn_count: session.turn_count,
        total_cost_usd: session.total_cost_usd,
        total_tokens: session.total_tokens,
        state: format!("{:?}", session.state),
        events: events.iter().map(|e| format!("{e:?}")).collect(),
    }))
}

/// POST /v1/agents/{id}/terminate
pub async fn terminate_agent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<AgentResponse>, ApiError> {
    let key = format!("{AGENT_PREFIX}{id}");

    let _ = state
        .kv_store
        .get::<Agent>(&key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("agent {id}")))?;

    let all_agents: Vec<Agent> = state
        .kv_store
        .scan_prefix::<Agent>(AGENT_PREFIX)
        .await?
        .into_iter()
        .map(|(_, a)| a)
        .collect();

    let children_ids: Vec<Uuid> = all_agents
        .iter()
        .filter(|a| a.parent_agent_id.as_ref().map(|p| p.0) == Some(id))
        .map(|a| a.id.0)
        .collect();

    for child_id in &children_ids {
        let child_key = format!("{AGENT_PREFIX}{child_id}");
        if let Some(mut child) = state.kv_store.get::<Agent>(&child_key).await? {
            LifecycleService::terminate_descendant(&mut child);
            state.kv_store.set(&child_key, &child).await?;
        }
    }

    let mut agent = state
        .kv_store
        .get::<Agent>(&key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("agent {id}")))?;
    LifecycleService::terminate(&mut agent);
    state.kv_store.set(&key, &agent).await?;

    let _ = state.telemetry.record("agent.terminated", "success", |e| {
        e.with_agent_id(agent.id.0.to_string())
            .with_mission_id(agent.mission_id.0.to_string())
    });

    Ok(Json(AgentResponse {
        id: agent.id.0.to_string(),
        name: agent.name,
        role: agent.role,
        state: format!("{:?}", agent.state),
        lifecycle_mode: format!("{:?}", agent.lifecycle_mode),
        parent_agent_id: agent.parent_agent_id.map(|id| id.0.to_string()),
    }))
}
