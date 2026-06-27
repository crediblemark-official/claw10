use std::sync::Arc;

use clawhive_auth::identity::IdentityService;
use clawhive_auth::rbac::RbacService;
use clawhive_auth::credential::CredentialService;

#[derive(Clone)]
pub struct AppState {
    pub identity_service: Arc<IdentityService>,
    pub rbac_service: Arc<std::sync::Mutex<RbacService>>,
    pub credential_service: Arc<CredentialService>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            identity_service: Arc::new(IdentityService),
            rbac_service: Arc::new(std::sync::Mutex::new(RbacService::new())),
            credential_service: Arc::new(CredentialService),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
