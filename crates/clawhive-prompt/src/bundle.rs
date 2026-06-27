use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Complete prompt package assembled for a single model call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptBundle {
    pub system_messages: Vec<String>,
    pub context_message: String,
    pub response_schema: Value,
    pub tools: Vec<ToolDefinition>,
    pub metadata: PromptMetadata,
}

/// Metadata attached to every prompt bundle for audit and versioning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMetadata {
    pub prompt_bundle_id: String,
    pub prompt_version: String,
    pub agent_id: String,
    pub agent_role: String,
    pub lifecycle_mode: String,
    pub mission_id: String,
    pub task_id: String,
    pub policy_bundle_id: String,
    pub policy_hash: String,
    pub context_format: ContextFormat,
}

/// Which format was used for the context message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextFormat {
    Toon,
    Json,
}

/// A tool the model may call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Structured request for the PromptAssembler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptBuildRequest {
    pub agent: AgentPromptInput,
    pub mission: MissionPromptInput,
    pub task: TaskPromptInput,
    pub memories: Vec<MemoryPromptInput>,
    pub team: Vec<TeamMemberInput>,
    pub budget: BudgetPromptInput,
    pub tools: Vec<ToolDefinition>,
    pub policy_ir: PolicyIrInput,
    pub model_profile: String,
    pub output_contract: OutputContractInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPromptInput {
    pub id: String,
    pub role: String,
    pub lifecycle_mode: String,
    pub organization_id: String,
    pub memory_scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionPromptInput {
    pub id: String,
    pub objective: String,
    pub scope: Option<String>,
    pub status: String,
    pub risk_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPromptInput {
    pub id: String,
    pub objective: String,
    pub status: String,
    pub deadline: Option<String>,
    pub acceptance_criteria: Vec<String>,
    pub required_evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPromptInput {
    pub content: String,
    pub memory_type: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMemberInput {
    pub id: String,
    pub role: String,
    pub status: String,
    pub objective: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetPromptInput {
    pub allocated: f64,
    pub spent: f64,
    pub remaining: f64,
    pub reserved: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyIrInput {
    pub id: String,
    pub hash: String,
    pub rules: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputContractInput {
    pub output_type: String,
    pub schema: Value,
}

/// A structured model response after schema + policy validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedAgentResponse {
    pub raw: Value,
    pub parsed: Value,
    pub schema_valid: bool,
    pub policy_valid: bool,
    pub response_hash: String,
}

impl PromptBundle {
    #[must_use]
    pub fn estimate_input_tokens(&self) -> u32 {
        let mut total = 0u32;
        for msg in &self.system_messages {
            total += (msg.len() as f64 * 0.4) as u32;
        }
        total += (self.context_message.len() as f64 * 0.4) as u32;
        total += (serde_json::to_string(&self.response_schema).unwrap_or_default().len() as f64 * 0.4) as u32;
        total
    }
}

#[cfg(test)]
#[path = "bundle_test.rs"]
mod tests;

