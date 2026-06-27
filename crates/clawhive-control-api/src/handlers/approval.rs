use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::state::AppState;

#[derive(Serialize)]
pub struct ApprovalResponse {
    pub id: String,
    pub state: String,
}

pub async fn list_approvals(
    State(_state): State<AppState>,
) -> Json<Vec<ApprovalResponse>> {
    Json(vec![])
}

pub async fn approve_request(
    State(_state): State<AppState>,
    Path(_id): Path<Uuid>,
) -> Json<ApprovalResponse> {
    Json(ApprovalResponse {
        id: String::new(),
        state: String::new(),
    })
}

pub async fn deny_request(
    State(_state): State<AppState>,
    Path(_id): Path<Uuid>,
) -> Json<ApprovalResponse> {
    Json(ApprovalResponse {
        id: String::new(),
        state: String::new(),
    })
}
