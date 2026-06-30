use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use clawhive_domain::{Permission, SkillCostProfile, SkillId, SkillState};

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Serialize)]
pub struct SkillResponse {
    pub id: String,
    pub name: String,
    pub state: String,
}

#[derive(Deserialize)]
pub struct CreateSkillRequest {
    pub name: String,
    pub purpose: String,
    pub version: String,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    pub steps: Vec<String>,
    pub required_tools: Vec<String>,
    pub required_permissions: Vec<Permission>,
    pub cost_profile: SkillCostProfile,
}

#[derive(Deserialize)]
pub struct SkillsQuery {
    pub state: Option<String>,
}

#[derive(Deserialize)]
pub struct TransitionSkillRequest {
    pub state: SkillState,
}

#[derive(Deserialize)]
pub struct SignSkillRequest {
    pub signature: String,
}

fn to_response(skill: &clawhive_domain::Skill) -> SkillResponse {
    SkillResponse {
        id: skill.id.0.to_string(),
        name: skill.name.clone(),
        state: format!("{:?}", skill.state),
    }
}

/// POST /v1/skills
pub async fn create_skill(
    State(state): State<AppState>,
    Json(req): Json<CreateSkillRequest>,
) -> Result<(StatusCode, Json<SkillResponse>), ApiError> {
    let skill = state
        .skill_service
        .create_skill(
            req.name,
            req.purpose,
            req.version,
            req.input_schema,
            req.output_schema,
            req.steps,
            req.required_tools,
            req.required_permissions,
            req.cost_profile,
        )
        .await?;

    Ok((StatusCode::CREATED, Json(to_response(&skill))))
}

/// GET /v1/skills
pub async fn list_skills(
    State(state): State<AppState>,
    Query(query): Query<SkillsQuery>,
) -> Result<Json<Vec<SkillResponse>>, ApiError> {
    let state_filter = match query.state {
        Some(s) => {
            let state: SkillState = serde_json::from_value(serde_json::Value::String(s))
                .map_err(|e| ApiError::Validation(format!("invalid state: {e}")))?;
            Some(state)
        }
        None => None,
    };

    let skills = state.skill_service.list_skills(state_filter).await?;
    Ok(Json(skills.iter().map(to_response).collect()))
}

/// GET /v1/skills/{id}
pub async fn get_skill(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<SkillResponse>, ApiError> {
    let skill_id = SkillId(
        Uuid::parse_str(&id).map_err(|e| ApiError::Validation(format!("invalid skill id: {e}")))?,
    );
    let skill = state
        .skill_service
        .get_skill(&skill_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("skill {id}")))?;

    Ok(Json(to_response(&skill)))
}

/// POST /v1/skills/{id}/transition
pub async fn transition_skill(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<TransitionSkillRequest>,
) -> Result<Json<SkillResponse>, ApiError> {
    let skill_id = SkillId(
        Uuid::parse_str(&id).map_err(|e| ApiError::Validation(format!("invalid skill id: {e}")))?,
    );
    let skill = state
        .skill_service
        .transition_state(&skill_id, req.state)
        .await?;

    Ok(Json(to_response(&skill)))
}

/// POST /v1/skills/{id}/sign
pub async fn sign_skill(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<SignSkillRequest>,
) -> Result<Json<SkillResponse>, ApiError> {
    let skill_id = SkillId(
        Uuid::parse_str(&id).map_err(|e| ApiError::Validation(format!("invalid skill id: {e}")))?,
    );
    let skill = state.skill_service.sign_skill(&skill_id, req.signature).await?;

    Ok(Json(to_response(&skill)))
}
