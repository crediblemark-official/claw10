use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::state::AppState;

#[derive(Serialize)]
pub struct LineageResponse {
    pub id: String,
    pub entries: Vec<serde_json::Value>,
}

pub async fn get_lineage(
    State(_state): State<AppState>,
    Path(_id): Path<Uuid>,
) -> Json<LineageResponse> {
    Json(LineageResponse {
        id: String::new(),
        entries: vec![],
    })
}

pub async fn get_agent_legacy(
    State(_state): State<AppState>,
    Path(_id): Path<Uuid>,
) -> Json<LineageResponse> {
    Json(LineageResponse {
        id: String::new(),
        entries: vec![],
    })
}
