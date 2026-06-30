use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Identifies a permission role for agent delegation.
#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct RoleId(pub Uuid);

/// A capability that can be delegated to an agent.
#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct Permission(pub String);
