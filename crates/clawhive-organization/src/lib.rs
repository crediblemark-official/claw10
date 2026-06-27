use chrono::Utc;
use uuid::Uuid;

use clawhive_domain::{
    Department, DepartmentId, Organization, OrganizationId, TenantId,
};

#[derive(Debug, thiserror::Error)]
pub enum OrgError {
    #[error("organization not found: {0}")]
    NotFound(String),
    #[error("validation: {0}")]
    Validation(String),
}

pub struct OrganizationService;

impl OrganizationService {
    pub fn create_organization(
        tenant_id: TenantId,
        name: String,
        mission_statement: Option<String>,
    ) -> Organization {
        let now = Utc::now();
        Organization {
            id: OrganizationId(Uuid::now_v7()),
            tenant_id,
            name,
            mission_statement,
            departments: vec![],
            budget: Default::default(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn add_department(
        org: &mut Organization,
        name: String,
        parent_department_id: Option<DepartmentId>,
    ) -> Department {
        let now = Utc::now();
        let dept = Department {
            id: DepartmentId(Uuid::now_v7()),
            organization_id: org.id.clone(),
            name,
            parent_department_id,
            roles: vec![],
            budget: Default::default(),
            created_at: now,
            updated_at: now,
        };
        org.departments.push(dept.clone());
        dept
    }
}
