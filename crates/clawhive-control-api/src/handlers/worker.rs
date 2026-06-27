use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::state::AppState;

#[derive(Serialize)]
pub struct WorkerResponse {
    pub id: String,
    pub worker_type: String,
    pub state: String,
}

pub async fn list_workers(
    State(_state): State<AppState>,
) -> Json<Vec<WorkerResponse>> {
    Json(vec![])
}

pub async fn drain_worker(
    State(_state): State<AppState>,
    Path(_id): Path<Uuid>,
) -> Json<WorkerResponse> {
    Json(WorkerResponse {
        id: String::new(),
        worker_type: String::new(),
        state: String::new(),
    })
}
