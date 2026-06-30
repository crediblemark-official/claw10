use thiserror::Error;

#[derive(Error, Debug)]
pub enum SpawnError {
    #[error("validation failed: {0}")]
    Validation(String),

    #[error("parent not active")]
    ParentNotActive,

    #[error("parent cannot spawn")]
    ParentCannotSpawn,

    #[error("spawn depth exceeded: max {max}, current {current}")]
    DepthExceeded { max: u32, current: u32 },

    #[error("swarm size exceeded")]
    SwarmSizeExceeded,

    #[error("budget insufficient: remaining {remaining}, required {required}")]
    BudgetInsufficient { remaining: f64, required: f64 },

    #[error("permission not delegable: {0}")]
    PermissionNotDelegable(String),

    #[error("duplicate objective: {0}")]
    DuplicateObjective(String),

    #[error("child spawn denied: {0}")]
    ChildSpawnDenied(String),

    #[error("max children exceeded: max {max}, requested {requested}")]
    MaxChildrenExceeded { max: u32, requested: u32 },

    #[error("child spawn depth exceeded: max {max}, current {current}")]
    ChildSpawnDepthExceeded { max: u32, current: u32 },

    #[error("policy denied: {0}")]
    PolicyDenied(String),

    #[error("agent not found: {0}")]
    AgentNotFound(String),

    #[error("mission not found: {0}")]
    MissionNotFound(String),

    #[error("descendant termination failed: {0}")]
    DescendantTerminationFailed(String),

    #[error("spawn error: {0}")]
    Other(String),
}
