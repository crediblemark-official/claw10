use serde_json::Value;
use uuid::Uuid;

use crate::bundle::{
    ContextFormat, OutputContractInput, PromptBuildRequest, PromptBundle, PromptMetadata,
};
use crate::policy_digest::PolicyDigestBuilder;
use crate::prompts::contracts;
use crate::registry::IcvsPromptRegistry;
use crate::validation::{SchemaValidationOutcome, SchemaValidator};

pub struct PromptAssembler {
    registry: IcvsPromptRegistry,
    prompt_version: String,
}

impl PromptAssembler {
    #[must_use]
    pub fn new() -> Self {
        Self {
            registry: IcvsPromptRegistry::new(),
            prompt_version: "1.0.0".to_string(),
        }
    }

    pub fn with_version(mut self, version: &str) -> Self {
        self.prompt_version = version.to_string();
        self
    }

    #[must_use]
    pub fn registry(&self) -> &IcvsPromptRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut IcvsPromptRegistry {
        &mut self.registry
    }

    pub fn build(&mut self, request: PromptBuildRequest) -> Result<PromptBundle, PromptError> {
        let mut system_messages: Vec<String> = Vec::new();

        // 1. Base kernel
        let kernel = self.registry.get_kernel();
        for p in kernel {
            system_messages.push(p.content.clone());
        }

        // 2. Role prompt
        let role_prompts = self
            .registry
            .get_role_prompt(&request.agent.role)
            .map_err(|e| PromptError::RoleNotFound(request.agent.role.clone(), e.to_string()))?;
        for p in &role_prompts {
            system_messages.push(p.content.clone());
        }

        // 3. Lifecycle prompt
        let lifecycle_prompts = self
            .registry
            .get_lifecycle_prompt(&request.agent.lifecycle_mode)
            .map_err(|e| {
                PromptError::LifecycleNotFound(request.agent.lifecycle_mode.clone(), e.to_string())
            })?;
        for p in &lifecycle_prompts {
            system_messages.push(p.content.clone());
        }

        // 4. Policy digest
        let policy_digest = PolicyDigestBuilder::build(&request.policy_ir);
        system_messages.push(policy_digest);

        // 5. Injection safety
        let injection = self.registry.get_injection_prompt();
        for p in injection {
            system_messages.push(p.content.clone());
        }

        // Build context message
        let context_format = if request.memories.is_empty()
            && request.team.is_empty()
            && request.task.objective.is_empty()
        {
            ContextFormat::Json
        } else {
            ContextFormat::Toon
        };
        let context_message = build_context_message(&request, context_format);

        // Build response schema
        let response_schema = request
            .output_contract
            .schema
            .clone();

        // Build metadata
        let prompt_bundle_id = Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext))
            .to_string();

        let metadata = PromptMetadata {
            prompt_bundle_id: prompt_bundle_id.clone(),
            prompt_version: self.prompt_version.clone(),
            agent_id: request.agent.id.clone(),
            agent_role: request.agent.role.clone(),
            lifecycle_mode: request.agent.lifecycle_mode.clone(),
            mission_id: request.mission.id.clone(),
            task_id: request.task.id.clone(),
            policy_bundle_id: request.policy_ir.id.clone(),
            policy_hash: request.policy_ir.hash.clone(),
            context_format,
        };

        let bundle = PromptBundle {
            system_messages,
            context_message,
            response_schema,
            tools: request.tools,
            metadata,
        };

        Ok(bundle)
    }

    pub fn validate_response(
        response: &Value,
        contract: &OutputContractInput,
    ) -> SchemaValidationOutcome {
        let schema = if contract.schema.is_null() {
            contracts::get_schema(&contract.output_type).unwrap_or_default()
        } else {
            contract.schema.clone()
        };
        SchemaValidator::validate(response, &schema)
    }
}

impl Default for PromptAssembler {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PromptError {
    #[error("Role not found: {0} ({1})")]
    RoleNotFound(String, String),

    #[error("Lifecycle not found: {0} ({1})")]
    LifecycleNotFound(String, String),

    #[error("Assembly error: {0}")]
    Assembly(String),
}

fn build_context_message(request: &PromptBuildRequest, format: ContextFormat) -> String {
    match format {
        ContextFormat::Toon => build_toon_context(request),
        ContextFormat::Json => build_json_context(request),
    }
}

fn build_toon_context(request: &PromptBuildRequest) -> String {
    use claw10_toon::ToonContext;

    let mut ctx = ToonContext::new();

    if !request.mission.objective.is_empty() {
        let mission = format!(
            "id: {}\nobjective: \"{}\"\nrisk: {}",
            request.mission.id,
            request.mission.objective.replace('"', "\\\""),
            request.mission.risk_level
        );
        ctx.add_section("mission", mission);
    }

    if !request.task.objective.is_empty() {
        let task = format!(
            "id: {}\nobjective: \"{}\"\nstatus: {}\nacceptance_criteria: {}",
            request.task.id,
            request.task.objective.replace('"', "\\\""),
            request.task.status,
            request.task.acceptance_criteria.join(", "),
        );
        ctx.add_section("task", task);
    }

    if !request.memories.is_empty() {
        let memories: Vec<String> = request
            .memories
            .iter()
            .map(|m| {
                format!(
                    "- \"{}\" (type: {}, confidence: {})",
                    m.content.replace('"', "\\\""),
                    m.memory_type,
                    m.confidence
                )
            })
            .collect();
        ctx.add_section("memory", memories.join("\n"));
    }

    if !request.team.is_empty() {
        let roster: Vec<String> = request
            .team
            .iter()
            .map(|m| {
                format!(
                    "- {} role: {} status: {}",
                    m.id, m.role, m.status
                )
            })
            .collect();
        ctx.add_section("team", roster.join("\n"));
    }

    let budget_text = format!(
        "allocated: ${:.2}\nspent: ${:.2}\nremaining: ${:.2}\nreserved: ${:.2}",
        request.budget.allocated,
        request.budget.spent,
        request.budget.remaining,
        request.budget.reserved,
    );
    ctx.add_section("budget", budget_text);

    ctx.build().unwrap_or_else(|e| format!("[TOON encode error: {e}]"))
}

fn build_json_context(request: &PromptBuildRequest) -> String {
    let ctx = serde_json::json!({
        "agent": {
            "id": request.agent.id,
            "role": request.agent.role,
            "lifecycle": request.agent.lifecycle_mode,
        },
        "mission": {
            "id": request.mission.id,
            "objective": request.mission.objective,
            "risk_level": request.mission.risk_level,
        },
        "task": {
            "id": request.task.id,
            "objective": request.task.objective,
            "acceptance_criteria": request.task.acceptance_criteria,
        },
        "budget": {
            "allocated": request.budget.allocated,
            "spent": request.budget.spent,
            "remaining": request.budget.remaining,
        },
    });
    serde_json::to_string_pretty(&ctx).unwrap_or_default()
}

#[cfg(test)]
#[path = "assembler_test.rs"]
mod tests;

