use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use clawhive_domain::{WorkerHeartbeat, WorkerId, WorkerState, WorkerType};
use clawhive_event::ClawHiveEvent;

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Serialize)]
pub struct WorkerResponse {
    pub id: String,
    pub name: String,
    pub state: String,
    pub is_draining: bool,
}

#[derive(Deserialize)]
pub struct RegisterWorkerRequest {
    pub name: String,
    pub worker_type: WorkerType,
    pub capabilities: Vec<String>,
    pub version: String,
}

#[derive(Deserialize)]
pub struct HeartbeatRequest {
    pub cpu_percent: f64,
    pub memory_percent: f64,
    pub active_runtimes: u32,
    pub queue_depth: u32,
    pub tool_availability: Vec<String>,
    pub sandbox_healthy: bool,
}

#[derive(Deserialize)]
pub struct WorkersQuery {
    pub state: Option<String>,
}

/// POST /v1/workers
pub async fn register_worker(
    State(state): State<AppState>,
    Json(body): Json<RegisterWorkerRequest>,
) -> Result<(StatusCode, Json<WorkerResponse>), ApiError> {
    let capabilities = body
        .capabilities
        .into_iter()
        .map(|c| clawhive_domain::WorkerCapability {
            name: c,
            version: None,
        })
        .collect();

    let worker = state
        .worker_service
        .register(body.name, body.worker_type, capabilities, body.version)
        .await;

    let _ = state.telemetry.record("worker.registered", "success", |e| {
        e.with_worker_id(worker.id.0.to_string())
            .with_additional("worker_type".into(), format!("{:?}", worker.worker_type))
    });

    Ok((
        StatusCode::CREATED,
        Json(WorkerResponse {
            id: worker.id.0.to_string(),
            name: worker.name,
            state: format!("{:?}", worker.state),
            is_draining: worker.is_draining,
        }),
    ))
}

/// GET /v1/workers
pub async fn list_workers(
    State(state): State<AppState>,
    Query(query): Query<WorkersQuery>,
) -> Result<Json<Vec<WorkerResponse>>, ApiError> {
    let state_filter = query.state.as_deref().and_then(|s| match s {
        "Online" => Some(WorkerState::Online),
        "Offline" => Some(WorkerState::Offline),
        "Draining" => Some(WorkerState::Draining),
        "Quarantined" => Some(WorkerState::Quarantined),
        _ => None,
    });

    let workers = state
        .worker_service
        .list(state_filter.as_ref())
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(
        workers
            .into_iter()
            .map(|w| WorkerResponse {
                id: w.id.0.to_string(),
                name: w.name,
                state: format!("{:?}", w.state),
                is_draining: w.is_draining,
            })
            .collect(),
    ))
}

/// GET /v1/workers/{id}
pub async fn get_worker(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<WorkerResponse>, ApiError> {
    let worker = state
        .worker_service
        .get(&WorkerId(id))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound(format!("worker {id}")))?;

    Ok(Json(WorkerResponse {
        id: worker.id.0.to_string(),
        name: worker.name,
        state: format!("{:?}", worker.state),
        is_draining: worker.is_draining,
    }))
}

/// POST /v1/workers/{id}/heartbeat
pub async fn worker_heartbeat(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<HeartbeatRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let hb = WorkerHeartbeat {
        cpu_percent: body.cpu_percent,
        memory_percent: body.memory_percent,
        active_runtimes: body.active_runtimes,
        queue_depth: body.queue_depth,
        tool_availability: body.tool_availability,
        sandbox_healthy: body.sandbox_healthy,
        timestamp: chrono::Utc::now(),
    };

    state
        .worker_service
        .heartbeat(&WorkerId(id), hb)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("not found") {
                ApiError::NotFound(msg)
            } else {
                ApiError::Validation(msg)
            }
        })?;

    let _ = state.telemetry.record("worker.heartbeat", "success", |e| {
        e.with_worker_id(id.to_string())
    });

    let _ = state.event_bus.publish(ClawHiveEvent::WorkerHeartbeat {
        worker_id: id,
        timestamp: chrono::Utc::now(),
    }).await;

    Ok(Json(serde_json::json!({ "status": "heartbeat received" })))
}

/// POST /v1/workers/{id}/drain
pub async fn drain_worker(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<WorkerResponse>, ApiError> {
    state
        .worker_service
        .drain(&WorkerId(id))
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("not found") {
                ApiError::NotFound(msg)
            } else {
                ApiError::Validation(msg)
            }
        })?;

    let _ = state.telemetry.record("worker.drained", "success", |e| {
        e.with_worker_id(id.to_string())
    });

    let worker = state
        .worker_service
        .get(&WorkerId(id))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound(format!("worker {id}")))?;

    Ok(Json(WorkerResponse {
        id: worker.id.0.to_string(),
        name: worker.name,
        state: format!("{:?}", worker.state),
        is_draining: worker.is_draining,
    }))
}

/// POST /v1/workers/{id}/offline
pub async fn mark_offline(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .worker_service
        .mark_offline(&WorkerId(id))
        .await
        .map_err(|e| ApiError::NotFound(e.to_string()))?;

    let _ = state.telemetry.record("worker.offline", "success", |e| {
        e.with_worker_id(id.to_string())
    });

    Ok(Json(serde_json::json!({ "status": "marked offline" })))
}

/// POST /v1/workers/{id}/quarantine
pub async fn quarantine_worker(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .worker_service
        .quarantine(&WorkerId(id))
        .await
        .map_err(|e| ApiError::NotFound(e.to_string()))?;

    let _ = state.telemetry.record("worker.quarantined", "success", |e| {
        e.with_worker_id(id.to_string())
    });

    Ok(Json(serde_json::json!({ "status": "quarantined" })))
}

/// GET /v1/workers/stale
pub async fn stale_workers(
    State(state): State<AppState>,
) -> Result<Json<Vec<WorkerResponse>>, ApiError> {
    let stale = state
        .worker_service
        .detect_stale(30)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    for w in &stale {
        let _ = state.event_bus.publish(ClawHiveEvent::WorkerStale {
            worker_id: w.id.0,
            last_seen: chrono::Utc::now(),
            timestamp: chrono::Utc::now(),
        }).await;
    }

    Ok(Json(
        stale
            .into_iter()
            .map(|w| WorkerResponse {
                id: w.id.0.to_string(),
                name: w.name,
                state: format!("{:?}", w.state),
                is_draining: w.is_draining,
            })
            .collect(),
    ))
}

/// GET /v1/workers/counts
pub async fn worker_counts(
    State(state): State<AppState>,
) -> Result<Json<std::collections::HashMap<String, usize>>, ApiError> {
    let counts = state
        .worker_service
        .count_by_state()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(counts))
}
