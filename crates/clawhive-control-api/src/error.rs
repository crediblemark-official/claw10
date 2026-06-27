use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug)]
pub enum ApiError {
    NotFound(String),
    Validation(String),
    Unauthorized(String),
    PolicyDenied(String),
    Conflict(String),
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

impl From<clawhive_auth::AuthError> for ApiError {
    fn from(e: clawhive_auth::AuthError) -> Self {
        match e {
            clawhive_auth::AuthError::IdentityNotFound(m) => ApiError::NotFound(m),
            clawhive_auth::AuthError::Unauthorized(m) => ApiError::Unauthorized(m),
            clawhive_auth::AuthError::CredentialExpired => {
                ApiError::Unauthorized("credential expired".into())
            }
            clawhive_auth::AuthError::CredentialRevoked => {
                ApiError::Unauthorized("credential revoked".into())
            }
            clawhive_auth::AuthError::InsufficientPermissions { .. } => {
                ApiError::Unauthorized("insufficient permissions".into())
            }
            _ => ApiError::Internal(e.to_string()),
        }
    }
}
