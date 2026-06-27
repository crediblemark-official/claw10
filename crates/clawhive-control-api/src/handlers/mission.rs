use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::AppState;

#[derive(Serialize)]
pub struct MissionResponse {
    pub id: String,
    pub objective: String,
}

#[derive(Deserialize)]
pub struct CreateMissionRequest {
    pub objective: String,
    pub organization_id: String,
}

pub async fn list_missions(
    State(_state): State<AppState>,
) -> Json<Vec<MissionResponse>> {
    Json(vec![])
}

pub async fn get_mission(
    State(_state): State<AppState>,
    Path(_id): Path<Uuid>,
) -> Json<MissionResponse> {
    Json(MissionResponse {
        id: String::new(),
        objective: String::new(),
    })
}

pub async fn create_mission(
    State(_state): State<AppState>,
    Json(_req): Json<CreateMissionRequest>,
) -> Json<MissionResponse> {
    Json(MissionResponse {
        id: String::new(),
        objective: String::new(),
    })
}
