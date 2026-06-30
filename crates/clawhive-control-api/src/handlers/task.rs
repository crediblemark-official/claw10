use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use clawhive_domain::{AgentId, Budget, MissionId, RiskLevel, TaskState};
use clawhive_task::TaskService;
use clawhive_store::StoreExt;

use crate::error::ApiError;
use crate::handlers::audit::{self, build_audit_event};
use crate::state::AppState;
use crate::store::TASK_PREFIX;

#[derive(Serialize)]
pub struct TaskResponse {
    pub id: String,
    pub objective: String,
    pub mission_id: String,
    pub state: String,
}

#[derive(Deserialize)]
pub struct CreateTaskRequest {
    pub objective: String,
    pub mission_id: String,
    pub owner_id: Option<String>,
    pub budget: Option<TaskBudget>,
}

#[derive(Deserialize)]
pub struct TaskBudget {
    pub allocated_usd: f64,
}

pub async fn list_tasks(
    State(state): State<AppState>,
) -> Result<Json<Vec<TaskResponse>>, ApiError> {
    let tasks = state
        .kv_store
        .scan_prefix::<clawhive_domain::Task>(TASK_PREFIX)
        .await?
        .into_iter()
        .map(|(_, t)| TaskResponse {
            id: t.id.0.to_string(),
            objective: t.objective,
            mission_id: t.mission_id.0.to_string(),
            state: format!("{:?}", t.state),
        })
        .collect();

    Ok(Json(tasks))
}

pub async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TaskResponse>, ApiError> {
    let key = format!("{TASK_PREFIX}{id}");
    let task = state
        .kv_store
        .get::<clawhive_domain::Task>(&key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("task {id}")))?;

    Ok(Json(TaskResponse {
        id: task.id.0.to_string(),
        objective: task.objective,
        mission_id: task.mission_id.0.to_string(),
        state: format!("{:?}", task.state),
    }))
}

pub async fn create_task(
    State(state): State<AppState>,
    Json(req): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<TaskResponse>), ApiError> {
    let mission_id = MissionId(
        Uuid::parse_str(&req.mission_id)
            .map_err(|e| ApiError::Validation(format!("invalid mission_id: {e}")))?,
    );

    let owner_id = AgentId(
        req.owner_id
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|e| ApiError::Validation(format!("invalid owner_id: {e}")))?
            .unwrap_or_else(Uuid::nil),
    );

    let budget = match req.budget {
        Some(b) => Budget {
            allocated_usd: b.allocated_usd,
            spent_usd: 0.0,
            soft_limit_usd: None,
            hard_limit_usd: None,
            recurring_monthly_usd: None,
        },
        None => Budget {
            allocated_usd: 10.0,
            spent_usd: 0.0,
            soft_limit_usd: None,
            hard_limit_usd: None,
            recurring_monthly_usd: None,
        },
    };

    let task = TaskService::create_task(
        mission_id,
        owner_id.clone(),
        req.objective,
        serde_json::Value::Null,
        serde_json::Value::Null,
        budget,
        RiskLevel("low".into()),
    );

    let key = format!("{TASK_PREFIX}{}", task.id.0);
    state.kv_store.set(&key, &task).await?;

    let _ = state.telemetry.record("task.created", "success", |e| {
        e.with_task_id(task.id.0.to_string())
            .with_mission_id(task.mission_id.0.to_string())
    });

    audit::emit_event(
        Arc::clone(&state.audit_service),
        build_audit_event(
            "task_created",
            Some(owner_id.0.to_string()),
            Some(task.mission_id.0.to_string()),
            Some(task.id.0.to_string()),
            serde_json::json!({"objective": task.objective}),
        ),
    );

    Ok((
        StatusCode::CREATED,
        Json(TaskResponse {
            id: task.id.0.to_string(),
            objective: task.objective,
            mission_id: task.mission_id.0.to_string(),
            state: format!("{:?}", task.state),
        }),
    ))
}

#[derive(Deserialize)]
pub struct TransitionTaskRequest {
    pub state: TaskState,
}

pub async fn transition_task(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<TransitionTaskRequest>,
) -> Result<Json<TaskResponse>, ApiError> {
    let key = format!("{TASK_PREFIX}{id}");
    let mut task = state
        .kv_store
        .get::<clawhive_domain::Task>(&key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("task {id}")))?;

    TaskService::transition(&mut task, req.state)
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    state.kv_store.set(&key, &task).await?;

    audit::emit_event(
        Arc::clone(&state.audit_service),
        build_audit_event(
            "task_transitioned",
            None,
            Some(task.mission_id.0.to_string()),
            Some(task.id.0.to_string()),
            serde_json::json!({"state": format!("{:?}", task.state)}),
        ),
    );

    Ok(Json(TaskResponse {
        id: task.id.0.to_string(),
        objective: task.objective,
        mission_id: task.mission_id.0.to_string(),
        state: format!("{:?}", task.state),
    }))
}
