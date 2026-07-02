use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use claw10_agent::{AgentError, AgentRuntime, AgentStore};
use claw10_domain::{AgentId, ChannelType, IdentityId, WorkerId};

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct RegisterChannelRequest {
    pub channel_type: ChannelType,
    pub config: serde_json::Value,
}

#[derive(Serialize)]
pub struct ChannelResponse {
    pub id: String,
    pub channel_type: String,
    pub is_active: bool,
}

#[derive(Deserialize)]
pub struct CreateSessionRequest {
    pub identity_id: String,
    pub channel_id: String,
    pub ttl_seconds: i64,
}

#[derive(Serialize)]
pub struct SessionResponse {
    pub id: String,
    pub identity_id: String,
    pub channel_id: String,
    pub state: String,
    pub expires_at: String,
}

#[derive(Deserialize)]
pub struct DispatchRequest {
    pub recipient: String,
    pub subject: Option<String>,
    pub body: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct DispatchResponse {
    pub channel_id: String,
    pub success: bool,
    pub response: Option<String>,
    pub dispatched_at: String,
}

#[derive(Deserialize)]
pub struct ChannelsQuery {
    pub channel_type: Option<String>,
}

// ── Channel Endpoints ──────────────────────────────────────────────

/// POST /v1/gateway/channels
pub async fn register_channel(
    State(state): State<AppState>,
    Json(body): Json<RegisterChannelRequest>,
) -> Result<(StatusCode, Json<ChannelResponse>), ApiError> {
    let channel = state
        .gateway_service
        .register_channel(body.channel_type, body.config)
        .await;

    let _ = state.telemetry.record("gateway.channel_registered", "success", |e| {
        e.with_additional("channel_id".into(), channel.id.clone())
            .with_additional("channel_type".into(), format!("{:?}", channel.channel_type))
    });

    Ok((
        StatusCode::CREATED,
        Json(ChannelResponse {
            id: channel.id,
            channel_type: format!("{:?}", channel.channel_type),
            is_active: channel.is_active,
        }),
    ))
}

/// GET /v1/gateway/channels
pub async fn list_channels(
    State(state): State<AppState>,
) -> Result<Json<Vec<ChannelResponse>>, ApiError> {
    let channels = state
        .gateway_service
        .list_channels(None)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(
        channels
            .into_iter()
            .map(|c| ChannelResponse {
                id: c.id,
                channel_type: format!("{:?}", c.channel_type),
                is_active: c.is_active,
            })
            .collect(),
    ))
}

/// GET /v1/gateway/channels/{id}
pub async fn get_channel(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ChannelResponse>, ApiError> {
    let channel = state
        .gateway_service
        .get_channel(&id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound(format!("channel {id}")))?;

    Ok(Json(ChannelResponse {
        id: channel.id,
        channel_type: format!("{:?}", channel.channel_type),
        is_active: channel.is_active,
    }))
}

/// POST /v1/gateway/channels/{id}/activate
pub async fn activate_channel(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .gateway_service
        .activate_channel(&id)
        .await
        .map_err(|e| ApiError::NotFound(e.to_string()))?;

    let _ = state.telemetry.record("gateway.channel_activated", "success", |e| {
        e.with_additional("channel_id".into(), id.clone())
    });

    Ok(Json(serde_json::json!({ "status": "activated" })))
}

/// POST /v1/gateway/channels/{id}/deactivate
pub async fn deactivate_channel(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .gateway_service
        .deactivate_channel(&id)
        .await
        .map_err(|e| ApiError::NotFound(e.to_string()))?;

    let _ = state.telemetry.record("gateway.channel_deactivated", "success", |e| {
        e.with_additional("channel_id".into(), id.clone())
    });

    Ok(Json(serde_json::json!({ "status": "deactivated" })))
}

/// POST /v1/gateway/channels/{id}/dispatch
pub async fn dispatch_message(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<DispatchRequest>,
) -> Result<Json<DispatchResponse>, ApiError> {
    let recipient = body.recipient.clone();
    let message = claw10_gateway::Message {
        recipient: body.recipient,
        subject: body.subject,
        body: body.body,
        metadata: body.metadata,
    };

    let result = state
        .gateway_service
        .dispatch(&id, &message)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("not found") {
                ApiError::NotFound(msg)
            } else {
                ApiError::Validation(msg)
            }
        })?;

    let _ = state.telemetry.record("gateway.message_dispatched", if result.success { "success" } else { "failed" }, |e| {
        e.with_additional("channel_id".into(), result.channel_id.clone())
            .with_additional("recipient".into(), recipient)
    });

    Ok(Json(DispatchResponse {
        channel_id: result.channel_id,
        success: result.success,
        response: result.response,
        dispatched_at: result.dispatched_at.to_rfc3339(),
    }))
}

/// POST /v1/gateway/webhooks/{channel_id}
///
/// Receive an incoming webhook from an external service (Telegram, Discord, etc.).
/// The body and query parameters are forwarded to the gateway service for parsing.
pub async fn handle_webhook(
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
    headers: axum::http::HeaderMap,
    query: axum::extract::Query<HashMap<String, String>>,
    body: Option<axum::extract::Json<serde_json::Value>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // WhatsApp verification GET request
    if let Some(mode) = query.0.get("hub.mode")
        && mode == "subscribe" {
            let channel = state
                .gateway_service
                .get_channel(&channel_id)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?
                .ok_or_else(|| ApiError::NotFound(format!("channel {channel_id}")))?;

            let expected_token = channel
                .config
                .get("verify_token")
                .and_then(|v| v.as_str())
                .unwrap_or("claw10");
            let challenge = query.0.get("hub.challenge").map(|s| s.as_str()).unwrap_or("0");

            if query.0.get("hub.verify_token").map(|s| s.as_str()) == Some(expected_token) {
                return Ok(Json(serde_json::json!({"challenge": challenge})));
            }
            return Err(ApiError::Validation("invalid verify_token".into()));
        }

    let payload = body.map(|j| j.0).unwrap_or(serde_json::Value::Null);

    let header_map: std::collections::HashMap<String, String> = headers
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_lowercase(),
                v.to_str().unwrap_or_default().to_string(),
            )
        })
        .collect();

    let result = state
        .gateway_service
        .receive(&channel_id, &payload, &header_map)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("not found") {
                ApiError::NotFound(msg)
            } else {
                ApiError::Validation(msg)
            }
        })?;

    let _ = state.telemetry.record(
        "gateway.webhook_received",
        "success",
        |e| {
            e.with_additional("channel_id".into(), channel_id.clone())
                .with_additional("sender_id".into(), result.message.sender_id.clone())
        },
    );

    // Return the channel-specific response (e.g. Slack challenge) immediately.
    if let Some(resp) = result.response {
        return Ok(Json(resp));
    }

    // Route to an agent if the channel is configured with an `agent_id`.
    let channel = state
        .gateway_service
        .get_channel(&channel_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound(format!("channel {channel_id}")))?;

    if let Some(agent_id_str) = channel.config.get("agent_id").and_then(|v| v.as_str())
        && let Ok(agent_uuid) = agent_id_str.parse::<uuid::Uuid>() {
            let agent_state = state.clone();
            let sender_id = result.message.sender_id.clone();
            let objective = result.message.text.clone();
            let channel_id_for_reply = channel_id.clone();

            tokio::spawn(async move {
                if let Err(e) = run_agent_and_reply(
                    agent_state,
                    AgentId(agent_uuid),
                    objective,
                    sender_id,
                    channel_id_for_reply,
                )
                .await
                {
                    tracing::warn!(
                        "failed to run agent {agent_uuid} for channel {channel_id}: {e:?}"
                    );
                }
            });
        }

    Ok(Json(serde_json::json!({"status": "ok"})))
}

async fn run_agent_and_reply(
    state: AppState,
    agent_id: AgentId,
    objective: String,
    recipient: String,
    channel_id: String,
) -> Result<(), ApiError> {
    let model_router = state
        .model_router
        .clone()
        .ok_or_else(|| ApiError::Internal("model router not configured".into()))?;
    let tool_registry = state
        .tool_registry
        .clone()
        .ok_or_else(|| ApiError::Internal("tool registry not configured".into()))?;

    let agent_store = AgentStore::new(Arc::clone(&state.kv_store));
    let budget_service = Arc::new(claw10_budget::BudgetService);

    let runtime = AgentRuntime::new(
        agent_store,
        model_router,
        tool_registry,
        budget_service,
        Arc::clone(&state.worker_service),
        Some(WorkerId(uuid::Uuid::now_v7())),
    );

    let (session, _events) = runtime
        .execute_agent(&agent_id, objective, HashMap::new(), None, None)
        .await

        .map_err(|e| match &e {
            AgentError::AgentNotFound(_) => ApiError::NotFound(format!("agent {}", agent_id.0)),
            AgentError::BudgetExhausted => ApiError::Validation("budget exhausted".into()),
            _ => ApiError::Internal(e.to_string()),
        })?;

    // Extract the last assistant message as the reply text.
    let reply_text = session
        .messages
        .iter()
        .rev()
        .find(|m| matches!(m.role, claw10_model_router::types::MessageRole::Assistant))
        .map(|m| m.content.clone())
        .unwrap_or_else(|| "(no response)".into());

    let message = claw10_gateway::Message {
        recipient,
        subject: None,
        body: reply_text,
        metadata: None,
    };

    state
        .gateway_service
        .dispatch(&channel_id, &message)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(())
}

// ── Session Endpoints ──────────────────────────────────────────────

/// POST /v1/gateway/sessions
pub async fn create_session(
    State(state): State<AppState>,
    Json(body): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<SessionResponse>), ApiError> {
    let identity_id = IdentityId(
        uuid::Uuid::parse_str(&body.identity_id)
            .map_err(|e| ApiError::Validation(format!("invalid identity_id: {e}")))?,
    );

    let session = state
        .gateway_service
        .create_session(identity_id, body.channel_id, body.ttl_seconds)
        .await
        .map_err(|e| ApiError::NotFound(e.to_string()))?;

    let _ = state.telemetry.record("gateway.session_created", "success", |e| {
        e.with_additional("session_id".into(), session.id.clone())
            .with_additional("channel_id".into(), session.channel_id.clone())
    });

    Ok((
        StatusCode::CREATED,
        Json(SessionResponse {
            id: session.id,
            identity_id: session.identity_id.0.to_string(),
            channel_id: session.channel_id,
            state: format!("{:?}", session.state),
            expires_at: session.expires_at.to_rfc3339(),
        }),
    ))
}

/// GET /v1/gateway/sessions/{id}
pub async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<SessionResponse>, ApiError> {
    let session = state
        .gateway_service
        .get_session(&id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound(format!("session {id}")))?;

    Ok(Json(SessionResponse {
        id: session.id,
        identity_id: session.identity_id.0.to_string(),
        channel_id: session.channel_id,
        state: format!("{:?}", session.state),
        expires_at: session.expires_at.to_rfc3339(),
    }))
}

/// POST /v1/gateway/sessions/{id}/terminate
pub async fn terminate_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .gateway_service
        .terminate_session(&id)
        .await
        .map_err(|e| ApiError::NotFound(e.to_string()))?;

    let _ = state.telemetry.record("gateway.session_terminated", "success", |e| {
        e.with_additional("session_id".into(), id.clone())
    });

    Ok(Json(serde_json::json!({ "status": "terminated" })))
}

/// GET /v1/identities/{identity_id}/sessions
pub async fn list_identity_sessions(
    State(state): State<AppState>,
    Path(identity_id): Path<uuid::Uuid>,
) -> Result<Json<Vec<SessionResponse>>, ApiError> {
    let sessions = state
        .gateway_service
        .list_sessions(&IdentityId(identity_id))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(
        sessions
            .into_iter()
            .map(|s| SessionResponse {
                id: s.id,
                identity_id: s.identity_id.0.to_string(),
                channel_id: s.channel_id,
                state: format!("{:?}", s.state),
                expires_at: s.expires_at.to_rfc3339(),
            })
            .collect(),
    ))
}
