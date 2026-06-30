use axum::{
    Json,
    extract::{Path, Query, State},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use clawhive_domain::{Agent, CheckpointReason, RuntimeLease};
use clawhive_event::ClawHiveEvent;
use clawhive_event::events::{WakeTrigger};
use clawhive_lifecycle::LifecycleService;
use clawhive_store::StoreExt;

use crate::error::ApiError;
use crate::state::AppState;
use crate::store::AGENT_PREFIX;

#[derive(Deserialize)]
pub struct CreateCheckpointRequest {
    pub reason: CheckpointReason,
}

#[derive(Serialize)]
pub struct CheckpointResponse {
    pub id: String,
    pub agent_id: String,
    pub reason: String,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct HeartbeatResponse {
    pub remaining_seconds: i64,
}

#[derive(Deserialize)]
pub struct WakeRequest {
    pub worker_id: String,
    pub renewal_interval_seconds: u64,
}

#[derive(Deserialize)]
pub struct MigrateRequest {
    pub target_worker_id: String,
    pub renewal_interval_seconds: u64,
}

#[derive(Deserialize)]
pub struct StaleQuery {
    pub grace_seconds: Option<i64>,
}

#[derive(Deserialize)]
pub struct AssignLeaseRequest {
    pub worker_id: String,
    pub renewal_interval_seconds: u64,
}

#[derive(Serialize)]
pub struct StaleAgentResponse {
    pub id: String,
    pub name: String,
    pub state: String,
    pub worker_id: Option<String>,
    pub lease_expires_at: Option<String>,
}

/// POST /v1/agents/{id}/checkpoints
pub async fn create_checkpoint(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<CreateCheckpointRequest>,
) -> Result<Json<CheckpointResponse>, ApiError> {
    let key = format!("{AGENT_PREFIX}{id}");
    let mut agent: Agent = state
        .kv_store
        .get(&key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("agent not found: {id}")))?;

    let cp = LifecycleService::create_checkpoint(&agent, body.reason);
    agent.checkpoints.push(cp.clone());
    agent.updated_at = Utc::now();
    state.kv_store.set(&key, &agent).await?;

    let _ = state.telemetry.record("checkpoint.created", "success", |e| {
        e.with_agent_id(agent.id.0.to_string())
            .with_mission_id(agent.mission_id.0.to_string())
    });

    Ok(Json(CheckpointResponse {
        id: cp.id.0.to_string(),
        agent_id: cp.agent_id,
        reason: format!("{:?}", cp.reason),
        created_at: cp.created_at.to_rfc3339(),
    }))
}

/// GET /v1/agents/{id}/checkpoints
pub async fn list_checkpoints(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<CheckpointResponse>>, ApiError> {
    let key = format!("{AGENT_PREFIX}{id}");
    let agent: Agent = state
        .kv_store
        .get(&key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("agent not found: {id}")))?;

    Ok(Json(
        agent
            .checkpoints
            .iter()
            .map(|cp| CheckpointResponse {
                id: cp.id.0.to_string(),
                agent_id: cp.agent_id.clone(),
                reason: format!("{:?}", cp.reason),
                created_at: cp.created_at.to_rfc3339(),
            })
            .collect(),
    ))
}

/// POST /v1/agents/{id}/hibernate
pub async fn hibernate_agent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CheckpointResponse>, ApiError> {
    let key = format!("{AGENT_PREFIX}{id}");
    let mut agent: Agent = state
        .kv_store
        .get(&key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("agent not found: {id}")))?;

    LifecycleService::hibernate(&mut agent).map_err(|e| ApiError::Validation(e.to_string()))?;
    state.kv_store.set(&key, &agent).await?;

    let _ = state.telemetry.record("agent.hibernated", "success", |e| {
        e.with_agent_id(agent.id.0.to_string())
            .with_mission_id(agent.mission_id.0.to_string())
    });

    let _ = state.event_bus.publish(ClawHiveEvent::AgentHibernated {
        agent_id: agent.id.0,
        checkpoint_id: agent.checkpoints.last().map(|c| c.id.0).unwrap_or_default(),
        timestamp: chrono::Utc::now(),
    }).await;

    let cp = agent.checkpoints.last().ok_or_else(|| {
        ApiError::Internal("hibernate succeeded but no checkpoint created".into())
    })?;

    Ok(Json(CheckpointResponse {
        id: cp.id.0.to_string(),
        agent_id: cp.agent_id.clone(),
        reason: format!("{:?}", cp.reason),
        created_at: cp.created_at.to_rfc3339(),
    }))
}

/// POST /v1/agents/{id}/wake
pub async fn wake_agent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<WakeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let key = format!("{AGENT_PREFIX}{id}");
    let mut agent: Agent = state
        .kv_store
        .get(&key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("agent not found: {id}")))?;

    let lease = RuntimeLease {
        worker_id: body.worker_id,
        acquired_at: Utc::now(),
        expires_at: Utc::now() + chrono::Duration::seconds(body.renewal_interval_seconds as i64),
        renewal_interval_seconds: body.renewal_interval_seconds,
    };
    LifecycleService::wake(&mut agent, lease).map_err(|e| ApiError::Validation(e.to_string()))?;
    state.kv_store.set(&key, &agent).await?;

    let _ = state.telemetry.record("agent.woken", "success", |e| {
        e.with_agent_id(agent.id.0.to_string())
            .with_mission_id(agent.mission_id.0.to_string())
    });

    let _ = state.event_bus.publish(ClawHiveEvent::AgentWoken {
        agent_id: agent.id.0,
        trigger: WakeTrigger::ManualWake,
        timestamp: chrono::Utc::now(),
    }).await;

    Ok(Json(serde_json::json!({ "status": "woken" })))
}

/// POST /v1/agents/{id}/heartbeat
pub async fn heartbeat_agent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<HeartbeatResponse>, ApiError> {
    let key = format!("{AGENT_PREFIX}{id}");
    let mut agent: Agent = state
        .kv_store
        .get(&key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("agent not found: {id}")))?;

    let remaining =
        LifecycleService::heartbeat(&mut agent).map_err(|e| ApiError::Validation(e.to_string()))?;
    state.kv_store.set(&key, &agent).await?;

    let _ = state.telemetry.record("agent.heartbeat", "success", |e| {
        e.with_agent_id(agent.id.0.to_string())
    });

    Ok(Json(HeartbeatResponse {
        remaining_seconds: remaining.num_seconds(),
    }))
}

/// GET /v1/agents/stale
pub async fn list_stale_agents(
    State(state): State<AppState>,
    Query(query): Query<StaleQuery>,
) -> Result<Json<Vec<StaleAgentResponse>>, ApiError> {
    let all_agents: Vec<Agent> = state
        .kv_store
        .scan_prefix_unsorted::<Agent>(AGENT_PREFIX)
        .await?
        .into_iter()
        .map(|(_, a)| a)
        .collect();

    let grace = query.grace_seconds.unwrap_or(30);
    let stale = LifecycleService::detect_stale(&all_agents, grace);

    Ok(Json(
        stale
            .into_iter()
            .map(|a| StaleAgentResponse {
                id: a.id.0.to_string(),
                name: a.name.clone(),
                state: format!("{:?}", a.state),
                worker_id: a.current_runtime.as_ref().map(|l| l.worker_id.clone()),
                lease_expires_at: a
                    .current_runtime
                    .as_ref()
                    .map(|l| l.expires_at.to_rfc3339()),
            })
            .collect(),
    ))
}

/// POST /v1/agents/{id}/migrate
pub async fn migrate_agent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<MigrateRequest>,
) -> Result<Json<CheckpointResponse>, ApiError> {
    let key = format!("{AGENT_PREFIX}{id}");
    let mut agent: Agent = state
        .kv_store
        .get(&key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("agent not found: {id}")))?;

    LifecycleService::migrate(&mut agent, &body.target_worker_id, body.renewal_interval_seconds)
        .map_err(|e| ApiError::Validation(e.to_string()))?;
    state.kv_store.set(&key, &agent).await?;

    let _ = state.telemetry.record("agent.migrated", "success", |e| {
        e.with_agent_id(agent.id.0.to_string())
            .with_worker_id(body.target_worker_id.clone())
    });

    let from_worker = agent.current_runtime.as_ref().map(|l| l.worker_id.clone()).unwrap_or_default();
    let _ = state.event_bus.publish(ClawHiveEvent::AgentMigrated {
        agent_id: agent.id.0,
        from_worker,
        to_worker: body.target_worker_id.clone(),
        checkpoint_id: agent.checkpoints.last().map(|c| c.id.0).unwrap_or_default(),
        timestamp: chrono::Utc::now(),
    }).await;

    let cp = agent.checkpoints.last().ok_or_else(|| {
        ApiError::Internal("migrate succeeded but no checkpoint created".into())
    })?;

    Ok(Json(CheckpointResponse {
        id: cp.id.0.to_string(),
        agent_id: cp.agent_id.clone(),
        reason: format!("{:?}", cp.reason),
        created_at: cp.created_at.to_rfc3339(),
    }))
}

/// POST /v1/agents/{id}/lease
pub async fn assign_lease(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<AssignLeaseRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let key = format!("{AGENT_PREFIX}{id}");
    let mut agent: Agent = state
        .kv_store
        .get(&key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("agent not found: {id}")))?;

    LifecycleService::assign_lease(&mut agent, &body.worker_id, body.renewal_interval_seconds);
    state.kv_store.set(&key, &agent).await?;

    let _ = state.telemetry.record("agent.lease_assigned", "success", |e| {
        e.with_agent_id(agent.id.0.to_string())
            .with_worker_id(body.worker_id.clone())
    });

    Ok(Json(serde_json::json!({ "status": "lease assigned" })))
}
