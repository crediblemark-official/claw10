use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use clawhive_domain::{AgentId, ArtifactId, TaskId};
use clawhive_store::StoreExt;

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Serialize)]
pub struct ArtifactResponse {
    pub id: String,
    pub task_id: String,
    pub agent_id: String,
    pub name: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub content_hash: String,
}

#[derive(Deserialize)]
pub struct CreateArtifactRequest {
    pub task_id: String,
    pub agent_id: String,
    pub name: String,
    pub mime_type: String,
    pub content_base64: String,
}

#[derive(Deserialize)]
pub struct ArtifactsQuery {
    pub task_id: Option<String>,
    pub agent_id: Option<String>,
}

fn to_response(artifact: &clawhive_domain::Artifact) -> ArtifactResponse {
    ArtifactResponse {
        id: artifact.id.0.to_string(),
        task_id: artifact.task_id.0.to_string(),
        agent_id: artifact.agent_id.0.to_string(),
        name: artifact.name.clone(),
        mime_type: artifact.mime_type.clone(),
        size_bytes: artifact.size_bytes,
        content_hash: artifact.content_hash.clone(),
    }
}

/// POST /v1/artifacts
pub async fn store_artifact(
    State(state): State<AppState>,
    Json(req): Json<CreateArtifactRequest>,
) -> Result<(StatusCode, Json<ArtifactResponse>), ApiError> {
    let task_id = TaskId(
        Uuid::parse_str(&req.task_id)
            .map_err(|e| ApiError::Validation(format!("invalid task_id: {e}")))?,
    );
    let agent_id = AgentId(
        Uuid::parse_str(&req.agent_id)
            .map_err(|e| ApiError::Validation(format!("invalid agent_id: {e}")))?,
    );
    let content = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &req.content_base64)
        .map_err(|e| ApiError::Validation(format!("invalid base64 content: {e}")))?;

    let artifact = state
        .artifact_service
        .store_artifact(task_id, agent_id, req.name, req.mime_type, content)
        .await?;

    Ok((StatusCode::CREATED, Json(to_response(&artifact))))
}

/// GET /v1/artifacts
pub async fn list_artifacts(
    State(state): State<AppState>,
    Query(query): Query<ArtifactsQuery>,
) -> Result<Json<Vec<ArtifactResponse>>, ApiError> {
    let artifacts = if let Some(task_id) = query.task_id {
        let task_id = TaskId(
            Uuid::parse_str(&task_id)
                .map_err(|e| ApiError::Validation(format!("invalid task_id: {e}")))?,
        );
        state.artifact_service.list_by_task(&task_id).await?
    } else if let Some(agent_id) = query.agent_id {
        let agent_id = AgentId(
            Uuid::parse_str(&agent_id)
                .map_err(|e| ApiError::Validation(format!("invalid agent_id: {e}")))?,
        );
        state.artifact_service.list_by_agent(&agent_id).await?
    } else {
        // No filter: scan all artifact metadata.
        let all: Vec<(String, clawhive_domain::Artifact)> = state
            .kv_store
            .scan_prefix("artifact:meta:")
            .await?;
        all.into_iter().map(|(_, a)| a).collect()
    };

    Ok(Json(artifacts.iter().map(to_response).collect()))
}

/// GET /v1/artifacts/{id}
pub async fn get_artifact(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ArtifactResponse>, ApiError> {
    let artifact_id = ArtifactId(
        Uuid::parse_str(&id).map_err(|e| ApiError::Validation(format!("invalid artifact id: {e}")))?,
    );
    let artifact = state
        .artifact_service
        .get_artifact(&artifact_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("artifact {id}")))?;

    Ok(Json(to_response(&artifact)))
}

/// GET /v1/artifacts/{id}/content
pub async fn get_artifact_content(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let artifact_id = ArtifactId(
        Uuid::parse_str(&id).map_err(|e| ApiError::Validation(format!("invalid artifact id: {e}")))?,
    );
    let content = state
        .artifact_service
        .get_content(&artifact_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("artifact content {id}")))?;

    Ok(content.into_response())
}

/// DELETE /v1/artifacts/{id}
pub async fn delete_artifact(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let artifact_id = ArtifactId(
        Uuid::parse_str(&id).map_err(|e| ApiError::Validation(format!("invalid artifact id: {e}")))?,
    );
    state.artifact_service.delete_artifact(&artifact_id).await?;

    Ok(Json(serde_json::json!({ "deleted": id })))
}
