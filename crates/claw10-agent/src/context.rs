use std::collections::HashMap;

use claw10_domain::Agent;
use claw10_model_router::types::{MessageRole, ModelMessage, ToolDefinition as RouterToolDefinition};
use claw10_tool::registry::ToolRegistry;

pub struct ContextBuilder;

impl ContextBuilder {
    /// Merakit system prompt dan context prompt menggunakan subsistem `claw10-prompt`.
    /// Mengembalikan tuple `(system_prompt, context_message)`.
    #[must_use]
    pub fn build_system_prompt(
        agent: &Agent,
        objective: &str,
        additional_context: &HashMap<String, String>,
        tool_definitions: Vec<claw10_prompt::ToolDefinition>,
    ) -> (String, String) {
        let mut assembler = claw10_prompt::PromptAssembler::new();

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

        let mut memories = vec![];
        if let Some(distilled) = additional_context.get("distilled_memory") {
            memories.push(claw10_prompt::MemoryPromptInput {
                content: distilled.clone(),
                memory_type: "distilled_long_term".to_string(),
                confidence: 1.0,
            });
        }

        let request = claw10_prompt::PromptBuildRequest {
            agent: claw10_prompt::AgentPromptInput {
                id: agent.id.0.to_string(),
                role: agent.role.clone(),
                lifecycle_mode: format!("{:?}", agent.lifecycle_mode),
                organization_id: "default".to_string(),
                memory_scopes: agent.genome.memory.default_read_scopes.clone(),
            },
            mission: claw10_prompt::MissionPromptInput {
                id: agent.mission_id.0.to_string(),
                objective: additional_context
                    .get("mission_statement")
                    .cloned()
                    .unwrap_or_else(|| "E-commerce System Architecture and Design Plan".to_string()),
                scope: None,
                status: "Active".to_string(),
                risk_level: "Medium".to_string(),
            },
            task: claw10_prompt::TaskPromptInput {
                id: uuid::Uuid::now_v7().to_string(),
                objective: objective.to_string(),
                status: "In_Progress".to_string(),
                deadline: None,
                acceptance_criteria: vec![],
                required_evidence: vec![],
            },
            memories,
            team: vec![],
            budget: claw10_prompt::BudgetPromptInput {
                allocated: agent.budget.allocated_usd,
                spent: agent.budget.spent_usd,
                remaining: agent.budget.remaining(),
                reserved: 0.0,
            },
            tools: tool_definitions,
            policy_ir: claw10_prompt::PolicyIrInput {
                id: agent.policy_bundle.id.0.to_string(),
                hash: agent.policy_bundle.version.clone(),
                rules,
            },
            model_profile: agent.genome.model_policy.preferred_profile.clone(),
            output_contract: claw10_prompt::OutputContractInput {
                output_type: "assistant".to_string(),
                schema: serde_json::Value::Null,
            },
        };

        match assembler.build(request) {
            Ok(bundle) => {
                let mut system_messages = bundle.system_messages;

                // Load config dinamis untuk SOUL, IDENTITY, USER profil dari database context
                let mut agent_str = String::new();
                if let Some(soul) = additional_context.get("agent_soul") {
                    agent_str.push_str(&format!("=== AGENT SOUL ===\n{}\n\n", soul));
                }
                if let Some(identity) = additional_context.get("agent_name") {
                    agent_str.push_str(&format!("=== AGENT IDENTITY ===\nIdentity: {}\n", identity));
                }
                if !agent_str.is_empty() {
                    system_messages.insert(0, agent_str);
                }

                let mut op_str = String::new();
                if let Some(name) = additional_context.get("operator_name") {
                    op_str.push_str("=== OPERATOR PROFILE ===\n");
                    op_str.push_str(&format!("Name: {}\n", name));
                    if let Some(timezone) = additional_context.get("operator_timezone") {
                        op_str.push_str(&format!("Timezone: {}\n", timezone));
                    }
                    if let Some(language) = additional_context.get("operator_language") {
                        op_str.push_str(&format!("Preferred Language: {}\n", language));
                    }
                    if let Some(style) = additional_context.get("operator_style") {
                        op_str.push_str(&format!("Communication Style: {}\n", style));
                    }
                }
                if !op_str.is_empty() {
                    system_messages.insert(0, op_str);
                }

                // Integrasikan system_context (memories, policies, skills, agents roster, lineage, history)
                if let Some(sys_ctx) = additional_context.get("system_context") {
                    system_messages.push(sys_ctx.clone());
                }

                let system_prompt = system_messages.join("\n\n");
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
    fn tool_definitions_for_prompt(tool_registry: &ToolRegistry) -> Vec<claw10_prompt::ToolDefinition> {
        tool_registry
            .list()
            .iter()
            .map(|t| claw10_prompt::ToolDefinition {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.input_schema(),
            })
            .collect()
    }
}
