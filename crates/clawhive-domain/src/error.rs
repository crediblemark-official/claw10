use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("policy denied: {0}")]
    PolicyDenied(String),

    #[error("budget exhausted: {0}")]
    BudgetExhausted(String),

    #[error("spawn depth exceeded: max {max}, current {current}")]
    SpawnDepthExceeded { max: u32, current: u32 },

    #[error("duplicate objective: {0}")]
    DuplicateObjective(String),

    #[error("invalid state transition: from {from:?} to {to:?}")]
    InvalidStateTransition { from: String, to: String },

    #[error("approval required: {0}")]
    ApprovalRequired(String),

    #[error("orphan agent: {0}")]
    Orphan(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("domain error: {0}")]
    Other(String),
}

pub type DomainResult<T> = Result<T, DomainError>;
