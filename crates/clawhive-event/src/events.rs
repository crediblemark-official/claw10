//! Domain event types yang dipublish dan disubscribe oleh seluruh sistem.
//!
//! Semua event harus serializable (JSON) agar bisa dikirim via NATS atau
//! disimpan dalam event store untuk replay.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Semua event domain-level dalam ClawHive.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClawHiveEvent {
    // ── Agent Lifecycle ────────────────────────────────────────────
    /// Agent baru berhasil di-spawn dan disimpan.
    AgentSpawned {
        agent_id: Uuid,
        parent_agent_id: Option<Uuid>,
        mission_id: Uuid,
        role: String,
        lifecycle_mode: String,
        timestamp: DateTime<Utc>,
    },

    /// Agent aktif mulai hibernasi.
    AgentHibernated {
        agent_id: Uuid,
        checkpoint_id: Uuid,
        timestamp: DateTime<Utc>,
    },

    /// Agent hibernasi berhasil dibangunkan.
    AgentWoken {
        agent_id: Uuid,
        trigger: WakeTrigger,
        timestamp: DateTime<Utc>,
    },

    /// Agent diterminasi (ephemeral selesai atau kill).
    AgentTerminated {
        agent_id: Uuid,
        reason: TerminationReason,
        timestamp: DateTime<Utc>,
    },

    /// Agent dimigrasikan ke worker lain.
    AgentMigrated {
        agent_id: Uuid,
        from_worker: String,
        to_worker: String,
        checkpoint_id: Uuid,
        timestamp: DateTime<Utc>,
    },

    // ── Spawn ──────────────────────────────────────────────────────
    /// Spawn request diterima dan divalidasi.
    SpawnRequestApproved {
        spawn_request_id: Uuid,
        parent_agent_id: Uuid,
        child_count: usize,
        timestamp: DateTime<Utc>,
    },

    /// Spawn request ditolak.
    SpawnRequestDenied {
        spawn_request_id: Uuid,
        parent_agent_id: Uuid,
        reason: String,
        timestamp: DateTime<Utc>,
    },

    // ── Scheduler ─────────────────────────────────────────────────
    /// Schedule agent jatuh tempo dan perlu dibangunkan.
    ScheduleDue {
        agent_id: Uuid,
        cron: String,
        timestamp: DateTime<Utc>,
    },

    // ── Memory ────────────────────────────────────────────────────
    /// Memory candidate baru masuk admission pipeline.
    MemoryCandidateSubmitted {
        memory_id: Uuid,
        agent_id: Uuid,
        scope: String,
        timestamp: DateTime<Utc>,
    },

    /// Memory berhasil diaktifkan setelah admission.
    MemoryActivated {
        memory_id: Uuid,
        scope: String,
        confidence: f64,
        timestamp: DateTime<Utc>,
    },

    /// Memory ditolak dalam admission pipeline.
    MemoryRejected {
        memory_id: Uuid,
        reason: String,
        timestamp: DateTime<Utc>,
    },

    // ── Task ──────────────────────────────────────────────────────
    /// Task selesai diverifikasi.
    TaskVerified {
        task_id: Uuid,
        verifier_agent_id: Uuid,
        timestamp: DateTime<Utc>,
    },

    /// Task gagal dan butuh escalation.
    TaskFailed {
        task_id: Uuid,
        agent_id: Uuid,
        reason: String,
        timestamp: DateTime<Utc>,
    },

    // ── Worker ────────────────────────────────────────────────────
    /// Worker heartbeat diterima.
    WorkerHeartbeat {
        worker_id: Uuid,
        timestamp: DateTime<Utc>,
    },

    /// Worker dideteksi stale.
    WorkerStale {
        worker_id: Uuid,
        last_seen: DateTime<Utc>,
        timestamp: DateTime<Utc>,
    },
}

impl ClawHiveEvent {
    /// Subject/topic NATS untuk event ini.
    /// Format: `clawhive.<domain>.<action>`
    #[must_use]
    pub fn subject(&self) -> &'static str {
        match self {
            ClawHiveEvent::AgentSpawned { .. } => "clawhive.agent.spawned",
            ClawHiveEvent::AgentHibernated { .. } => "clawhive.agent.hibernated",
            ClawHiveEvent::AgentWoken { .. } => "clawhive.agent.woken",
            ClawHiveEvent::AgentTerminated { .. } => "clawhive.agent.terminated",
            ClawHiveEvent::AgentMigrated { .. } => "clawhive.agent.migrated",
            ClawHiveEvent::SpawnRequestApproved { .. } => "clawhive.spawn.approved",
            ClawHiveEvent::SpawnRequestDenied { .. } => "clawhive.spawn.denied",
            ClawHiveEvent::ScheduleDue { .. } => "clawhive.schedule.due",
            ClawHiveEvent::MemoryCandidateSubmitted { .. } => "clawhive.memory.submitted",
            ClawHiveEvent::MemoryActivated { .. } => "clawhive.memory.activated",
            ClawHiveEvent::MemoryRejected { .. } => "clawhive.memory.rejected",
            ClawHiveEvent::TaskVerified { .. } => "clawhive.task.verified",
            ClawHiveEvent::TaskFailed { .. } => "clawhive.task.failed",
            ClawHiveEvent::WorkerHeartbeat { .. } => "clawhive.worker.heartbeat",
            ClawHiveEvent::WorkerStale { .. } => "clawhive.worker.stale",
        }
    }

    /// Timestamp event.
    #[must_use]
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            ClawHiveEvent::AgentSpawned { timestamp, .. }
            | ClawHiveEvent::AgentHibernated { timestamp, .. }
            | ClawHiveEvent::AgentWoken { timestamp, .. }
            | ClawHiveEvent::AgentTerminated { timestamp, .. }
            | ClawHiveEvent::AgentMigrated { timestamp, .. }
            | ClawHiveEvent::SpawnRequestApproved { timestamp, .. }
            | ClawHiveEvent::SpawnRequestDenied { timestamp, .. }
            | ClawHiveEvent::ScheduleDue { timestamp, .. }
            | ClawHiveEvent::MemoryCandidateSubmitted { timestamp, .. }
            | ClawHiveEvent::MemoryActivated { timestamp, .. }
            | ClawHiveEvent::MemoryRejected { timestamp, .. }
            | ClawHiveEvent::TaskVerified { timestamp, .. }
            | ClawHiveEvent::TaskFailed { timestamp, .. }
            | ClawHiveEvent::WorkerHeartbeat { timestamp, .. }
            | ClawHiveEvent::WorkerStale { timestamp, .. } => *timestamp,
        }
    }
}

/// Alasan agent dibangunkan dari hibernasi.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WakeTrigger {
    ScheduleDue,
    EventSubscription { event_type: String },
    ManualWake,
    Heartbeat,
}

/// Alasan terminasi agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminationReason {
    TaskCompleted,
    BudgetExhausted,
    TtlExpired,
    ParentTerminated,
    PolicyViolation,
    OperatorKill,
    Orphaned,
}
