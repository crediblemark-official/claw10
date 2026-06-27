use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Serialize;
use tracing::info;

#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    #[error("Telemetry emission failed: {0}")]
    EmissionFailed(String),
}

/// A structured telemetry event for observability pipeline consumption.
///
/// Follows the ClawHive Telemetry Specification (PRD §36).
/// Required fields: timestamp, event_type, status.
/// Optional fields for correlation: tenant_id, mission_id, task_id, agent_id, etc.
#[derive(Debug, Clone, Serialize)]
pub struct TelemetryEvent {
    pub timestamp: DateTime<Utc>,
    pub tenant_id: Option<String>,
    pub mission_id: Option<String>,
    pub task_id: Option<String>,
    pub agent_id: Option<String>,
    pub parent_agent_id: Option<String>,
    pub lineage_id: Option<String>,
    pub worker_id: Option<String>,
    pub trace_id: Option<String>,
    pub event_type: String,
    pub lifecycle_mode: Option<String>,
    pub risk_level: Option<String>,
    pub status: String,
    pub cost_usd: f64,
    pub additional: HashMap<String, String>,
}

impl TelemetryEvent {
    #[must_use]
    pub fn new(event_type: impl Into<String>, status: impl Into<String>) -> Self {
        Self {
            timestamp: Utc::now(),
            tenant_id: None,
            mission_id: None,
            task_id: None,
            agent_id: None,
            parent_agent_id: None,
            lineage_id: None,
            worker_id: None,
            trace_id: None,
            event_type: event_type.into(),
            lifecycle_mode: None,
            risk_level: None,
            status: status.into(),
            cost_usd: 0.0,
            additional: HashMap::new(),
        }
    }

    #[must_use]
    pub fn with_tenant_id(mut self, tenant_id: String) -> Self {
        self.tenant_id = Some(tenant_id);
        self
    }

    #[must_use]
    pub fn with_mission_id(mut self, mission_id: String) -> Self {
        self.mission_id = Some(mission_id);
        self
    }

    #[must_use]
    pub fn with_task_id(mut self, task_id: String) -> Self {
        self.task_id = Some(task_id);
        self
    }

    #[must_use]
    pub fn with_agent_id(mut self, agent_id: String) -> Self {
        self.agent_id = Some(agent_id);
        self
    }

    #[must_use]
    pub fn with_parent_agent_id(mut self, parent_agent_id: String) -> Self {
        self.parent_agent_id = Some(parent_agent_id);
        self
    }

    #[must_use]
    pub fn with_lineage_id(mut self, lineage_id: String) -> Self {
        self.lineage_id = Some(lineage_id);
        self
    }

    #[must_use]
    pub fn with_worker_id(mut self, worker_id: String) -> Self {
        self.worker_id = Some(worker_id);
        self
    }

    #[must_use]
    pub fn with_trace_id(mut self, trace_id: String) -> Self {
        self.trace_id = Some(trace_id);
        self
    }

    #[must_use]
    pub fn with_lifecycle_mode(mut self, lifecycle_mode: String) -> Self {
        self.lifecycle_mode = Some(lifecycle_mode);
        self
    }

    #[must_use]
    pub fn with_risk_level(mut self, risk_level: String) -> Self {
        self.risk_level = Some(risk_level);
        self
    }

    #[must_use]
    pub fn with_cost(mut self, cost_usd: f64) -> Self {
        self.cost_usd = cost_usd;
        self
    }

    #[must_use]
    pub fn with_additional(mut self, key: String, value: String) -> Self {
        self.additional.insert(key, value);
        self
    }
}

/// Telemetry service for emitting structured events.
///
/// Per FR-093: Telemetry failure must not affect task/agent state.
/// All emission methods are fire-and-forget.
#[derive(Clone)]
pub struct TelemetryService {
    enabled: bool,
}

impl TelemetryService {
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Emit a telemetry event as a structured JSON tracing log.
    ///
    /// # Errors
    /// Returns `TelemetryError` only if serialization fails.
    /// Per FR-093, callers should not propagate this error to task state.
    pub fn emit(&self, event: &TelemetryEvent) -> Result<(), TelemetryError> {
        if !self.enabled {
            return Ok(());
        }

        let json = serde_json::to_string(event)
            .map_err(|e| TelemetryError::EmissionFailed(e.to_string()))?;

        info!(target: "clawhive_telemetry", "{}", json);
        Ok(())
    }

    /// Convenience method: build an event from parts and emit it.
    ///
    /// # Errors
    /// Same as `emit`.
    pub fn record(
        &self,
        event_type: &str,
        status: &str,
        builder: impl FnOnce(TelemetryEvent) -> TelemetryEvent,
    ) -> Result<(), TelemetryError> {
        let event = builder(TelemetryEvent::new(event_type, status));
        self.emit(&event)
    }
}

impl Default for TelemetryService {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;

