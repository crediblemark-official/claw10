use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTrace {
    pub trace_id: String,
    pub prompt_bundle_id: String,
    pub agent_id: String,
    pub agent_role: String,
    pub lifecycle_mode: String,
    pub mission_id: String,
    pub task_id: String,
    pub prompt_version: String,
    pub system_message_count: usize,
    pub context_format: String,
    pub policy_bundle_id: String,
    pub policy_hash: String,
    pub estimated_input_tokens: u32,
    pub output_contract: String,
    pub assembled_at: String,
}

pub struct PromptTracer;

impl PromptTracer {
    #[must_use]
    pub fn record(
        prompt_bundle_id: &str,
        agent_id: &str,
        agent_role: &str,
        lifecycle_mode: &str,
        mission_id: &str,
        task_id: &str,
        prompt_version: &str,
        system_message_count: usize,
        context_format: &str,
        policy_bundle_id: &str,
        policy_hash: &str,
        estimated_input_tokens: u32,
        output_contract: &str,
    ) -> PromptTrace {
        let trace_id = Self::generate_trace_id(prompt_bundle_id, agent_id);

        PromptTrace {
            trace_id,
            prompt_bundle_id: prompt_bundle_id.to_string(),
            agent_id: agent_id.to_string(),
            agent_role: agent_role.to_string(),
            lifecycle_mode: lifecycle_mode.to_string(),
            mission_id: mission_id.to_string(),
            task_id: task_id.to_string(),
            prompt_version: prompt_version.to_string(),
            system_message_count,
            context_format: context_format.to_string(),
            policy_bundle_id: policy_bundle_id.to_string(),
            policy_hash: policy_hash.to_string(),
            estimated_input_tokens,
            output_contract: output_contract.to_string(),
            assembled_at: Utc::now().to_rfc3339(),
        }
    }

    fn generate_trace_id(prompt_bundle_id: &str, agent_id: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(prompt_bundle_id.as_bytes());
        hasher.update(agent_id.as_bytes());
        hasher.update(Utc::now().to_rfc3339().as_bytes());
        hex::encode(hasher.finalize())[..16].to_string()
    }
}

#[cfg(test)]
#[path = "audit_test.rs"]
mod tests;

