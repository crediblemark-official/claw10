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
    /// Batas panjang konteks dalam token (diestimasi 1 token ≈ 4 karakter).
    pub max_context_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Active,
    Paused,
    Completed,
    Terminated,
}

impl AgentSession {
    #[must_use]
    pub fn new(agent_id: AgentId) -> Self {
        Self::with_context_limit(agent_id, None)
    }

    #[must_use]
    pub fn with_context_limit(agent_id: AgentId, max_context_tokens: Option<u32>) -> Self {
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
            max_context_tokens,
        }
    }

    pub fn add_message(&mut self, message: ModelMessage) {
        self.messages.push(message);
        self.trim_to_context_limit();
        self.updated_at = Utc::now();
    }

    /// Buang pesan lama (kecuali system prompt) hingga di bawah batas konteks.
    fn trim_to_context_limit(&mut self) {
        let Some(limit) = self.max_context_tokens else {
            return;
        };
        let max_chars = (limit as usize).saturating_mul(4);

        while self.context_length() > max_chars && self.messages.len() > 1 {
            // Cari pesan non-system tertua untuk dibuang
            let remove_idx = self
                .messages
                .iter()
                .position(|m| m.role != clawhive_model_router::types::MessageRole::System);
            if let Some(idx) = remove_idx {
                self.messages.remove(idx);
            } else {
                break;
            }
        }
    }

    pub fn record_turn(&mut self, tokens: u32, cost: f64) {
        self.turn_count += 1;
        self.total_tokens += tokens;
        self.total_cost_usd += cost;
        self.updated_at = Utc::now();
    }

    #[must_use]
    pub fn system_prompt(&self) -> Option<&str> {
        self.messages
            .iter()
            .find(|m| m.role == clawhive_model_router::types::MessageRole::System)
            .map(|m| m.content.as_str())
    }

    #[must_use]
    pub fn context_length(&self) -> usize {
        self.messages.iter().map(|m| m.content.len()).sum()
    }
}
