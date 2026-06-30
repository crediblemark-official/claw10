use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

use clawhive_store::StoreError;

#[derive(Debug)]
pub enum ApiError {
    NotFound(String),
    Validation(String),
    Unauthorized(String),
    PolicyDenied(String),
    Conflict(String),
    SpawnFailed(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::Validation(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::Unauthorized(msg) => (StatusCode::FORBIDDEN, msg),
            ApiError::PolicyDenied(msg) => (StatusCode::FORBIDDEN, msg),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, msg),
            ApiError::SpawnFailed(msg) => {
                tracing::error!("Spawn failed: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
            ApiError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error".into())
            }
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

impl From<clawhive_domain::DomainError> for ApiError {
    fn from(e: clawhive_domain::DomainError) -> Self {
        match e {
            clawhive_domain::DomainError::NotFound(m) => ApiError::NotFound(m),
            clawhive_domain::DomainError::Validation(m) => ApiError::Validation(m),
            clawhive_domain::DomainError::PolicyDenied(m) => ApiError::PolicyDenied(m),
            clawhive_domain::DomainError::BudgetExhausted(m) => ApiError::Validation(m),
            clawhive_domain::DomainError::Conflict(m) => ApiError::Conflict(m),
            clawhive_domain::DomainError::Unauthorized(m) => ApiError::Unauthorized(m),
            _ => ApiError::Internal(e.to_string()),
        }
    }
}

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::NotFound(m) => ApiError::NotFound(m),
            StoreError::Database(m) => ApiError::Internal(m),
            StoreError::Serialization(m) => ApiError::Internal(m),
        }
    }
}

impl From<clawhive_skill::SkillError> for ApiError {
    fn from(e: clawhive_skill::SkillError) -> Self {
        match e {
            clawhive_skill::SkillError::NotFound(m) => ApiError::NotFound(m),
            clawhive_skill::SkillError::InvalidTransition { from, to } => {
                ApiError::Validation(format!("invalid transition: {from:?} -> {to:?}"))
            }
            clawhive_skill::SkillError::Unsigned => {
                ApiError::Validation("skill must be signed before activation".into())
            }
            clawhive_skill::SkillError::Store(se) => se.into(),
        }
    }
}

impl From<clawhive_artifact::ArtifactError> for ApiError {
    fn from(e: clawhive_artifact::ArtifactError) -> Self {
        match e {
            clawhive_artifact::ArtifactError::NotFound(m) => ApiError::NotFound(m),
            clawhive_artifact::ArtifactError::ContentHashMismatch(m) => {
                ApiError::Validation(format!("content hash mismatch: {m}"))
            }
            clawhive_artifact::ArtifactError::Store(se) => se.into(),
        }
    }
}

impl From<clawhive_audit::AuditError> for ApiError {
    fn from(e: clawhive_audit::AuditError) -> Self {
        ApiError::Internal(e.to_string())
    }
}

