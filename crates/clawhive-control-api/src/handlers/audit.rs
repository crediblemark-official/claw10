use std::sync::Arc;

use axum::{
    Json,
    extract::{Query, State},
};
use serde::{Deserialize, Serialize};

use clawhive_domain::{AuditEvent, AuditEventId};

use crate::error::ApiError;
use crate::state::AppState;

/// Emit an audit event fire-and-forget so failures do not break operations.
pub fn emit_event(audit_service: Arc<clawhive_audit::AuditService>, event: AuditEvent) {
    tokio::spawn(async move {
        let _ = audit_service.emit_event(event).await;
    });
}

#[derive(Serialize)]
pub struct AuditEventResponse {
    pub id: String,
    pub event_type: String,
    pub agent_id: Option<String>,
    pub mission_id: Option<String>,
    pub task_id: Option<String>,
    pub status: String,
    pub timestamp: String,
}

#[derive(Deserialize)]
pub struct AuditQuery {
    pub agent_id: Option<String>,
    pub mission_id: Option<String>,
    pub task_id: Option<String>,
    pub event_type: Option<String>,
}

fn to_response(event: &AuditEvent) -> AuditEventResponse {
    AuditEventResponse {
        id: event.id.0.to_string(),
        event_type: event.event_type.clone(),
        agent_id: event.agent_id.clone(),
        mission_id: event.mission_id.clone(),
        task_id: event.task_id.clone(),
        status: event.status.clone(),
        timestamp: event.timestamp.to_rfc3339(),
    }
}

/// Helper to build a minimal AuditEvent.
pub fn build_audit_event(
    event_type: &str,
    agent_id: Option<String>,
    mission_id: Option<String>,
    task_id: Option<String>,
    details: serde_json::Value,
) -> AuditEvent {
    AuditEvent {
        id: AuditEventId(uuid::Uuid::now_v7()),
        tenant_id: "default".to_string(),
        mission_id,
        task_id,
        agent_id,
        parent_agent_id: None,
        lineage_id: None,
        worker_id: None,
        trace_id: None,
        event_type: event_type.to_string(),
        lifecycle_mode: None,
        risk_level: None,
        status: "recorded".to_string(),
        cost_usd: 0.0,
        payload: details,
        timestamp: chrono::Utc::now(),
    }
}

/// GET /v1/audit
pub async fn list_audit_events(
    State(state): State<AppState>,
    Query(query): Query<AuditQuery>,
) -> Result<Json<Vec<AuditEventResponse>>, ApiError> {
    let mut events = state.audit_service.list_all().await?;

    if let Some(agent_id) = query.agent_id {
        events.retain(|e| e.agent_id.as_deref() == Some(&agent_id));
    }
    if let Some(mission_id) = query.mission_id {
        events.retain(|e| e.mission_id.as_deref() == Some(&mission_id));
    }
    if let Some(task_id) = query.task_id {
        events.retain(|e| e.task_id.as_deref() == Some(&task_id));
    }
    if let Some(event_type) = query.event_type {
        events.retain(|e| e.event_type == event_type);
    }

    Ok(Json(events.iter().map(to_response).collect()))
}
