use thiserror::Error;

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("identity not found: {0}")]
    IdentityNotFound(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("credential expired")]
    CredentialExpired,

    #[error("credential revoked")]
    CredentialRevoked,

    #[error("insufficient permissions: required {required:?}, has {has:?}")]
    InsufficientPermissions {
        required: Vec<String>,
        has: Vec<String>,
    },

    #[error("permission not delegable: {0}")]
    PermissionNotDelegable(String),

    #[error("session expired")]
    SessionExpired,

    #[error("domain error: {0}")]
    Domain(String),

    #[error("auth error: {0}")]
    Other(String),
}

impl From<clawhive_domain::DomainError> for AuthError {
    fn from(e: clawhive_domain::DomainError) -> Self {
        AuthError::Domain(e.to_string())
    }
}
