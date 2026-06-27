use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

#[derive(Serialize)]
pub struct PolicyResponse {
    pub result: String,
}

#[derive(Deserialize)]
pub struct CompilePolicyRequest {
    pub source: String,
}

pub async fn compile_policy(
    State(_state): State<AppState>,
    Json(_req): Json<CompilePolicyRequest>,
) -> Json<PolicyResponse> {
    Json(PolicyResponse {
        result: "compiled".into(),
    })
}

pub async fn simulate_policy(
    State(_state): State<AppState>,
    Json(_req): Json<serde_json::Value>,
) -> Json<PolicyResponse> {
    Json(PolicyResponse {
        result: "simulated".into(),
    })
}
