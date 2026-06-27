use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::budget::Budget;
use crate::tenant::TenantId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: OrganizationId,
    pub tenant_id: TenantId,
    pub name: String,
    pub mission_statement: Option<String>,
    pub departments: Vec<Department>,
    pub budget: Budget,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Department {
    pub id: DepartmentId,
    pub organization_id: OrganizationId,
    pub name: String,
    pub parent_department_id: Option<DepartmentId>,
    pub roles: Vec<Role>,
    pub budget: Budget,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepartmentId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub id: RoleId,
    pub name: String,
    pub permissions: Vec<Permission>,
    pub parent_role_id: Option<RoleId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct RoleId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct Permission(pub String);
