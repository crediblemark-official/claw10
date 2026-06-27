use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worker {
    pub id: WorkerId,
    pub name: String,
    pub worker_type: WorkerType,
    pub capabilities: Vec<WorkerCapability>,
    pub state: WorkerState,
    pub heartbeat: Option<WorkerHeartbeat>,
    pub version: String,
    pub is_draining: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerType {
    Local,
    Sandbox,
    Remote,
    Cloud,
    Edge,
    Device,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerCapability {
    pub name: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkerState {
    Online,
    Offline,
    Draining,
    Quarantined,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerHeartbeat {
    pub cpu_percent: f64,
    pub memory_percent: f64,
    pub active_runtimes: u32,
    pub queue_depth: u32,
    pub tool_availability: Vec<String>,
    pub sandbox_healthy: bool,
    pub timestamp: DateTime<Utc>,
}
