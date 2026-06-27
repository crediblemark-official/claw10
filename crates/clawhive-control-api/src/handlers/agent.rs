use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::state::AppState;

#[derive(Serialize)]
pub struct AgentResponse {
    pub id: String,
    pub name: String,
    pub state: String,
}

pub async fn list_agents(
    State(_state): State<AppState>,
) -> Json<Vec<AgentResponse>> {
    Json(vec![])
}

pub async fn get_agent(
    State(_state): State<AppState>,
    Path(_id): Path<Uuid>,
) -> Json<AgentResponse> {
    Json(AgentResponse {
        id: String::new(),
        name: String::new(),
        state: String::new(),
    })
}

pub async fn pause_agent(
    State(_state): State<AppState>,
    Path(_id): Path<Uuid>,
) -> Json<AgentResponse> {
    Json(AgentResponse {
        id: String::new(),
        name: String::new(),
        state: String::new(),
    })
}

pub async fn terminate_agent(
    State(_state): State<AppState>,
    Path(_id): Path<Uuid>,
) -> Json<AgentResponse> {
    Json(AgentResponse {
        id: String::new(),
        name: String::new(),
        state: String::new(),
    })
}
