//! Persistensi agent ke/dari `Arc<dyn Store>`.
//!
//! Setiap agent disimpan dengan key `agent:<uuid>`.
//! Prefix scan digunakan untuk list dan query agent per mission.

use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use clawhive_domain::{Agent, AgentId, AgentState, LifecycleMode, MissionId};
use clawhive_store::{Store, StoreError, StoreExt};

#[derive(Debug, thiserror::Error)]
pub enum AgentStoreError {
    #[error("agent tidak ditemukan: {0}")]
    NotFound(String),

    #[error("store error: {0}")]
    Store(#[from] StoreError),
}

const KEY_PREFIX: &str = "agent:";

/// Filter untuk query agent.
#[derive(Debug, Default)]
pub struct AgentQuery {
    pub mission_id: Option<MissionId>,
    pub state: Option<AgentState>,
    pub lifecycle_mode: Option<LifecycleMode>,
    pub parent_agent_id: Option<AgentId>,
}

/// Service persistensi agent.
pub struct AgentStore {
    store: Arc<dyn Store>,
}

impl AgentStore {
    #[must_use]
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }

    fn key(id: &AgentId) -> String {
        format!("{KEY_PREFIX}{}", id.0)
    }

    /// Simpan atau update agent.
    pub async fn save(&self, agent: &Agent) -> Result<(), AgentStoreError> {
        self.store.set(&Self::key(&agent.id), agent).await?;
        Ok(())
    }

    /// Ambil agent by ID.
    pub async fn get(&self, id: &AgentId) -> Result<Option<Agent>, AgentStoreError> {
        Ok(self.store.get::<Agent>(&Self::key(id)).await?)
    }

    /// Ambil agent by ID, error jika tidak ada.
    pub async fn get_or_not_found(&self, id: &AgentId) -> Result<Agent, AgentStoreError> {
        self.get(id)
            .await?
            .ok_or_else(|| AgentStoreError::NotFound(id.0.to_string()))
    }

    /// Hapus agent dari store.
    pub async fn delete(&self, id: &AgentId) -> Result<(), AgentStoreError> {
        self.store.delete(&Self::key(id)).await?;
        Ok(())
    }

    /// List semua agent, dengan optional filter.
    pub async fn list(&self, query: AgentQuery) -> Result<Vec<Agent>, AgentStoreError> {
        let all: Vec<(String, Agent)> = self.store.scan_prefix(KEY_PREFIX).await?;

        let agents = all
            .into_iter()
            .map(|(_, a)| a)
            .filter(|a| {
                if let Some(ref mission_id) = query.mission_id {
                    if a.mission_id != *mission_id {
                        return false;
                    }
                }
                if let Some(ref state) = query.state {
                    if a.state != *state {
                        return false;
                    }
                }
                if let Some(ref mode) = query.lifecycle_mode {
                    if a.lifecycle_mode != *mode {
                        return false;
                    }
                }
                if let Some(ref parent_id) = query.parent_agent_id {
                    match &a.parent_agent_id {
                        Some(pid) => {
                            if pid != parent_id {
                                return false;
                            }
                        }
                        None => return false,
                    }
                }
                true
            })
            .collect();

        Ok(agents)
    }

    /// Simpan batch agents sekaligus (digunakan setelah spawn).
    pub async fn save_many(&self, agents: &[Agent]) -> Result<(), AgentStoreError> {
        for agent in agents {
            self.save(agent).await?;
        }
        Ok(())
    }

    /// Update state agent tanpa load penuh (read-modify-write).
    pub async fn update_state(
        &self,
        id: &AgentId,
        new_state: AgentState,
    ) -> Result<(), AgentStoreError> {
        let mut agent = self.get_or_not_found(id).await?;
        agent.state = new_state;
        agent.updated_at = Utc::now();
        self.save(&agent).await
    }

    /// Buat AgentId baru (UUID v7).
    #[must_use]
    pub fn new_id() -> AgentId {
        AgentId(Uuid::now_v7())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clawhive_domain::{
        AgentGenome, AgentState, AutonomyConfig, Budget, LifecycleMode, LineageId, MemoryConfig,
        MissionId, ModelPolicy, NetworkPolicy, OrganizationId, PolicyBundle, PolicyBundleId,
        RuntimeConfig,
    };
    use clawhive_store::InMemoryStore;

    fn agent_fixture() -> Agent {
        use clawhive_domain::IdentityId;
        let now = Utc::now();
        Agent {
            id: AgentId(Uuid::now_v7()),
            identity_id: IdentityId(Uuid::now_v7()),
            organization_id: OrganizationId(Uuid::now_v7()),
            mission_id: MissionId(Uuid::now_v7()),
            parent_agent_id: None,
            lineage_id: LineageId(Uuid::now_v7()),
            name: "test-agent".into(),
            role: "specialist".into(),
            genome: AgentGenome {
                id: "genome-test".into(),
                version: "1.0.0".into(),
                role: "specialist".into(),
                lifecycle_modes: vec![LifecycleMode::Ephemeral],
                model_policy: ModelPolicy {
                    preferred_profile: "gpt-4".into(),
                    fallback_profiles: vec![],
                    max_context_tokens: 8192,
                },
                autonomy: AutonomyConfig {
                    can_spawn: false,
                    max_spawn_depth: 0,
                    max_children: 0,
                },
                delegable_permissions: vec![],
                non_delegable_permissions: vec![],
                memory: MemoryConfig {
                    default_read_scopes: vec![],
                    default_write_scope: None,
                },
                runtime: RuntimeConfig {
                    preferred_class: "local".into(),
                    network: NetworkPolicy::DenyByDefault,
                },
                verification_required: false,
            },
            state: AgentState::Ready,
            lifecycle_mode: LifecycleMode::Ephemeral,
            persistent_pattern: None,
            budget: Budget {
                allocated_usd: 10.0,
                spent_usd: 0.0,
                soft_limit_usd: Some(8.0),
                hard_limit_usd: Some(10.0),
                recurring_monthly_usd: None,
            },
            delegable_permissions: vec![],
            non_delegable_permissions: vec![],
            current_runtime: None,
            checkpoints: vec![],
            subscriptions: vec![],
            schedules: vec![],
            policy_bundle: PolicyBundle {
                id: clawhive_domain::PolicyBundleId(Uuid::now_v7()),
                name: "default".into(),
                version: "1.0.0".into(),
                rules: vec![],
                is_active: true,
                signed_by: None,
                signature: None,
                activated_at: None,
                created_at: now,
            },
            turn_count: 0,
            total_cost_usd: 0.0,
            created_at: now,
            updated_at: now,
            terminated_at: None,
        }
    }

    #[tokio::test]
    async fn test_save_and_get() {
        let store = Arc::new(InMemoryStore::new());
        let svc = AgentStore::new(store);
        let agent = agent_fixture();

        svc.save(&agent).await.unwrap();
        let loaded = svc.get(&agent.id).await.unwrap().unwrap();
        assert_eq!(loaded.id, agent.id);
    }

    #[tokio::test]
    async fn test_get_not_found() {
        let store = Arc::new(InMemoryStore::new());
        let svc = AgentStore::new(store);
        let result = svc.get(&AgentId(Uuid::now_v7())).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_update_state() {
        let store = Arc::new(InMemoryStore::new());
        let svc = AgentStore::new(store);
        let agent = agent_fixture();

        svc.save(&agent).await.unwrap();
        svc.update_state(&agent.id, AgentState::Hibernating)
            .await
            .unwrap();

        let loaded = svc.get_or_not_found(&agent.id).await.unwrap();
        assert_eq!(loaded.state, AgentState::Hibernating);
    }

    #[tokio::test]
    async fn test_list_by_state() {
        let store = Arc::new(InMemoryStore::new());
        let svc = AgentStore::new(store);

        let mut a1 = agent_fixture();
        a1.state = AgentState::Active;
        let mut a2 = agent_fixture();
        a2.id = AgentId(Uuid::now_v7());
        a2.state = AgentState::Hibernating;

        svc.save(&a1).await.unwrap();
        svc.save(&a2).await.unwrap();

        let active = svc
            .list(AgentQuery {
                state: Some(AgentState::Active),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, a1.id);
    }

    #[tokio::test]
    async fn test_save_many() {
        let store = Arc::new(InMemoryStore::new());
        let svc = AgentStore::new(store);

        let a1 = agent_fixture();
        let mut a2 = agent_fixture();
        a2.id = AgentId(Uuid::now_v7());

        svc.save_many(&[a1.clone(), a2.clone()]).await.unwrap();

        assert!(svc.get(&a1.id).await.unwrap().is_some());
        assert!(svc.get(&a2.id).await.unwrap().is_some());
    }
}

