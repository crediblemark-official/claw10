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

    fn create_role_id() -> RoleId {
        RoleId(Uuid::now_v7())
    }

    fn create_permission(name: &str) -> Permission {
        Permission(name.to_string())
    }

    #[test]
    fn test_get_roles_permissions_empty() {
        let rbac = RbacService::new();
        let perms = rbac.get_roles_permissions(&[]);
        assert!(perms.is_empty());
    }

    #[test]
    fn test_get_roles_permissions_single_role() {
        let mut rbac = RbacService::new();
        let role1 = create_role_id();
        let p1 = create_permission("read");
        let p2 = create_permission("write");
        rbac.assign_permissions(role1.clone(), vec![p1.clone(), p2.clone()]);

        let perms = rbac.get_roles_permissions(&[role1]);
        assert_eq!(perms.len(), 2);
        assert!(perms.contains(&p1));
        assert!(perms.contains(&p2));
    }

    #[test]
    fn test_get_roles_permissions_multiple_roles() {
        let mut rbac = RbacService::new();

        let role1 = create_role_id();
        let p1 = create_permission("read");
        rbac.assign_permissions(role1.clone(), vec![p1.clone()]);

        let role2 = create_role_id();
        let p2 = create_permission("write");
        rbac.assign_permissions(role2.clone(), vec![p2.clone()]);

        let perms = rbac.get_roles_permissions(&[role1, role2]);
        assert_eq!(perms.len(), 2);
        assert!(perms.contains(&p1));
        assert!(perms.contains(&p2));
    }

    #[test]
    fn test_get_roles_permissions_duplicate_permissions() {
        let mut rbac = RbacService::new();

        let role1 = create_role_id();
        let p1 = create_permission("read");
        let p2 = create_permission("write");
        rbac.assign_permissions(role1.clone(), vec![p1.clone(), p2.clone()]);

        let role2 = create_role_id();
        let p3 = create_permission("write"); // same as p2
        let p4 = create_permission("delete");
        rbac.assign_permissions(role2.clone(), vec![p3.clone(), p4.clone()]);

        let perms = rbac.get_roles_permissions(&[role1, role2]);
        assert_eq!(perms.len(), 3);
        assert!(perms.contains(&p1));
        assert!(perms.contains(&p2));
        assert!(perms.contains(&p4));
    }

    #[test]
    fn test_get_roles_permissions_missing_roles() {
        let mut rbac = RbacService::new();

        let role1 = create_role_id();
        let p1 = create_permission("read");
        rbac.assign_permissions(role1.clone(), vec![p1.clone()]);

        let role_missing = create_role_id();

        let perms = rbac.get_roles_permissions(&[role1, role_missing]);
        assert_eq!(perms.len(), 1);
        assert!(perms.contains(&p1));
    }
}
