use std::fmt::Write;

use clawhive_domain::agent::Agent;
use clawhive_domain::evidence::Evidence;
use clawhive_domain::lineage::Lineage;
use clawhive_domain::memory::Memory;
use clawhive_domain::mission::Mission;
use clawhive_domain::policy::PolicyBundle;
use clawhive_domain::skill::Skill;
use clawhive_domain::task::Task;
use clawhive_domain::worker::Worker;

fn fmt_id<T: std::fmt::Debug>(id: &T) -> String {
    format!("{:?}", id)
}

#[derive(Debug, thiserror::Error)]
pub enum ToonError {
    #[error("Encoding error: {0}")]
    Encoding(String),
}

pub struct ToonContext {
    sections: Vec<String>,
}

impl ToonContext {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sections: Vec::new(),
        }
    }

    pub fn add_section(&mut self, name: &str, content: String) {
        self.sections.push(format!("\n[{}]\n{}", name, content));
    }

    pub fn build(&self) -> String {
        let mut output = String::from("[TOON v1]");
        for section in &self.sections {
            write!(output, "{}", section).unwrap();
        }
        output
    }
}

impl Default for ToonContext {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ToonEncoder;

impl ToonEncoder {
    pub fn encode_task(task: &Task) -> String {
        let deadline = task
            .deadline
            .map(|d| d.format("%Y-%m-%dT%H:%M:%SZ").to_string())
            .unwrap_or_default();

        vec![
            format!("id: {}", fmt_id(&task.id)),
            format!("objective: \"{}\"", task.objective.replace('"', "\\\"")),
            format!("state: {:?}", task.state),
            format!("risk: {:?}", task.risk),
            format!("deadline: {}", deadline),
        ].join("\n")
    }

    pub fn encode_mission(mission: &Mission) -> String {
        vec![
            format!("id: {}", fmt_id(&mission.id)),
            format!(
                "objective: \"{}\"",
                mission.objective.replace('"', "\\\"")
            ),
            format!("mode: {:?}", mission.lifecycle_mode),
        ].join("\n")
    }

    pub fn encode_memories(memories: &[Memory]) -> String {
        let mut lines = Vec::new();
        for m in memories {
            lines.push(format!(
                "- \"{}\" (type: {:?}, confidence: {})",
                m.content.replace('"', "\\\""),
                m.memory_type,
                m.confidence
            ));
        }
        lines.join("\n")
    }

    pub fn encode_policy_summary(bundles: &[PolicyBundle]) -> String {
        let mut lines = Vec::new();
        for bundle in bundles {
            if bundle.is_active {
                lines.push(format!(
                    "- {} ({} rules)",
                    fmt_id(&bundle.id),
                    bundle.rules.len()
                ));
                for rule in &bundle.rules {
                    let effect = if matches!(rule.effect, clawhive_domain::policy::PolicyEffect::Allow) {
                        "ALLOW"
                    } else {
                        "DENY"
                    };
                    lines.push(format!("  {} {} on {}", effect, rule.action, rule.resource));
                }
            }
        }
        lines.join("\n")
    }

    pub fn encode_agent_roster(agents: &[Agent]) -> String {
        let mut lines = Vec::new();
        for agent in agents {
            lines.push(format!(
                "{} role: {} state: {:?}",
                fmt_id(&agent.id),
                agent.role,
                agent.state,
            ));
        }
        lines.join("\n")
    }

    pub fn encode_lineage(lineage: &Lineage) -> String {
        let mut lines = vec![
            format!("root: {}", fmt_id(&lineage.root_agent_id)),
            format!("entries: {} total", lineage.entries.len()),
        ];
        for entry in &lineage.entries {
            lines.push(format!(
                "  - {} (parent: {}, state: {})",
                fmt_id(&entry.agent_id),
                entry.parent_agent_id.as_ref().map_or("none".to_string(), fmt_id),
                entry.state,
            ));
        }
        lines.join("\n")
    }

    pub fn encode_evidence(evidence: &[Evidence]) -> String {
        let mut lines = Vec::new();
        for ev in evidence {
            lines.push(format!(
                "- {} ({:?}, accepted: {})",
                fmt_id(&ev.id),
                ev.evidence_type,
                ev.accepted
            ));
        }
        lines.join("\n")
    }

    pub fn encode_skills(skills: &[Skill]) -> String {
        let mut lines = Vec::new();
        for skill in skills {
            lines.push(format!(
                "- {} v{} (state: {:?}, cost: ${})",
                skill.name,
                skill.version,
                skill.state,
                skill.cost_profile.estimated_cost_usd,
            ));
        }
        lines.join("\n")
    }

    pub fn encode_history(history: &[String]) -> String {
        let mut lines = Vec::new();
        for (i, msg) in history.iter().enumerate() {
            lines.push(format!("[{}] {}", i, msg));
        }
        lines.join("\n")
    }

    pub fn encode_workers(workers: &[Worker]) -> String {
        let mut lines = Vec::new();
        for worker in workers {
            lines.push(format!(
                "- {} (type: {:?}, state: {:?})",
                worker.name, worker.worker_type, worker.state,
            ));
        }
        lines.join("\n")
    }

    pub fn encode_tools(tools: &[String]) -> String {
        let mut lines = Vec::new();
        for tool in tools {
            lines.push(format!("- {}", tool));
        }
        lines.join("\n")
    }

    pub fn build_context(
        task: Option<&Task>,
        mission: Option<&Mission>,
        memories: &[Memory],
        policies: &[PolicyBundle],
        agents: &[Agent],
        lineage: Option<&Lineage>,
        evidence: &[Evidence],
    ) -> String {
        let mut ctx = ToonContext::new();

        if let Some(task) = task {
            ctx.add_section("task", Self::encode_task(task));
        }

        if let Some(mission) = mission {
            ctx.add_section("mission", Self::encode_mission(mission));
        }

        if !memories.is_empty() {
            ctx.add_section("memory", Self::encode_memories(memories));
        }

        if !policies.is_empty() {
            ctx.add_section("policy", Self::encode_policy_summary(policies));
        }

        if !agents.is_empty() {
            ctx.add_section("agents", Self::encode_agent_roster(agents));
        }

        if let Some(lineage) = lineage {
            ctx.add_section("lineage", Self::encode_lineage(lineage));
        }

        if !evidence.is_empty() {
            ctx.add_section("evidence", Self::encode_evidence(evidence));
        }

        ctx.build()
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;

