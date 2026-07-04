use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use claw10_domain::{Approval, ApprovalState, ApprovalTargetType};
use claw10_store::StoreExt;

use crate::error::ApiError;
use crate::state::AppState;
use crate::store::{APPROVAL_PREFIX, TOOL_APPROVAL_PREFIX};

#[derive(Serialize)]
pub struct ApprovalResponse {
    pub id: String,
    pub target_type: String,
    pub target_id: String,
    pub state: String,
}

#[derive(Deserialize)]
pub struct ApprovalActionRequest {
    pub decided_by: Option<String>,
}

pub async fn list_approvals(
    State(state): State<AppState>,
) -> Result<Json<Vec<ApprovalResponse>>, ApiError> {
    let mut responses = Vec::new();

    // 1. Generic Approval records
    let approvals = state
        .kv_store
        .scan_prefix::<Approval>(APPROVAL_PREFIX)
        .await?;
    for (_, a) in approvals {
        responses.push(ApprovalResponse {
            id: a.id.0.to_string(),
            target_type: format!("{:?}", a.target_type),
            target_id: a.target_id,
            state: format!("{:?}", a.state),
        });
    }

    // 2. ToolApprovalRequest records (dibuat oleh agent executor)
    let tool_approvals = state
        .kv_store
        .scan_prefix::<claw10_domain::approval::ToolApprovalRequest>(TOOL_APPROVAL_PREFIX)
        .await?;
    let target_type = format!("{:?}", ApprovalTargetType::ToolInvocation);
    for (_, t) in tool_approvals {
        responses.push(ApprovalResponse {
            id: t.id.clone(),
            target_type: target_type.clone(),
            target_id: t.id,
            state: format!("{:?}", t.state),
        });
    }

    Ok(Json(responses))
}

pub async fn approve_request(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApprovalResponse>, ApiError> {
    let id_str = id.to_string();

    // Coba update generic Approval record
    let approval_key = format!("{APPROVAL_PREFIX}{id_str}");
    if let Ok(Some(mut approval)) = state.kv_store.get::<Approval>(&approval_key).await {
        if !matches!(approval.state, ApprovalState::Pending) {
            return Err(ApiError::Validation(format!(
                "approval {id} is not pending"
            )));
        }
        approval.state = ApprovalState::Approved;
        approval.decided_at = Some(chrono::Utc::now());
        state.kv_store.set(&approval_key, &approval).await?;

        let _ = state.telemetry.record("approval.approved", "success", |e| {
            e.with_additional("approval_id".into(), id.to_string())
                .with_additional("target_type".into(), format!("{:?}", approval.target_type))
        });

        return Ok(Json(ApprovalResponse {
            id: approval.id.0.to_string(),
            target_type: format!("{:?}", approval.target_type),
            target_id: approval.target_id,
            state: "Approved".into(),
        }));
    }

    // Fallback: update ToolApprovalRequest
    let tool_key = format!("{TOOL_APPROVAL_PREFIX}{id_str}");
    let mut tool_approval = state
        .kv_store
        .get::<claw10_domain::approval::ToolApprovalRequest>(&tool_key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("approval {id}")))?;

    if !matches!(
        tool_approval.state,
        claw10_domain::approval::ToolApprovalState::Pending
    ) {
        return Err(ApiError::Validation(format!(
            "approval {id} is not pending"
        )));
    }

    tool_approval.state = claw10_domain::approval::ToolApprovalState::Approved;
    state.kv_store.set(&tool_key, &tool_approval).await?;

    let _ = state.telemetry.record("approval.approved", "success", |e| {
        e.with_additional("approval_id".into(), id.to_string())
            .with_additional("target_type".into(), "ToolInvocation".to_string())
    });

    Ok(Json(ApprovalResponse {
        id: tool_approval.id.clone(),
        target_type: format!("{:?}", ApprovalTargetType::ToolInvocation),
        target_id: tool_approval.id,
        state: "Approved".into(),
    }))
}

pub async fn deny_request(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApprovalResponse>, ApiError> {
    let id_str = id.to_string();

    // Coba update generic Approval record
    let approval_key = format!("{APPROVAL_PREFIX}{id_str}");
    if let Ok(Some(mut approval)) = state.kv_store.get::<Approval>(&approval_key).await {
        if !matches!(approval.state, ApprovalState::Pending) {
            return Err(ApiError::Validation(format!(
                "approval {id} is not pending"
            )));
        }
        approval.state = ApprovalState::Denied;
        approval.decided_at = Some(chrono::Utc::now());
        state.kv_store.set(&approval_key, &approval).await?;

        let _ = state.telemetry.record("approval.denied", "success", |e| {
            e.with_additional("approval_id".into(), id.to_string())
                .with_additional("target_type".into(), format!("{:?}", approval.target_type))
        });

        return Ok(Json(ApprovalResponse {
            id: approval.id.0.to_string(),
            target_type: format!("{:?}", approval.target_type),
            target_id: approval.target_id,
            state: "Denied".into(),
        }));
    }

    // Fallback: update ToolApprovalRequest
    let tool_key = format!("{TOOL_APPROVAL_PREFIX}{id_str}");
    let mut tool_approval = state
        .kv_store
        .get::<claw10_domain::approval::ToolApprovalRequest>(&tool_key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("approval {id}")))?;

    if !matches!(
        tool_approval.state,
        claw10_domain::approval::ToolApprovalState::Pending
    ) {
        return Err(ApiError::Validation(format!(
            "approval {id} is not pending"
        )));
    }

    tool_approval.state = claw10_domain::approval::ToolApprovalState::Denied;
    state.kv_store.set(&tool_key, &tool_approval).await?;

    let _ = state.telemetry.record("approval.denied", "success", |e| {
        e.with_additional("approval_id".into(), id.to_string())
            .with_additional("target_type".into(), "ToolInvocation".to_string())
    });

    Ok(Json(ApprovalResponse {
        id: tool_approval.id.clone(),
        target_type: format!("{:?}", ApprovalTargetType::ToolInvocation),
        target_id: tool_approval.id,
        state: "Denied".into(),
    }))
}
