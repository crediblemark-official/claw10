use tracing::warn;

use clawhive_domain::{
    Agent, Evidence, Lineage, Memory, Mission, PolicyBundle, Skill, Task, Worker,
};
use clawhive_toon::{ToonContext, ToonEncoder};

#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("Token budget exhausted: needed {needed}, available {available}")]
    TokenBudgetExhausted { needed: usize, available: usize },
    #[error("No suitable context sources available")]
    NoSources,
}

/// Defines what context sources are available for building.
#[derive(Debug, Default)]
pub struct ContextSources<'a> {
    pub task: Option<&'a Task>,
    pub mission: Option<&'a Mission>,
    pub memories: &'a [Memory],
    pub policies: &'a [PolicyBundle],
    pub skills: &'a [Skill],
    pub history: &'a [String],
    pub tools: &'a [String],
    pub agents: &'a [Agent],
    pub lineage: Option<&'a Lineage>,
    pub workers: &'a [Worker],
    pub evidence: &'a [Evidence],
}

#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub max_token_budget: usize,
    pub include_task: bool,
    pub include_mission: bool,
    pub include_memories: bool,
    pub include_policy: bool,
    pub include_skills: bool,
    pub include_history: bool,
    pub include_tools: bool,
    pub include_agent: bool,
    pub include_lineage: bool,
    pub include_workers: bool,
    pub include_evidence: bool,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            max_token_budget: 4096,
            include_task: true,
            include_mission: true,
            include_memories: true,
            include_policy: true,
            include_skills: true,
            include_history: true,
            include_tools: true,
            include_agent: true,
            include_lineage: true,
            include_workers: false,
            include_evidence: true,
        }
    }
}

pub struct ContextPipeline {
    config: PipelineConfig,
}

impl ContextPipeline {
    #[must_use]
    pub fn new(config: PipelineConfig) -> Self {
        Self { config }
    }

    /// Build a complete context string from available sources.
    /// Orchestrates selection -> encoding -> assembly within token budget.
    pub async fn build_context(&self, sources: ContextSources<'_>) -> Result<String, ContextError> {
        let mut ctx = ToonContext::new();

        if self.config.include_task {
            if let Some(task) = sources.task {
                ctx.add_section("task", ToonEncoder::encode_task(task));
            }
        }

        if self.config.include_mission {
            if let Some(mission) = sources.mission {
                ctx.add_section("mission", ToonEncoder::encode_mission(mission));
            }
        }

        if self.config.include_memories && !sources.memories.is_empty() {
            ctx.add_section("memory", ToonEncoder::encode_memories(sources.memories));
        }

        if self.config.include_policy && !sources.policies.is_empty() {
            ctx.add_section(
                "policy",
                ToonEncoder::encode_policy_summary(sources.policies),
            );
        }

        if self.config.include_skills && !sources.skills.is_empty() {
            ctx.add_section("skills", ToonEncoder::encode_skills(sources.skills));
        }

        if self.config.include_history && !sources.history.is_empty() {
            ctx.add_section("history", ToonEncoder::encode_history(sources.history));
        }

        if self.config.include_tools && !sources.tools.is_empty() {
            ctx.add_section("tools", ToonEncoder::encode_tools(sources.tools));
        }

        if self.config.include_agent && !sources.agents.is_empty() {
            ctx.add_section("agents", ToonEncoder::encode_agent_roster(sources.agents));
        }

        if self.config.include_lineage {
            if let Some(lineage) = sources.lineage {
                ctx.add_section("lineage", ToonEncoder::encode_lineage(lineage));
            }
        }

        if self.config.include_workers && !sources.workers.is_empty() {
            ctx.add_section("workers", ToonEncoder::encode_workers(sources.workers));
        }

        if self.config.include_evidence && !sources.evidence.is_empty() {
            ctx.add_section("evidence", ToonEncoder::encode_evidence(sources.evidence));
        }

        let context = ctx.build();
        let estimated_tokens = estimate_tokens(&context);

        if estimated_tokens > self.config.max_token_budget {
            warn!(
                estimated_tokens,
                max_budget = self.config.max_token_budget,
                "Context exceeds token budget, trimming"
            );
            Ok(self.trim_to_budget(&context, self.config.max_token_budget))
        } else {
            Ok(context)
        }
    }

    /// Trim context string to fit within token budget by truncating earliest/least important sections.
    fn trim_to_budget(&self, context: &str, max_tokens: usize) -> String {
        let sections: Vec<&str> = context.split("\n---\n").collect();
        let mut selected = Vec::new();
        let mut running_tokens = 0usize;

        for section in sections.iter().rev() {
            let section_tokens = estimate_tokens(section);
            if running_tokens + section_tokens <= max_tokens {
                selected.push(*section);
                running_tokens += section_tokens;
            } else {
                let available = max_tokens.saturating_sub(running_tokens);
                if available > 10 {
                    let chars_per_token = 4;
                    let max_chars = available * chars_per_token;
                    let truncated: String = section.chars().take(max_chars).collect();
                    selected.push(&*Box::leak(truncated.into_boxed_str()));
                }
                break;
            }
        }

        selected.reverse();
        selected.join("\n---\n")
    }
}

/// Rough token estimate: 1 token ≈ 4 characters.
fn estimate_tokens(text: &str) -> usize {
    (text.len() + 3) / 4
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;

