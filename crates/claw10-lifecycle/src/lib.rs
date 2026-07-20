#![allow(clippy::pedantic)]

use chrono::{DateTime, Utc};
use uuid::Uuid;

use claw10_domain::{
    Agent, AgentState, Checkpoint, CheckpointId, CheckpointReason, LifecycleMode,
    PersistentPattern, RuntimeLease,
};

#[derive(Debug, thiserror::Error)]
pub enum LifecycleError {
    #[error("agent not found: {0}")]
    NotFound(String),
    #[error("invalid state transition: {from} → {to}")]
    InvalidTransition { from: AgentState, to: AgentState },
    #[error("checkpoint not found: {0}")]
    CheckpointNotFound(String),
    #[error("agent is not hibernating")]
    NotHibernating,
    #[error("agent is already hibernating")]
    AlreadyHibernating,
    #[error("agent is not persistent")]
    NotPersistent,
    #[error("runtime lease expired")]
    LeaseExpired,
    #[error("migration target unavailable: {0}")]
    MigrationTargetUnavailable(String),
    #[error("heartbeat interval exceeded: {last}")]
    HeartbeatMissed { last: DateTime<Utc> },
    #[error("{0}")]
    Other(String),
}

pub struct LifecycleService;

impl LifecycleService {
    // ── Checkpoint ──────────────────────────────────────────────

    /// Create a snapshot checkpoint of an agent's state.
    #[must_use]
    pub fn create_checkpoint(agent: &Agent, reason: CheckpointReason) -> Checkpoint {
        Checkpoint {
            id: CheckpointId(Uuid::now_v7()),
            agent_id: agent.id.0.to_string(),
            state_snapshot: serde_json::json!({
                "state": agent.state,
                "turn_count": agent.turn_count,
                "total_cost_usd": agent.total_cost_usd,
                "budget": agent.budget,
                "current_runtime": agent.current_runtime,
            }),
            created_at: Utc::now(),
            reason,
        }
    }

    /// Restore agent state from a checkpoint.
    pub fn restore_checkpoint(
        agent: &mut Agent,
        checkpoint: &Checkpoint,
    ) -> Result<(), LifecycleError> {
        let snapshot = &checkpoint.state_snapshot;
        if let Some(state) = snapshot.get("state").and_then(|v| v.as_str()) {
            // Map string back to AgentState
            agent.state = match state {
                "Active" => AgentState::Active,
                "Hibernating" => AgentState::Hibernating,
                "Ready" => AgentState::Ready,
                _ => return Err(LifecycleError::Other(format!("unknown state: {state}"))),
            };
        }
        if let Some(turns) = snapshot.get("turn_count").and_then(|v| v.as_u64()) {
            agent.turn_count = turns;
        }
        agent.updated_at = Utc::now();
        Ok(())
    }

    // ── Hibernation ─────────────────────────────────────────────

    /// Transition an agent to hibernation state.
    /// Creates a pre-hibernation checkpoint and releases the runtime lease.
    pub fn hibernate(agent: &mut Agent) -> Result<Checkpoint, LifecycleError> {
        if agent.state == AgentState::Hibernating {
            return Err(LifecycleError::AlreadyHibernating);
        }
        if !matches!(agent.state, AgentState::Active | AgentState::Ready) {
            return Err(LifecycleError::InvalidTransition {
                from: agent.state.clone(),
                to: AgentState::Hibernating,
            });
        }

        let checkpoint = Self::create_checkpoint(agent, CheckpointReason::PreHibernation);
        agent.state = AgentState::Hibernating;
        agent.current_runtime = None;
        agent.updated_at = Utc::now();
        agent.checkpoints.push(checkpoint.clone());

        // GC old checkpoints to prevent unbounded growth
        Self::gc_checkpoints(agent);

        Ok(checkpoint)
    }

    /// Wake an agent from hibernation using its latest valid checkpoint.
    /// Falls back to earlier checkpoints if the latest one fails restoration.
    pub fn wake(agent: &mut Agent, target_runtime: RuntimeLease) -> Result<(), LifecycleError> {
        if agent.state != AgentState::Hibernating {
            return Err(LifecycleError::NotHibernating);
        }

        // Try checkpoints in reverse order (newest first), skip corrupted ones
        // Clone to avoid borrow conflict with restore_checkpoint(&mut agent, &cp)
        let checkpoints: Vec<Checkpoint> = agent.checkpoints.iter().rev().cloned().collect();
        let restored = checkpoints.iter().any(|cp| {
            if Self::restore_checkpoint(agent, cp).is_err() {
                tracing::warn!(
                    "wake: checkpoint {} restoration failed, trying earlier checkpoint",
                    cp.id.0
                );
                false
            } else {
                true
            }
        });

        if !restored {
            return Err(LifecycleError::Other(
                "no valid checkpoint available for resume — all checkpoints corrupted".into(),
            ));
        }

        agent.state = AgentState::Active;
        agent.current_runtime = Some(target_runtime);
        agent.updated_at = Utc::now();

        Ok(())
    }

    // ── Heartbeat / Liveness ────────────────────────────────────

    /// Process a heartbeat: renew the runtime lease if active.
    /// Returns the remaining TTL for the lease.
    pub fn heartbeat(agent: &mut Agent) -> Result<chrono::Duration, LifecycleError> {
        if agent.state != AgentState::Active {
            return Err(LifecycleError::InvalidTransition {
                from: agent.state.clone(),
                to: AgentState::Active,
            });
        }

        let lease = agent
            .current_runtime
            .as_ref()
            .ok_or_else(|| LifecycleError::Other("no runtime lease".into()))?
            .clone();

        if Utc::now() > lease.expires_at {
            return Err(LifecycleError::LeaseExpired);
        }

        let renewed = Self::renew_lease(&lease);
        agent.current_runtime = Some(renewed);
        agent.updated_at = Utc::now();

        let remaining = lease.expires_at - Utc::now();
        Ok(remaining)
    }

    /// Renew a runtime lease by extending its expiry.
    #[must_use]
    pub fn renew_lease(lease: &RuntimeLease) -> RuntimeLease {
        RuntimeLease {
            worker_id: lease.worker_id.clone(),
            acquired_at: lease.acquired_at,
            expires_at: Utc::now()
                + chrono::Duration::seconds(lease.renewal_interval_seconds as i64),
            renewal_interval_seconds: lease.renewal_interval_seconds,
        }
    }

    /// Detect agents whose lease has expired past the grace period.
    /// Also detects Active agents with no runtime lease (inconsistent state).
    #[must_use]
    pub fn detect_stale(agents: &[Agent], grace_seconds: i64) -> Vec<&Agent> {
        let now = Utc::now();
        agents
            .iter()
            .filter(|a| {
                if a.state != AgentState::Active {
                    return false;
                }
                match &a.current_runtime {
                    Some(lease) => (now - lease.expires_at).num_seconds() > grace_seconds,
                    None => {
                        // Active but no runtime lease = inconsistent state = stale
                        true
                    }
                }
            })
            .collect()
    }

    // ── Migration ───────────────────────────────────────────────

    /// Migrate an agent to a different runtime environment.
    /// Creates a pre-migration checkpoint, transfers state, and assigns a new lease.
    pub fn migrate(
        agent: &mut Agent,
        target_worker_id: &str,
        renewal_interval_seconds: u64,
    ) -> Result<Checkpoint, LifecycleError> {
        if agent.lifecycle_mode != LifecycleMode::Persistent {
            return Err(LifecycleError::NotPersistent);
        }
        if agent.state != AgentState::Active {
            return Err(LifecycleError::InvalidTransition {
                from: agent.state.clone(),
                to: AgentState::Migrating,
            });
        }

        agent.state = AgentState::Migrating;
        let checkpoint = Self::create_checkpoint(agent, CheckpointReason::PreMigration);
        agent.checkpoints.push(checkpoint.clone());

        agent.current_runtime = Some(RuntimeLease {
            worker_id: target_worker_id.into(),
            acquired_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::seconds(renewal_interval_seconds as i64),
            renewal_interval_seconds,
        });
        agent.state = AgentState::Active;
        agent.updated_at = Utc::now();

        // GC old checkpoints to prevent unbounded growth
        Self::gc_checkpoints(agent);

        Ok(checkpoint)
    }

    // ── Lease management ────────────────────────────────────────

    /// Assign an initial runtime lease to an agent.
    pub fn assign_lease(agent: &mut Agent, worker_id: &str, renewal_interval_seconds: u64) {
        agent.current_runtime = Some(RuntimeLease {
            worker_id: worker_id.into(),
            acquired_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::seconds(renewal_interval_seconds as i64),
            renewal_interval_seconds,
        });
    }

    // ── Secure Teardown (AC-06, AC-07) ───────────────────────────

    /// Execute the full terminate flow:
    /// Active/Paused/Hibernating → Completing → PreservingTrace → Terminating → Terminated
    pub fn terminate(agent: &mut Agent) {
        let now = Utc::now();

        // Create final checkpoint
        let cp = Self::create_checkpoint(agent, CheckpointReason::StateTransition);
        agent.checkpoints.push(cp);

        // Transition through teardown stages
        agent.state = AgentState::Completing;
        agent.updated_at = now;

        // PreserveTrace stage
        agent.state = AgentState::PreservingTrace;
        agent.updated_at = Utc::now();

        // Revoke runtime
        agent.current_runtime = None;

        // Terminating stage
        agent.state = AgentState::Terminating;
        agent.updated_at = Utc::now();

        // Final state
        agent.state = AgentState::Terminated;
        agent.terminated_at = Some(Utc::now());
        agent.updated_at = Utc::now();
    }

    /// Terminate a descendant agent (AC-07).
    /// Descendants are frozen, then forced through teardown.
    pub fn terminate_descendant(agent: &mut Agent) {
        let now = Utc::now();

        // First freeze the child
        if matches!(
            agent.state,
            AgentState::Active | AgentState::Hibernating | AgentState::Paused
        ) {
            let cp = Self::create_checkpoint(agent, CheckpointReason::StateTransition);
            agent.checkpoints.push(cp);

            agent.state = AgentState::Completing;
            agent.updated_at = now;
        }

        // Force through remaining stages
        agent.state = AgentState::PreservingTrace;
        agent.updated_at = Utc::now();
        agent.current_runtime = None;
        agent.state = AgentState::Terminating;
        agent.updated_at = Utc::now();
        agent.state = AgentState::Terminated;
        agent.terminated_at = Some(Utc::now());
        agent.updated_at = Utc::now();
    }

    // ── Event-Driven Wake (AC-05) ────────────────────────────────

    /// Check if a hibernating agent has a subscription matching a given event.
    #[must_use]
    pub fn has_subscription(agent: &Agent, event_type: &str) -> bool {
        agent
            .subscriptions
            .iter()
            .any(|s| s.event_type == event_type || s.event_type == "*")
    }

    /// Wake an agent in response to a matching subscription event.
    /// Returns `true` if the agent was woken, `false` if not applicable.
    pub fn wake_for_event(
        agent: &mut Agent,
        event_type: &str,
        target_runtime: RuntimeLease,
    ) -> bool {
        if agent.state != AgentState::Hibernating {
            return false;
        }
        if !Self::has_subscription(agent, event_type) {
            return false;
        }
        // Wake using the standard wake path
        Self::wake(agent, target_runtime).is_ok()
    }

    // ── Persistent Pattern Support ───────────────────────────────

    // ── Checkpoint Garbage Collection ──────────────────────────

    /// Maximum number of checkpoints to retain per agent.
    pub const MAX_CHECKPOINTS_PER_AGENT: usize = 10;

    /// Garbage-collect old checkpoints for an agent, keeping only the most recent ones.
    /// Returns the number of checkpoints removed.
    pub fn gc_checkpoints(agent: &mut Agent) -> usize {
        let before = agent.checkpoints.len();
        if before > Self::MAX_CHECKPOINTS_PER_AGENT {
            let drain_count = before - Self::MAX_CHECKPOINTS_PER_AGENT;
            agent.checkpoints.drain(..drain_count);
            agent.updated_at = Utc::now();
            return drain_count;
        }
        0
    }

    /// Garbage-collect checkpoints older than the given duration.
    /// Returns the number of checkpoints removed.
    pub fn gc_checkpoints_by_age(agent: &mut Agent, max_age: chrono::Duration) -> usize {
        let cutoff = Utc::now() - max_age;
        let before = agent.checkpoints.len();
        agent.checkpoints.retain(|cp| cp.created_at > cutoff);
        let removed = before - agent.checkpoints.len();
        if removed > 0 {
            agent.updated_at = Utc::now();
        }
        removed
    }

    /// Apply the agent's persistent pattern logic.
    /// Returns `true` if the agent should be active, `false` if it should remain hibernating.
    #[must_use]
    pub fn should_be_active(agent: &Agent) -> bool {
        match agent.persistent_pattern {
            Some(ref pattern) => match pattern {
                PersistentPattern::AlwaysOn => true,
                PersistentPattern::Scheduled => {
                    // Check if any schedule indicates the agent should be active now
                    !agent.schedules.is_empty()
                }
                PersistentPattern::Campaign => {
                    // Campaign agent is active until budget exhausted or external termination
                    match agent.budget.hard_limit_usd {
                        Some(limit) => agent.total_cost_usd < limit,
                        None => true,
                    }
                }
            },
            None => true,
        }
    }

    /// Transition the agent according to its pattern.
    pub fn apply_pattern(agent: &mut Agent) {
        if agent.lifecycle_mode != LifecycleMode::Persistent {
            return;
        }
        if agent.state != AgentState::Hibernating && agent.state != AgentState::Active {
            return;
        }

        let should_be_active = Self::should_be_active(agent);

        if should_be_active && agent.state == AgentState::Hibernating {
            // Agent should be active but is hibernating → wake is needed (external trigger)
            // This is a signal; actual wake requires a lease assignment
        } else if !should_be_active && agent.state == AgentState::Active {
            // Agent should hibernate
            if let Err(e) = Self::hibernate(agent) {
                tracing::warn!(
                    "apply_pattern: failed to hibernate agent {}: {e}",
                    agent.id.0
                );
            }
        }
    }

    // ── State Consistency Validation ────────────────────────────

    /// Validate and fix inconsistent agent states.
    /// Returns a list of issues that were detected and fixed.
    ///
    /// Detects:
    /// - Active agents with no runtime lease → auto-hibernate
    /// - Hibernating agents with active runtime lease → clear lease
    /// - Terminated agents with lingering checkpoints → clear checkpoints
    pub fn validate_agent_state(agent: &mut Agent) -> Vec<String> {
        let mut issues = Vec::new();

        if agent.state == AgentState::Active && agent.current_runtime.is_none() {
            issues.push(
                "Active state with no runtime lease — forcing Hibernating state".into(),
            );
            agent.state = AgentState::Hibernating;
        }

        if agent.state == AgentState::Hibernating && agent.current_runtime.is_some() {
            issues.push(
                "Hibernating state with active runtime lease — clearing lease".into(),
            );
            agent.current_runtime = None;
        }

        if agent.state == AgentState::Terminated && !agent.checkpoints.is_empty() {
            issues.push(format!(
                "Terminated state with {} lingering checkpoints — clearing",
                agent.checkpoints.len()
            ));
            agent.checkpoints.clear();
        }

        if !issues.is_empty() {
            agent.updated_at = Utc::now();
        }

        issues
    }

    // ── Periodic Checkpoint ─────────────────────────────────────

    /// Create a periodic checkpoint for long-running agent sessions.
    /// Automatically runs GC to keep checkpoint count bounded.
    #[must_use]
    pub fn create_periodic_checkpoint(agent: &mut Agent) -> Checkpoint {
        let cp = Self::create_checkpoint(agent, CheckpointReason::Periodic);
        agent.checkpoints.push(cp.clone());
        Self::gc_checkpoints(agent);
        agent.updated_at = Utc::now();
        cp
    }
}
