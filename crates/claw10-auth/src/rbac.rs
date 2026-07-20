use std::collections::HashMap;

use claw10_domain::{Permission, RoleId};

pub struct RbacService {
    role_permissions: HashMap<RoleId, Vec<Permission>>,
}

impl RbacService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            role_permissions: HashMap::new(),
        }
    }

    pub fn assign_permissions(&mut self, role_id: RoleId, permissions: Vec<Permission>) {
        self.role_permissions.insert(role_id, permissions);
    }

    #[must_use]
    pub fn get_role_permissions(&self, role_id: &RoleId) -> Vec<Permission> {
        self.role_permissions
            .get(role_id)
            .cloned()
            .unwrap_or_default()
    }

    #[must_use]
    pub fn get_roles_permissions(&self, role_ids: &[RoleId]) -> Vec<Permission> {
        let mut perms: Vec<Permission> = role_ids
            .iter()
            .flat_map(|rid| self.get_role_permissions(rid))
            .collect();
        perms.sort();
        perms.dedup();
        perms
    }

    #[must_use]
    pub fn child_permissions(
        parent_delegable: &[Permission],
        requested: &[Permission],
    ) -> Vec<Permission> {
        let parent_set: std::collections::HashSet<&Permission> = parent_delegable.iter().collect();
        requested
            .iter()
            .filter(|p| parent_set.contains(*p))
            .cloned()
            .collect()
    }
}

impl Default for RbacService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "rbac_test.rs"]
mod tests;
