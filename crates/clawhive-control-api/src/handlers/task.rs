use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::AppState;

#[derive(Serialize)]
pub struct TaskResponse {
    pub id: String,
    pub objective: String,
}

#[derive(Deserialize)]
pub struct CreateTaskRequest {
    pub objective: String,
    pub mission_id: String,
}

pub async fn list_tasks(
    State(_state): State<AppState>,
) -> Json<Vec<TaskResponse>> {
    Json(vec![])
}

pub async fn get_task(
    State(_state): State<AppState>,
    Path(_id): Path<Uuid>,
) -> Json<TaskResponse> {
    Json(TaskResponse {
        id: String::new(),
        objective: String::new(),
    })
}

pub async fn create_task(
    State(_state): State<AppState>,
    Json(_req): Json<CreateTaskRequest>,
) -> Json<TaskResponse> {
    Json(TaskResponse {
        id: String::new(),
        objective: String::new(),
    })
}
