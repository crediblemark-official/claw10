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
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_get_role_permissions_existing_role() {
        let mut rbac = RbacService::new();
        let role_id = RoleId(Uuid::now_v7());
        let permissions = vec![
            Permission("read".to_string()),
            Permission("write".to_string()),
        ];

        rbac.assign_permissions(role_id.clone(), permissions.clone());

        let retrieved = rbac.get_role_permissions(&role_id);
        assert_eq!(retrieved, permissions);
    }

    #[test]
    fn test_get_role_permissions_non_existent_role() {
        let rbac = RbacService::new();
        let role_id = RoleId(Uuid::now_v7());

        let retrieved = rbac.get_role_permissions(&role_id);
        assert!(retrieved.is_empty());
    }

    #[test]
    fn test_get_roles_permissions() {
        let mut rbac = RbacService::new();

        let role1 = RoleId(Uuid::now_v7());
        let perms1 = vec![
            Permission("read".to_string()),
            Permission("write".to_string()),
        ];
        rbac.assign_permissions(role1.clone(), perms1);

        let role2 = RoleId(Uuid::now_v7());
        let perms2 = vec![
            Permission("write".to_string()),
            Permission("delete".to_string()),
        ];
        rbac.assign_permissions(role2.clone(), perms2);

        let role_ids = vec![role1, role2, RoleId(Uuid::now_v7())]; // including a non-existent role

        let retrieved = rbac.get_roles_permissions(&role_ids);

        // Expected permissions: "read", "write", "delete"
        // Sorted and deduped
        let expected = vec![
            Permission("delete".to_string()),
            Permission("read".to_string()),
            Permission("write".to_string()),
        ];
        assert_eq!(retrieved, expected);
    }
}
