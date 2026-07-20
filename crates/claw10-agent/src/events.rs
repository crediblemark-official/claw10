use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentEvent {
    SessionStarted {
        agent_id: String,
        session_id: String,
        objective: String,
    },
    ModelCall {
        turn: u32,
        tokens: u32,
        cost: f64,
    },
    /// Delta teks streaming per-karakter dari LLM (sebelum turn selesai).
    TextDelta {
        turn: u32,
        delta: String,
    },
    ToolCall {
        tool: String,
        args: serde_json::Value,
        result: serde_json::Value,
    },
    Thought {
        turn: u32,
        content: String,
    },
    ObjectiveComplete {
        summary: String,
        evidence: Vec<String>,
    },
    SessionPaused {
        reason: String,
    },
    SessionTerminated {
        reason: String,
    },
    Error {
        message: String,
    },
    BudgetWarning {
        remaining: f64,
    },
    /// Budget hard limit has been reached — execution is being stopped.
    BudgetExceeded {
        spent: f64,
        hard_limit: f64,
    },
    /// A tool call failed verification and is being retried.
    ToolRetry {
        tool: String,
        attempt: u32,
        max_retries: u32,
        reason: String,
    },
    /// A tool call failed all verification retries.
    VerificationFailed {
        tool: String,
        reason: String,
        suggestion: Option<String>,
    },
}
