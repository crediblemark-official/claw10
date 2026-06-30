use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use clawhive_domain::{Budget, IdentityId, RiskLevel};
use clawhive_mission::MissionService;
use clawhive_store::StoreExt;

use crate::error::ApiError;
use crate::handlers::audit::{self, build_audit_event};
use crate::state::AppState;
use crate::store::MISSION_PREFIX;

#[derive(Serialize)]
pub struct MissionResponse {
    pub id: String,
    pub objective: String,
    pub owner_id: String,
    pub state: String,
}

#[derive(Deserialize)]
pub struct CreateMissionRequest {
    pub objective: String,
    pub owner_id: Option<String>,
    pub budget: Option<MissionBudget>,
}

#[derive(Deserialize)]
pub struct MissionBudget {
    pub allocated_usd: f64,
    pub soft_limit_usd: Option<f64>,
    pub hard_limit_usd: Option<f64>,
}

pub async fn list_missions(
    State(state): State<AppState>,
) -> Result<Json<Vec<MissionResponse>>, ApiError> {
    let missions = state
        .kv_store
        .scan_prefix::<clawhive_domain::Mission>(MISSION_PREFIX)
        .await?
        .into_iter()
        .map(|(_, m)| MissionResponse {
            id: m.id.0.to_string(),
            objective: m.objective,
            owner_id: m.owner_id.0.to_string(),
            state: format!("{:?}", m.state),
        })
        .collect();

    Ok(Json(missions))
}

pub async fn get_mission(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MissionResponse>, ApiError> {
    let key = format!("{MISSION_PREFIX}{id}");
    let mission = state
        .kv_store
        .get::<clawhive_domain::Mission>(&key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("mission {id}")))?;

    Ok(Json(MissionResponse {
        id: mission.id.0.to_string(),
        objective: mission.objective,
        owner_id: mission.owner_id.0.to_string(),
        state: format!("{:?}", mission.state),
    }))
}

pub async fn create_mission(
    State(state): State<AppState>,
    Json(req): Json<CreateMissionRequest>,
) -> Result<(StatusCode, Json<MissionResponse>), ApiError> {
    let owner_id = IdentityId(
        req.owner_id
            .as_deref()
            .map(|s| Uuid::parse_str(s))
            .transpose()
            .map_err(|e| ApiError::Validation(format!("invalid owner_id: {e}")))?
            .unwrap_or_else(Uuid::nil),
    );

    let budget = match req.budget {
        Some(b) => Budget {
            allocated_usd: b.allocated_usd,
            spent_usd: 0.0,
            soft_limit_usd: b.soft_limit_usd,
            hard_limit_usd: b.hard_limit_usd,
            recurring_monthly_usd: None,
        },
        None => Budget {
            allocated_usd: 100.0,
            spent_usd: 0.0,
            soft_limit_usd: Some(80.0),
            hard_limit_usd: Some(100.0),
            recurring_monthly_usd: None,
        },
    };

    let mission = MissionService::create_mission(owner_id, req.objective, budget, RiskLevel("low".into()));

    let key = format!("{MISSION_PREFIX}{}", mission.id.0);
    state.kv_store.set(&key, &mission).await?;

    let _ = state.telemetry.record("mission.created", "success", |e| {
        e.with_mission_id(mission.id.0.to_string())
    });

    audit::emit_event(
        Arc::clone(&state.audit_service),
        build_audit_event(
            "mission_created",
            None,
            Some(mission.id.0.to_string()),
            None,
            serde_json::json!({"objective": mission.objective}),
        ),
    );

    Ok((
        StatusCode::CREATED,
        Json(MissionResponse {
            id: mission.id.0.to_string(),
            objective: mission.objective,
            owner_id: mission.owner_id.0.to_string(),
            state: format!("{:?}", mission.state),
        }),
    ))
}

pub async fn pause_mission(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MissionResponse>, ApiError> {
    let key = format!("{MISSION_PREFIX}{id}");
    let mut mission = state
        .kv_store
        .get::<clawhive_domain::Mission>(&key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("mission {id}")))?;

    MissionService::pause_mission(&mut mission)
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    state.kv_store.set(&key, &mission).await?;

    Ok(Json(MissionResponse {
        id: mission.id.0.to_string(),
        objective: mission.objective,
        owner_id: mission.owner_id.0.to_string(),
        state: format!("{:?}", mission.state),
    }))
}

pub async fn complete_mission(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MissionResponse>, ApiError> {
    let key = format!("{MISSION_PREFIX}{id}");
    let mut mission = state
        .kv_store
        .get::<clawhive_domain::Mission>(&key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("mission {id}")))?;

    MissionService::complete_mission(&mut mission)
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    state.kv_store.set(&key, &mission).await?;

    audit::emit_event(
        Arc::clone(&state.audit_service),
        build_audit_event(
            "mission_completed",
            None,
            Some(mission.id.0.to_string()),
            None,
            serde_json::Value::Null,
        ),
    );

    Ok(Json(MissionResponse {
        id: mission.id.0.to_string(),
        objective: mission.objective,
        owner_id: mission.owner_id.0.to_string(),
        state: format!("{:?}", mission.state),
    }))
}

pub async fn cancel_mission(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MissionResponse>, ApiError> {
    let key = format!("{MISSION_PREFIX}{id}");
    let mut mission = state
        .kv_store
        .get::<clawhive_domain::Mission>(&key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("mission {id}")))?;

    MissionService::cancel_mission(&mut mission)
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    state.kv_store.set(&key, &mission).await?;

    audit::emit_event(
        Arc::clone(&state.audit_service),
        build_audit_event(
            "mission_cancelled",
            None,
            Some(mission.id.0.to_string()),
            None,
            serde_json::Value::Null,
        ),
    );

    Ok(Json(MissionResponse {
        id: mission.id.0.to_string(),
        objective: mission.objective,
        owner_id: mission.owner_id.0.to_string(),
        state: format!("{:?}", mission.state),
    }))
}
