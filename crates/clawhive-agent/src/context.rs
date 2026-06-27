use std::collections::HashMap;

use clawhive_domain::Agent;
use clawhive_model_router::types::{ModelMessage, MessageRole, ToolDefinition};
use clawhive_tool::registry::ToolRegistry;

pub struct ContextBuilder;

impl ContextBuilder {
    pub fn build_system_prompt(
        agent: &Agent,
        objective: &str,
        additional_context: &HashMap<String, String>,
    ) -> String {
        let mut prompt = String::new();

        prompt.push_str(&format!("You are {}, a {} agent.\n\n", agent.name, agent.role));

        if let Some(mission_stmt) = additional_context.get("mission_statement") {
            prompt.push_str(&format!("## Mission\n{}\n\n", mission_stmt));
        }

        prompt.push_str(&format!("## Objective\n{}\n\n", objective));

        prompt.push_str(&format!(
            "## Constraints\n\
            - Turn budget: {}\n\
            - Budget remaining: ${:.2}\n\
            - Spawn permission: {}\n\
            - Max spawn depth: {}\n\n",
            agent.genome.autonomy.max_children,
            agent.budget.remaining(),
            agent.genome.autonomy.can_spawn,
            agent.genome.autonomy.max_spawn_depth,
        ));

        if !agent.delegable_permissions.is_empty() {
            prompt.push_str("## Available Permissions\n");
            for perm in &agent.delegable_permissions {
                prompt.push_str(&format!("- {}\n", perm.0));
            }
            prompt.push_str("\n");
        }

        prompt.push_str(
            "## Rules\n\
            1. You can use the available tools to accomplish your objective.\n\
            2. After each tool result, analyze and decide next steps.\n\
            3. When the objective is complete, respond with a summary and evidence.\n\
            4. Stay within your budget and turn limits.\n\
            5. Do not share credentials or sensitive information.\n",
        );

        prompt
    }

    pub fn build_initial_messages(
        agent: &Agent,
        objective: &str,
        context: &HashMap<String, String>,
    ) -> Vec<ModelMessage> {
        vec![ModelMessage {
            role: MessageRole::System,
            content: Self::build_system_prompt(agent, objective, context),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }]
    }

    pub fn tool_definitions(tool_registry: &ToolRegistry) -> Vec<ToolDefinition> {
        tool_registry
            .list()
            .iter()
            .map(|t| ToolDefinition {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.input_schema(),
            })
            .collect()
    }
}
