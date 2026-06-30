use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;



#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IdentityKind {
    Human,
    Agent,
    Service,
    Worker,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub id: IdentityId,
    pub kind: IdentityKind,
    pub name: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    pub id: CredentialId,
    pub identity_id: IdentityId,
    pub kind: CredentialKind,
    pub scope: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CredentialKind {
    ApiKey,
    Token,
    Session,
    Certificate,
}
