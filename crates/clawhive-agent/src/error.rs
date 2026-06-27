use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("agent not found: {0}")]
    AgentNotFound(String),

    #[error("session not found: {0}")]
    SessionNotFound(String),

    #[error("model error: {0}")]
    Model(String),

    #[error("tool error: {0}")]
    Tool(String),

    #[error("budget exhausted")]
    BudgetExhausted,

    #[error("max turns reached: {0}")]
    MaxTurnsReached(u32),

    #[error("agent terminated")]
    AgentTerminated,

    #[error("objective not achieved after {0} turns")]
    ObjectiveNotAchieved(u32),

    #[error("policy denied: {0}")]
    PolicyDenied(String),

    #[error("context too long: {0} tokens")]
    ContextTooLong(u32),

    #[error("agent error: {0}")]
    Other(String),
}

impl From<clawhive_model_router::ModelError> for AgentError {
    fn from(e: clawhive_model_router::ModelError) -> Self {
        AgentError::Model(e.to_string())
    }
}

impl From<clawhive_tool::ToolError> for AgentError {
    fn from(e: clawhive_tool::ToolError) -> Self {
        AgentError::Tool(e.to_string())
    }
}

impl From<clawhive_budget::BudgetError> for AgentError {
    fn from(e: clawhive_budget::BudgetError) -> Self {
        match e {
            clawhive_budget::BudgetError::Exhausted { .. } => AgentError::BudgetExhausted,
            clawhive_budget::BudgetError::HardLimitReached => AgentError::BudgetExhausted,
        }
    }
}

impl From<crate::store::AgentStoreError> for AgentError {
    fn from(e: crate::store::AgentStoreError) -> Self {
        match e {
            crate::store::AgentStoreError::NotFound(msg) => AgentError::AgentNotFound(msg),
            crate::store::AgentStoreError::Store(se) => AgentError::Other(se.to_string()),
        }
    }
}

