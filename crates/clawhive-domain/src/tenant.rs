use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub id: TenantId,
    pub name: String,
    pub slug: String,
    pub settings: TenantSettings,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantSettings {
    pub max_organizations: u32,
    pub max_active_agents: u32,
    pub max_persistent_agents: u32,
    pub default_budget_usd: f64,
}
