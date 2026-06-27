use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::AppState;

#[derive(Serialize)]
pub struct SpawnResponse {
    pub id: String,
    pub state: String,
}

#[derive(Deserialize)]
pub struct CreateSpawnRequest {
    pub mission_id: String,
    pub reason: String,
}

pub async fn list_spawn_requests(
    State(_state): State<AppState>,
) -> Json<Vec<SpawnResponse>> {
    Json(vec![])
}

pub async fn get_spawn_request(
    State(_state): State<AppState>,
    Path(_id): Path<Uuid>,
) -> Json<SpawnResponse> {
    Json(SpawnResponse {
        id: String::new(),
        state: String::new(),
    })
}

pub async fn create_spawn_request(
    State(_state): State<AppState>,
    Json(_req): Json<CreateSpawnRequest>,
) -> Json<SpawnResponse> {
    Json(SpawnResponse {
        id: String::new(),
        state: String::new(),
    })
}

pub async fn approve_spawn(
    State(_state): State<AppState>,
    Path(_id): Path<Uuid>,
) -> Json<SpawnResponse> {
    Json(SpawnResponse {
        id: String::new(),
        state: String::new(),
    })
}

pub async fn deny_spawn(
    State(_state): State<AppState>,
    Path(_id): Path<Uuid>,
) -> Json<SpawnResponse> {
    Json(SpawnResponse {
        id: String::new(),
        state: String::new(),
    })
}
