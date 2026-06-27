use thiserror::Error;

#[derive(Error, Debug)]
pub enum ToolError {
    #[error("tool not found: {0}")]
    ToolNotFound(String),

    #[error("tool execution failed: {0}")]
    ExecutionFailed(String),

    #[error("invalid arguments: {0}")]
    InvalidArguments(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("side effect not allowed: {0}")]
    SideEffectNotAllowed(String),

    #[error("timeout after {0}s")]
    Timeout(u64),

    #[error("budget exceeded for tool call")]
    BudgetExceeded,

    #[error("execution context missing required field: {0}")]
    MissingContext(String),

    #[error("tool error: {0}")]
    Other(String),
}
