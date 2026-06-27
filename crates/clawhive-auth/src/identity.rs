use chrono::Utc;
use uuid::Uuid;

use clawhive_domain::{
    AgentId, Identity, IdentityId, IdentityKind, OrganizationId, Permission, RoleId,
};

use crate::error::AuthError;

pub struct IdentityService;

impl IdentityService {
    pub fn create_agent_identity(
        organization_id: &OrganizationId,
        agent_id: &AgentId,
        roles: Vec<RoleId>,
        _permissions: Vec<Permission>,
    ) -> Identity {
        Identity {
            id: IdentityId(Uuid::now_v7()),
            kind: IdentityKind::Agent,
            name: format!("agent-{}", agent_id.0),
            organization_id: organization_id.clone(),
            roles,
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    pub fn verify_permission(
        identity: &Identity,
        required: &Permission,
        permissions: &[Permission],
    ) -> Result<(), AuthError> {
        if !identity.is_active {
            return Err(AuthError::Unauthorized("identity is inactive".into()));
        }

        if !permissions.contains(required) {
            let has: Vec<String> = permissions.iter().map(|p| p.0.clone()).collect();
            return Err(AuthError::InsufficientPermissions {
                required: vec![required.0.clone()],
                has,
            });
        }

        Ok(())
    }

    pub fn verify_permissions(
        identity: &Identity,
        required: &[Permission],
        permissions: &[Permission],
    ) -> Result<(), AuthError> {
        for req in required {
            Self::verify_permission(identity, req, permissions)?;
        }
        Ok(())
    }
}
