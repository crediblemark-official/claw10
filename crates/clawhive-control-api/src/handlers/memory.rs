use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use clawhive_domain::{AgentId, EvidenceId, MemoryId, MemoryType, MemoryStatus, TaskId};
use clawhive_event::ClawHiveEvent;

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct StoreMemoryRequest {
    pub tenant_id: String,
    pub scope: String,
    pub memory_type: MemoryType,
    pub content: String,
    pub source_agent: String,
    pub source_task: String,
    pub evidence_id: Option<String>,
    pub confidence: f64,
    pub classification: String,
}

#[derive(Serialize)]
pub struct MemoryResponse {
    pub id: String,
    pub tenant_id: String,
    pub memory_type: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct UpdateContentRequest {
    pub content: String,
}

#[derive(Deserialize)]
pub struct VerifyRequest {
    pub verifier: String,
}

#[derive(Deserialize)]
pub struct TransitionRequest {
    pub status: String,
}

#[derive(Deserialize)]
pub struct MemoryQueryParams {
    pub tenant_id: Option<String>,
    pub scope: Option<String>,
    pub memory_type: Option<String>,
    pub status: Option<String>,
    pub source_agent: Option<String>,
    pub min_confidence: Option<f64>,
}

/// POST /v1/memories
pub async fn store_memory(
    State(state): State<AppState>,
    Json(body): Json<StoreMemoryRequest>,
) -> Result<(StatusCode, Json<MemoryResponse>), ApiError> {
    let input = clawhive_memory::StoreMemoryInput {
        tenant_id: body.tenant_id,
        scope: body.scope,
        memory_type: body.memory_type,
        content: body.content,
        source_agent: AgentId(
            uuid::Uuid::parse_str(&body.source_agent)
                .map_err(|e| ApiError::Validation(format!("invalid source_agent: {e}")))?,
        ),
        source_task: TaskId(
            uuid::Uuid::parse_str(&body.source_task)
                .map_err(|e| ApiError::Validation(format!("invalid source_task: {e}")))?,
        ),
        evidence_id: body
            .evidence_id
            .map(|s| {
                uuid::Uuid::parse_str(&s)
                    .map(EvidenceId)
                    .map_err(|e| ApiError::Validation(format!("invalid evidence_id: {e}")))
            })
            .transpose()?,
        confidence: body.confidence,
        classification: body.classification,
    };

    let memory = state.memory_service.store(input).await;

    let _ = state.telemetry.record("memory.stored", "success", |e| {
        e.with_agent_id(memory.source.agent_id.0.to_string())
            .with_additional("memory_id".into(), memory.id.0.to_string())
            .with_additional("memory_type".into(), format!("{:?}", memory.memory_type))
    });

    let _ = state.event_bus.publish(ClawHiveEvent::MemoryCandidateSubmitted {
        memory_id: memory.id.0,
        agent_id: memory.source.agent_id.0,
        scope: memory.scope.clone(),
        timestamp: chrono::Utc::now(),
    }).await;

    Ok((
        StatusCode::CREATED,
        Json(MemoryResponse {
            id: memory.id.0.to_string(),
            tenant_id: memory.tenant_id,
            memory_type: format!("{:?}", memory.memory_type),
            status: format!("{:?}", memory.status),
            created_at: memory.created_at.to_rfc3339(),
        }),
    ))
}

/// GET /v1/memories/{id}
pub async fn get_memory(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<MemoryResponse>, ApiError> {
    let memory = state
        .memory_service
        .get(&MemoryId(id))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound(format!("memory {id}")))?;

    Ok(Json(MemoryResponse {
        id: memory.id.0.to_string(),
        tenant_id: memory.tenant_id,
        memory_type: format!("{:?}", memory.memory_type),
        status: format!("{:?}", memory.status),
        created_at: memory.created_at.to_rfc3339(),
    }))
}

/// PUT /v1/memories/{id}
pub async fn update_memory(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<UpdateContentRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .memory_service
        .update_content(&MemoryId(id), body.content)
        .await
        .map_err(|e| ApiError::NotFound(e.to_string()))?;

    let _ = state.telemetry.record("memory.updated", "success", |e| {
        e.with_additional("memory_id".into(), id.to_string())
    });

    Ok(Json(serde_json::json!({ "status": "updated" })))
}

/// DELETE /v1/memories/{id}
pub async fn delete_memory(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .memory_service
        .delete(&MemoryId(id))
        .await
        .map_err(|e| ApiError::NotFound(e.to_string()))?;

    let _ = state.telemetry.record("memory.deleted", "success", |e| {
        e.with_additional("memory_id".into(), id.to_string())
    });

    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

/// POST /v1/memories/{id}/verify
pub async fn verify_memory(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<VerifyRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let verifier = AgentId(
        uuid::Uuid::parse_str(&body.verifier)
            .map_err(|e| ApiError::Validation(format!("invalid verifier: {e}")))?,
    );
    let verifier_id = verifier.0.to_string();

    state
        .memory_service
        .verify(&MemoryId(id), verifier)
        .await
        .map_err(|e| ApiError::NotFound(e.to_string()))?;

    let _ = state.telemetry.record("memory.verified", "success", |e| {
        e.with_additional("memory_id".into(), id.to_string())
            .with_additional("verifier".into(), verifier_id)
    });

    Ok(Json(serde_json::json!({ "status": "verified" })))
}

/// POST /v1/memories/{id}/transition
pub async fn transition_memory(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<TransitionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    use std::str::FromStr;

    let status = clawhive_domain::MemoryStatus::from_str(&body.status)
        .map_err(|e| ApiError::Validation(format!("invalid status: {e}")))?;
    let status_str = format!("{:?}", status);

    state
        .memory_service
        .transition_status(&MemoryId(id), status.clone())
        .await
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let memory = state
        .memory_service
        .get(&MemoryId(id))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound(format!("memory {id}")))?;

    let _ = match status {
        MemoryStatus::Active => state.event_bus.publish(ClawHiveEvent::MemoryActivated {
            memory_id: memory.id.0,
            scope: memory.scope.clone(),
            confidence: memory.confidence,
            timestamp: chrono::Utc::now(),
        }).await,
        MemoryStatus::Rejected => state.event_bus.publish(ClawHiveEvent::MemoryRejected {
            memory_id: memory.id.0,
            reason: "transitioned to rejected".into(),
            timestamp: chrono::Utc::now(),
        }).await,
        _ => Ok(()),
    };

    let _ = state.telemetry.record("memory.transitioned", "success", |e| {
        e.with_additional("memory_id".into(), id.to_string())
            .with_additional("new_status".into(), status_str)
    });

    Ok(Json(serde_json::json!({ "status": "transitioned" })))
}

/// GET /v1/memories
pub async fn query_memories(
    State(state): State<AppState>,
    Query(params): Query<MemoryQueryParams>,
) -> Result<Json<Vec<MemoryResponse>>, ApiError> {
    use std::str::FromStr;

    let filter = clawhive_memory::MemoryQuery {
        tenant_id: params.tenant_id,
        scope: params.scope,
        memory_type: params
            .memory_type
            .and_then(|s| MemoryType::from_str(&s).ok()),
        status: params
            .status
            .and_then(|s| clawhive_domain::MemoryStatus::from_str(&s).ok()),
        source_agent: params
            .source_agent
            .and_then(|s| uuid::Uuid::parse_str(&s).ok())
            .map(AgentId),
        min_confidence: params.min_confidence,
    };

    let memories = state
        .memory_service
        .query(filter)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(
        memories
            .into_iter()
            .map(|m| MemoryResponse {
                id: m.id.0.to_string(),
                tenant_id: m.tenant_id,
                memory_type: format!("{:?}", m.memory_type),
                status: format!("{:?}", m.status),
                created_at: m.created_at.to_rfc3339(),
            })
            .collect(),
    ))
}

/// GET /v1/memories/counts
pub async fn memory_counts(
    State(state): State<AppState>,
) -> Result<Json<std::collections::HashMap<String, usize>>, ApiError> {
    let counts = state
        .memory_service
        .count_by_status()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(counts))
}
