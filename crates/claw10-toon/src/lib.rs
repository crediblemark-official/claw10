use std::fmt::Write;

use claw10_domain::agent::Agent;
use claw10_domain::evidence::Evidence;
use claw10_domain::lineage::Lineage;
use claw10_domain::memory::Memory;
use claw10_domain::mission::Mission;
use claw10_domain::policy::PolicyBundle;
use claw10_domain::skill::Skill;
use claw10_domain::task::Task;
use claw10_domain::worker::Worker;

fn fmt_id<T: std::fmt::Debug>(id: &T) -> String {
    format!("{:?}", id)
}

// Helper untuk melakukan escaping dan quoting pada string agar sesuai standar TOON
fn encode_string(val: &str) -> String {
    if val.contains(',') || val.contains('"') || val.contains('\n') || val.trim() != val {
        format!("\"{}\"", val.replace('"', "\\\"").replace('\n', "\\n"))
    } else {
        val.to_string()
    }
}

// Helper untuk merepresentasikan primitive array ke format TOON (inline terpisah koma)
fn encode_primitive_array(name: &str, items: &[String]) -> String {
    if items.is_empty() {
        format!("{}: []", name)
    } else {
        let formatted_items: Vec<String> = items.iter().map(|item| encode_string(item)).collect();
        format!("{}[{}]: {}", name, items.len(), formatted_items.join(","))
    }
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
    // Mengkodekan objek tunggal Task
    pub fn encode_task(task: &Task) -> String {
        let deadline = task
            .deadline
            .map(|d| d.format("%Y-%m-%dT%H:%M:%SZ").to_string())
            .unwrap_or_else(|| "none".to_string());

        vec![
            format!("id: {}", fmt_id(&task.id)),
            format!("objective: {}", encode_string(&task.objective)),
            format!("state: {:?}", task.state),
            format!("risk: {:?}", task.risk),
            format!("deadline: {}", deadline),
        ]
        .join("\n")
    }

    // Mengkodekan objek tunggal Mission
    pub fn encode_mission(mission: &Mission) -> String {
        vec![
            format!("id: {}", fmt_id(&mission.id)),
            format!("objective: {}", encode_string(&mission.objective)),
            format!("mode: {:?}", mission.lifecycle_mode),
        ]
        .join("\n")
    }

    // Mengkodekan list Memory menjadi format Tabular Array TOON
    pub fn encode_memories(memories: &[Memory]) -> String {
        if memories.is_empty() {
            return "memories: []".to_string();
        }
        let mut output = format!("memories[{}]{{content,type,confidence}}:", memories.len());
        for m in memories {
            write!(
                output,
                "\n  {},{:?},{:.2}",
                encode_string(&m.content),
                m.memory_type,
                m.confidence
            )
            .unwrap();
        }
        output
    }

    // Mengkodekan summary Policy menjadi format Tabular Array TOON
    pub fn encode_policy_summary(bundles: &[PolicyBundle]) -> String {
        let mut active_rules = Vec::new();
        for bundle in bundles {
            if bundle.is_active {
                for rule in &bundle.rules {
                    active_rules.push((
                        bundle.id.clone(),
                        rule.effect.clone(),
                        rule.action.clone(),
                        rule.resource.clone(),
                    ));
                }
            }
        }

        if active_rules.is_empty() {
            return "policies: []".to_string();
        }

        let mut output = format!(
            "policies[{}]{{bundle_id,effect,action,resource}}:",
            active_rules.len()
        );
        for (bundle_id, effect, action, resource) in active_rules {
            let effect_str = if matches!(effect, claw10_domain::policy::PolicyEffect::Allow) {
                "ALLOW"
            } else {
                "DENY"
            };
            write!(
                output,
                "\n  {},{},{},{}",
                fmt_id(&bundle_id),
                effect_str,
                encode_string(&action),
                encode_string(&resource)
            )
            .unwrap();
        }
        output
    }

    // Mengkodekan daftar agen menjadi format Tabular Array TOON
    pub fn encode_agent_roster(agents: &[Agent]) -> String {
        if agents.is_empty() {
            return "agents: []".to_string();
        }
        let mut output = format!("agents[{}]{{id,role,state}}:", agents.len());
        for agent in agents {
            write!(
                output,
                "\n  {},{},{:?}",
                fmt_id(&agent.id),
                encode_string(&agent.role),
                agent.state
            )
            .unwrap();
        }
        output
    }

    // Mengkodekan data silsilah keturunan Lineage beserta entri hierarkinya
    pub fn encode_lineage(lineage: &Lineage) -> String {
        let root = format!("root_agent_id: {}", fmt_id(&lineage.root_agent_id));
        let entries_str = if lineage.entries.is_empty() {
            "entries: []".to_string()
        } else {
            let mut output = format!(
                "entries[{}]{{agent_id,parent_agent_id,state}}:",
                lineage.entries.len()
            );
            for entry in &lineage.entries {
                let parent = entry
                    .parent_agent_id
                    .as_ref()
                    .map_or("none".to_string(), fmt_id);
                write!(
                    output,
                    "\n  {},{},{:?}",
                    fmt_id(&entry.agent_id),
                    parent,
                    entry.state
                )
                .unwrap();
            }
            output
        };
        format!("{}\n{}", root, entries_str)
    }

    // Mengkodekan bukti hasil kerja menjadi format Tabular Array TOON
    pub fn encode_evidence(evidence: &[Evidence]) -> String {
        if evidence.is_empty() {
            return "evidence: []".to_string();
        }
        let mut output = format!("evidence[{}]{{id,type,accepted}}:", evidence.len());
        for ev in evidence {
            write!(
                output,
                "\n  {},{:?},{}",
                fmt_id(&ev.id),
                ev.evidence_type,
                ev.accepted
            )
            .unwrap();
        }
        output
    }

    // Mengkodekan skill terdaftar menjadi format Tabular Array TOON
    pub fn encode_skills(skills: &[Skill]) -> String {
        if skills.is_empty() {
            return "skills: []".to_string();
        }
        let mut output = format!("skills[{}]{{name,version,state,cost}}:", skills.len());
        for skill in skills {
            write!(
                output,
                "\n  {},{},{:?},{:.2}",
                encode_string(&skill.name),
                encode_string(&skill.version),
                skill.state,
                skill.cost_profile.estimated_cost_usd
            )
            .unwrap();
        }
        output
    }

    // Mengkodekan riwayat pesan obrolan (Primitive Array)
    pub fn encode_history(history: &[String]) -> String {
        encode_primitive_array("history", history)
    }

    // Mengkodekan daftar worker terdaftar menjadi format Tabular Array TOON
    pub fn encode_workers(workers: &[Worker]) -> String {
        if workers.is_empty() {
            return "workers: []".to_string();
        }
        let mut output = format!("workers[{}]{{name,type,state}}:", workers.len());
        for w in workers {
            write!(
                output,
                "\n  {},{:?},{:?}",
                encode_string(&w.name),
                w.worker_type,
                w.state
            )
            .unwrap();
        }
        output
    }

    // Mengkodekan daftar tool terdaftar (Primitive Array)
    pub fn encode_tools(tools: &[String]) -> String {
        encode_primitive_array("tools", tools)
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
