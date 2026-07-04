use super::*;
use claw10_domain::{AgentId, CredentialKind, IdentityKind, Permission, RoleId, IdentityId};
use uuid::Uuid;

// --- UJI COBA MODUL RBAC ---

#[test]
fn test_rbac_assign_and_get_permissions() {
    let mut rbac = RbacService::new();
    let role_id = RoleId(Uuid::now_v7());
    let permissions = vec![
        Permission("read:file".to_string()),
        Permission("write:file".to_string()),
    ];

    rbac.assign_permissions(role_id.clone(), permissions.clone());

    let retrieved = rbac.get_role_permissions(&role_id);
    assert_eq!(retrieved.len(), 2);
    assert!(retrieved.contains(&Permission("read:file".to_string())));
}

#[test]
fn test_rbac_assign_permissions_overwrite() {
    let mut rbac = RbacService::new();
    let role_id = RoleId(Uuid::now_v7());

    // Initial assignment
    rbac.assign_permissions(role_id.clone(), vec![Permission("read:file".to_string())]);

    // Overwrite assignment
    rbac.assign_permissions(role_id.clone(), vec![Permission("write:file".to_string())]);

    let retrieved = rbac.get_role_permissions(&role_id);
    assert_eq!(retrieved.len(), 1);
    assert_eq!(retrieved[0], Permission("write:file".to_string()));
}

#[test]
fn test_rbac_assign_empty_permissions() {
    let mut rbac = RbacService::new();
    let role_id = RoleId(Uuid::now_v7());

    rbac.assign_permissions(role_id.clone(), vec![]);

    let retrieved = rbac.get_role_permissions(&role_id);
    assert_eq!(retrieved.len(), 0);
}

#[test]
fn test_rbac_unassigned_role_permissions() {
    let rbac = RbacService::new();
    let role_id = RoleId(Uuid::now_v7());

    let retrieved = rbac.get_role_permissions(&role_id);
    assert_eq!(retrieved.len(), 0);
}

#[test]
fn test_rbac_get_roles_permissions_deduplication() {
    let mut rbac = RbacService::new();
    let role_a = RoleId(Uuid::now_v7());
    let role_b = RoleId(Uuid::now_v7());

    // Berikan ijin bertumpang tindih
    rbac.assign_permissions(
        role_a.clone(),
        vec![
            Permission("read:file".to_string()),
            Permission("execute:command".to_string()),
        ],
    );
    rbac.assign_permissions(
        role_b.clone(),
        vec![
            Permission("read:file".to_string()),
            Permission("write:file".to_string()),
        ],
    );

    // Hasil harus di-dedup dan diurutkan
    let combined = rbac.get_roles_permissions(&[role_a, role_b]);
    assert_eq!(combined.len(), 3);
    assert_eq!(combined[0], Permission("execute:command".to_string()));
    assert_eq!(combined[1], Permission("read:file".to_string()));
    assert_eq!(combined[2], Permission("write:file".to_string()));
}

#[test]
fn test_rbac_child_permissions_filtration() {
    let parent_permissions = vec![
        Permission("read:file".to_string()),
        Permission("execute:command".to_string()),
    ];

    let requested = vec![
        Permission("read:file".to_string()),
        Permission("write:file".to_string()), // Parent tidak punya wewenang ini
    ];

    let delegated = RbacService::child_permissions(&parent_permissions, &requested);
    assert_eq!(delegated.len(), 1);
    assert_eq!(delegated[0], Permission("read:file".to_string()));
}

// --- UJI COBA MODUL IDENTITY ---

#[test]
fn test_identity_creation() {
    let agent_id = AgentId(Uuid::now_v7());
    let identity = IdentityService::create_agent_identity(&agent_id);

    assert!(matches!(identity.kind, IdentityKind::Agent));
    assert!(identity.is_active);
    assert!(identity.name.contains(&agent_id.0.to_string()));
}

#[test]
fn test_identity_verify_permission_active_and_sufficient() {
    let agent_id = AgentId(Uuid::now_v7());
    let identity = IdentityService::create_agent_identity(&agent_id);
    let has_permissions = vec![Permission("read:file".to_string())];
    let required = Permission("read:file".to_string());

    let res = IdentityService::verify_permission(&identity, &required, &has_permissions);
    assert!(res.is_ok());
}

#[test]
fn test_identity_verify_permission_inactive() {
    let agent_id = AgentId(Uuid::now_v7());
    let mut identity = IdentityService::create_agent_identity(&agent_id);
    identity.is_active = false; // Set tidak aktif

    let has_permissions = vec![Permission("read:file".to_string())];
    let required = Permission("read:file".to_string());

    let res = IdentityService::verify_permission(&identity, &required, &has_permissions);
    assert!(res.is_err());
    assert!(matches!(res, Err(AuthError::Unauthorized(_))));
}

#[test]
fn test_identity_verify_permission_insufficient() {
    let agent_id = AgentId(Uuid::now_v7());
    let identity = IdentityService::create_agent_identity(&agent_id);
    let has_permissions = vec![Permission("read:file".to_string())];
    let required = Permission("write:file".to_string());

    let res = IdentityService::verify_permission(&identity, &required, &has_permissions);
    assert!(res.is_err());
    if let Err(AuthError::InsufficientPermissions { required: req_err, has }) = res {
        assert_eq!(req_err, vec!["write:file".to_string()]);
        assert_eq!(has, vec!["read:file".to_string()]);
    } else {
        panic!("Diharapkan error InsufficientPermissions");
    }
}

// --- UJI COBA MODUL CREDENTIAL ---

#[test]
fn test_credential_issue_and_verify_success() {
    let identity_id = IdentityId(Uuid::now_v7());
    let credential = CredentialService::issue_credential(
        identity_id,
        CredentialKind::Token,
        "read:file,write:file".to_string(),
        3600, // TTL 1 jam
    );

    assert_eq!(credential.scope, "read:file,write:file");
    
    // Verifikasi scope yang tepat
    let res = CredentialService::verify_credential(&credential, "read:file");
    assert!(res.is_ok());
}

#[test]
fn test_credential_verify_revoked() {
    let identity_id = IdentityId(Uuid::now_v7());
    let mut credential = CredentialService::issue_credential(
        identity_id,
        CredentialKind::Token,
        "read:file".to_string(),
        3600,
    );

    CredentialService::revoke_credential(&mut credential);
    assert!(credential.revoked_at.is_some());

    let res = CredentialService::verify_credential(&credential, "read:file");
    assert!(res.is_err());
    assert!(matches!(res, Err(AuthError::CredentialRevoked)));
}

#[test]
fn test_credential_verify_expired() {
    let identity_id = IdentityId(Uuid::now_v7());
    // Buat kadaluarsa dengan memberikan TTL negatif
    let credential = CredentialService::issue_credential(
        identity_id,
        CredentialKind::Token,
        "read:file".to_string(),
        -10, // Kadaluarsa 10 detik lalu
    );

    let res = CredentialService::verify_credential(&credential, "read:file");
    assert!(res.is_err());
    assert!(matches!(res, Err(AuthError::CredentialExpired)));
}

#[test]
fn test_credential_verify_invalid_scope() {
    let identity_id = IdentityId(Uuid::now_v7());
    let credential = CredentialService::issue_credential(
        identity_id,
        CredentialKind::Token,
        "read:file".to_string(),
        3600,
    );

    // Minta scope "write:file" padahal credential hanya punya "read:file"
    let res = CredentialService::verify_credential(&credential, "write:file");
    assert!(res.is_err());
    assert!(matches!(res, Err(AuthError::Unauthorized(_))));
}
