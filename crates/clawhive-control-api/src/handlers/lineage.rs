use axum::{
    Json,
    extract::{Path, State},
};
use serde::Serialize;
use uuid::Uuid;

use clawhive_store::StoreExt;

use crate::error::ApiError;
use crate::state::AppState;
use crate::store::LINEAGE_PREFIX;

#[derive(Serialize)]
pub struct LineageResponse {
    pub id: String,
    pub mission_id: String,
    pub root_agent_id: String,
    pub entries: Vec<LineageEntryResponse>,
}

#[derive(Serialize)]
pub struct LineageEntryResponse {
    pub agent_id: String,
    pub parent_agent_id: Option<String>,
    pub role: String,
    pub state: String,
}

pub async fn get_lineage(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<LineageResponse>, ApiError> {
    let key = format!("{LINEAGE_PREFIX}{id}");
    let lineage = state
        .kv_store
        .get::<clawhive_domain::Lineage>(&key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("lineage {id}")))?;

    Ok(Json(LineageResponse {
        id: lineage.id.0.to_string(),
        mission_id: lineage.mission_id.0.to_string(),
        root_agent_id: lineage.root_agent_id.0.to_string(),
        entries: lineage
            .entries
            .into_iter()
            .map(|e| LineageEntryResponse {
                agent_id: e.agent_id.0.to_string(),
                parent_agent_id: e.parent_agent_id.map(|p| p.0.to_string()),
                role: e.role,
                state: e.state,
            })
            .collect(),
    }))
}

pub async fn get_agent_legacy(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<LineageResponse>, ApiError> {
    let all_lineages: Vec<clawhive_domain::Lineage> = state
        .kv_store
        .scan_prefix_unsorted::<clawhive_domain::Lineage>(LINEAGE_PREFIX)
        .await?
        .into_iter()
        .map(|(_, l)| l)
        .collect();

    let lineage = all_lineages
        .into_iter()
        .find(|l| {
            l.root_agent_id.0 == id
                || l.entries.iter().any(|e| e.agent_id.0 == id)
        })
        .ok_or_else(|| ApiError::NotFound(format!("no lineage found for agent {id}")))?;

    Ok(Json(LineageResponse {
        id: lineage.id.0.to_string(),
        mission_id: lineage.mission_id.0.to_string(),
        root_agent_id: lineage.root_agent_id.0.to_string(),
        entries: lineage
            .entries
            .into_iter()
            .map(|e| LineageEntryResponse {
                agent_id: e.agent_id.0.to_string(),
                parent_agent_id: e.parent_agent_id.map(|p| p.0.to_string()),
                role: e.role,
                state: e.state,
            })
            .collect(),
    }))
}
