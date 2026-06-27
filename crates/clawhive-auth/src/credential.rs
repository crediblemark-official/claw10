use chrono::Utc;
use uuid::Uuid;

use clawhive_domain::{
    Credential, CredentialId, CredentialKind, IdentityId,
};

use crate::error::AuthError;

pub struct CredentialService;

impl CredentialService {
    pub fn issue_credential(
        identity_id: IdentityId,
        kind: CredentialKind,
        scope: String,
        ttl_seconds: i64,
    ) -> Credential {
        let now = Utc::now();
        Credential {
            id: CredentialId(Uuid::now_v7()),
            identity_id,
            kind,
            scope,
            issued_at: now,
            expires_at: now + chrono::Duration::seconds(ttl_seconds),
            revoked_at: None,
        }
    }

    pub fn verify_credential(
        credential: &Credential,
        required_scope: &str,
    ) -> Result<(), AuthError> {
        if credential.revoked_at.is_some() {
            return Err(AuthError::CredentialRevoked);
        }

        if Utc::now() > credential.expires_at {
            return Err(AuthError::CredentialExpired);
        }

        if !credential.scope.contains(required_scope) {
            return Err(AuthError::Unauthorized(format!(
                "credential scope '{}' does not cover '{}'",
                credential.scope, required_scope
            )));
        }

        Ok(())
    }

    pub fn revoke_credential(credential: &mut Credential) {
        credential.revoked_at = Some(Utc::now());
    }
}
