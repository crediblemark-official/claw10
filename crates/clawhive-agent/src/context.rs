use std::collections::HashMap;

use clawhive_domain::Agent;
use clawhive_model_router::types::{MessageRole, ModelMessage, ToolDefinition as RouterToolDefinition};
use clawhive_tool::registry::ToolRegistry;

pub struct ContextBuilder;

impl ContextBuilder {
    /// Merakit system prompt dan context prompt menggunakan subsistem `clawhive-prompt`.
    /// Mengembalikan tuple `(system_prompt, context_message)`.
    #[must_use]
    pub fn build_system_prompt(
        agent: &Agent,
        objective: &str,
        additional_context: &HashMap<String, String>,
        tool_definitions: Vec<clawhive_prompt::ToolDefinition>,
    ) -> (String, String) {
        let mut assembler = clawhive_prompt::PromptAssembler::new();

        let rules = agent
            .policy_bundle
            .rules
            .iter()
            .map(|r| format!(
                "id: {}\nsubject: {:?}\neffect: {:?}\naction: {}\nresource: {}\npriority: {}",
                r.id.0, r.subject, r.effect, r.action, r.resource, r.priority
            ))
            .collect::<Vec<_>>()
            .join("\n---\n");

        let request = clawhive_prompt::PromptBuildRequest {
            agent: clawhive_prompt::AgentPromptInput {
                id: agent.id.0.to_string(),
                role: agent.role.clone(),
                lifecycle_mode: format!("{:?}", agent.lifecycle_mode),
                organization_id: "default".to_string(),
                memory_scopes: agent.genome.memory.default_read_scopes.clone(),
            },
            mission: clawhive_prompt::MissionPromptInput {
                id: agent.mission_id.0.to_string(),
                objective: additional_context
                    .get("mission_statement")
                    .cloned()
                    .unwrap_or_else(|| "E-commerce System Architecture and Design Plan".to_string()),
                scope: None,
                status: "Active".to_string(),
                risk_level: "Medium".to_string(),
            },
            task: clawhive_prompt::TaskPromptInput {
                id: uuid::Uuid::now_v7().to_string(),
                objective: objective.to_string(),
                status: "In_Progress".to_string(),
                deadline: None,
                acceptance_criteria: vec![],
                required_evidence: vec![],
            },
            memories: vec![],
            team: vec![],
            budget: clawhive_prompt::BudgetPromptInput {
                allocated: agent.budget.allocated_usd,
                spent: agent.budget.spent_usd,
                remaining: agent.budget.remaining(),
                reserved: 0.0,
            },
            tools: tool_definitions,
            policy_ir: clawhive_prompt::PolicyIrInput {
                id: agent.policy_bundle.id.0.to_string(),
                hash: agent.policy_bundle.version.clone(),
                rules,
            },
            model_profile: agent.genome.model_policy.preferred_profile.clone(),
            output_contract: clawhive_prompt::OutputContractInput {
                output_type: "assistant".to_string(),
                schema: serde_json::Value::Null,
            },
        };

        match assembler.build(request) {
            Ok(bundle) => {
                let system_prompt = bundle.system_messages.join("\n\n");
                (system_prompt, bundle.context_message)
            }
            Err(e) => {
                // Fallback jika terjadi error pada perakitan prompt
                let fallback_prompt = format!(
                    "You are {}, a {} agent.\n\nObjective: {}\n\nError assembling prompt: {}",
                    agent.name, agent.role, objective, e
                );
                (fallback_prompt, format!("Task: {}", objective))
            }
        }
    }

    #[must_use]
    pub fn build_initial_messages(
        agent: &Agent,
        objective: &str,
        context: &HashMap<String, String>,
        tool_registry: &ToolRegistry,
    ) -> Vec<ModelMessage> {
        let prompt_tool_defs = Self::tool_definitions_for_prompt(tool_registry);
        let (system_prompt, context_message) =
            Self::build_system_prompt(agent, objective, context, prompt_tool_defs);

        vec![
            ModelMessage {
                role: MessageRole::System,
                content: system_prompt,
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            ModelMessage {
                role: MessageRole::User,
                content: context_message,
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        ]
    }

    #[must_use]
    pub fn tool_definitions(tool_registry: &ToolRegistry) -> Vec<RouterToolDefinition> {
        tool_registry
            .list()
            .iter()
            .map(|t| RouterToolDefinition {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.input_schema(),
            })
            .collect()
    }

    #[must_use]
    fn tool_definitions_for_prompt(tool_registry: &ToolRegistry) -> Vec<clawhive_prompt::ToolDefinition> {
        tool_registry
            .list()
            .iter()
            .map(|t| clawhive_prompt::ToolDefinition {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.input_schema(),
            })
            .collect()
    }
}
