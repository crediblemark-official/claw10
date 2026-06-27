use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use clawhive_domain::AgentId;
use clawhive_model_router::types::ModelMessage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionId(pub Uuid);

#[derive(Debug, Clone)]
pub struct AgentSession {
    pub id: SessionId,
    pub agent_id: AgentId,
    pub messages: Vec<ModelMessage>,
    pub turn_count: u32,
    pub total_cost_usd: f64,
    pub total_tokens: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub state: SessionState,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Active,
    Paused,
    Completed,
    Terminated,
}

impl AgentSession {
    pub fn new(agent_id: AgentId) -> Self {
        let now = Utc::now();
        Self {
            id: SessionId(Uuid::now_v7()),
            agent_id,
            messages: Vec::new(),
            turn_count: 0,
            total_cost_usd: 0.0,
            total_tokens: 0,
            created_at: now,
            updated_at: now,
            state: SessionState::Active,
            metadata: HashMap::new(),
        }
    }

    pub fn add_message(&mut self, message: ModelMessage) {
        self.messages.push(message);
        self.updated_at = Utc::now();
    }

    pub fn record_turn(&mut self, tokens: u32, cost: f64) {
        self.turn_count += 1;
        self.total_tokens += tokens;
        self.total_cost_usd += cost;
        self.updated_at = Utc::now();
    }

    pub fn system_prompt(&self) -> Option<&str> {
        self.messages
            .iter()
            .find(|m| m.role == clawhive_model_router::types::MessageRole::System)
            .map(|m| m.content.as_str())
    }

    pub fn context_length(&self) -> usize {
        self.messages.iter().map(|m| m.content.len()).sum()
    }
}
