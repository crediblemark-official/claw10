#![allow(clippy::pedantic)]
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

pub enum ContextOutput {
    Toon(String),
    Json(String),
}

impl ContextOutput {
    #[must_use]
    pub fn to_string(&self) -> &str {
        match self {
            ContextOutput::Toon(s) | ContextOutput::Json(s) => s,
        }
    }
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

    pub fn build(&self) -> Result<String, ToonError> {
        let mut output = String::from("[TOON v1]");
        for section in &self.sections {
            write!(output, "{}", section)
                .map_err(|e| ToonError::Encoding(e.to_string()))?;
        }
        Ok(output)
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
    pub fn encode_task(task: &Task) -> Result<String, ToonError> {
        let deadline = task
            .deadline
            .map(|d| d.format("%Y-%m-%dT%H:%M:%SZ").to_string())
            .unwrap_or_else(|| "none".to_string());

        Ok(vec![
            format!("id: {}", fmt_id(&task.id)),
            format!("objective: {}", encode_string(&task.objective)),
            format!("state: {:?}", task.state),
            format!("risk: {:?}", task.risk),
            format!("deadline: {}", deadline),
        ]
        .join("\n"))
    }

    // Mengkodekan objek tunggal Mission
    pub fn encode_mission(mission: &Mission) -> Result<String, ToonError> {
        Ok(vec![
            format!("id: {}", fmt_id(&mission.id)),
            format!("objective: {}", encode_string(&mission.objective)),
            format!("mode: {:?}", mission.lifecycle_mode),
        ]
        .join("\n"))
    }

    // Mengkodekan list Memory menjadi format Tabular Array TOON
    pub fn encode_memories(memories: &[Memory]) -> Result<String, ToonError> {
        if memories.is_empty() {
            return Ok("memories: []".to_string());
        }
        let mut output = format!("memories[{}]{{content,type,confidence}}:", memories.len());
        for m in memories {
            write!(
                output,
                "\n  {},{},{:.2}",
                encode_string(&m.content),
                m.memory_type,
                m.confidence
            )
            .map_err(|e| ToonError::Encoding(e.to_string()))?;
        }
        Ok(output)
    }

    // Mengkodekan summary Policy menjadi format Tabular Array TOON
    pub fn encode_policy_summary(bundles: &[PolicyBundle]) -> Result<String, ToonError> {
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
            return Ok("policies: []".to_string());
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
            .map_err(|e| ToonError::Encoding(e.to_string()))?;
        }
        Ok(output)
    }

    // Mengkodekan daftar agen menjadi format Tabular Array TOON
    pub fn encode_agent_roster(agents: &[Agent]) -> Result<String, ToonError> {
        if agents.is_empty() {
            return Ok("agents: []".to_string());
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
            .map_err(|e| ToonError::Encoding(e.to_string()))?;
        }
        Ok(output)
    }

    // Mengkodekan data silsilah keturunan Lineage beserta entri hierarkinya
    pub fn encode_lineage(lineage: &Lineage) -> Result<String, ToonError> {
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
                .map_err(|e| ToonError::Encoding(e.to_string()))?;
            }
            output
        };
        Ok(format!("{}\n{}", root, entries_str))
    }

    // Mengkodekan bukti hasil kerja menjadi format Tabular Array TOON
    pub fn encode_evidence(evidence: &[Evidence]) -> Result<String, ToonError> {
        if evidence.is_empty() {
            return Ok("evidence: []".to_string());
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
            .map_err(|e| ToonError::Encoding(e.to_string()))?;
        }
        Ok(output)
    }

    // Mengkodekan skill terdaftar menjadi format Tabular Array TOON
    pub fn encode_skills(skills: &[Skill]) -> Result<String, ToonError> {
        if skills.is_empty() {
            return Ok("skills: []".to_string());
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
            .map_err(|e| ToonError::Encoding(e.to_string()))?;
        }
        Ok(output)
    }

    // Mengkodekan riwayat pesan obrolan (Primitive Array)
    pub fn encode_history(history: &[String]) -> Result<String, ToonError> {
        Ok(encode_primitive_array("history", history))
    }

    // Mengkodekan daftar worker terdaftar menjadi format Tabular Array TOON
    pub fn encode_workers(workers: &[Worker]) -> Result<String, ToonError> {
        if workers.is_empty() {
            return Ok("workers: []".to_string());
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
            .map_err(|e| ToonError::Encoding(e.to_string()))?;
        }
        Ok(output)
    }

    // Mengkodekan daftar tool terdaftar (Primitive Array)
    pub fn encode_tools(tools: &[String]) -> Result<String, ToonError> {
        Ok(encode_primitive_array("tools", tools))
    }

    pub fn build_context(
        task: Option<&Task>,
        mission: Option<&Mission>,
        memories: &[Memory],
        policies: &[PolicyBundle],
        agents: &[Agent],
        lineage: Option<&Lineage>,
        evidence: &[Evidence],
    ) -> Result<String, ToonError> {
        let mut ctx = ToonContext::new();

        if let Some(task) = task {
            ctx.add_section("task", Self::encode_task(task)?);
        }

        if let Some(mission) = mission {
            ctx.add_section("mission", Self::encode_mission(mission)?);
        }

        if !memories.is_empty() {
            ctx.add_section("memory", Self::encode_memories(memories)?);
        }

        if !policies.is_empty() {
            ctx.add_section("policy", Self::encode_policy_summary(policies)?);
        }

        if !agents.is_empty() {
            ctx.add_section("agents", Self::encode_agent_roster(agents)?);
        }

        if let Some(lineage) = lineage {
            ctx.add_section("lineage", Self::encode_lineage(lineage)?);
        }

        if !evidence.is_empty() {
            ctx.add_section("evidence", Self::encode_evidence(evidence)?);
        }

        ctx.build()
    }

    pub fn suitability_score(
        task: Option<&Task>,
        mission: Option<&Mission>,
        memories: &[Memory],
        policies: &[PolicyBundle],
        agents: &[Agent],
        lineage: Option<&Lineage>,
        evidence: &[Evidence],
    ) -> f64 {
        let mut score: f64 = 0.0;

        let is_tabular_only = task.is_none()
            && mission.is_none()
            && lineage.is_none()
            && evidence.is_empty()
            && policies.is_empty();
        if is_tabular_only {
            score += 0.3;
        }

        let mut ctx = ToonContext::new();
        if let Some(task) = task {
            if let Ok(s) = Self::encode_task(task) {
                ctx.add_section("task", s);
            }
        }
        if let Some(mission) = mission {
            if let Ok(s) = Self::encode_mission(mission) {
                ctx.add_section("mission", s);
            }
        }
        if !memories.is_empty() {
            if let Ok(s) = Self::encode_memories(memories) {
                ctx.add_section("memory", s);
            }
        }
        if !policies.is_empty() {
            if let Ok(s) = Self::encode_policy_summary(policies) {
                ctx.add_section("policy", s);
            }
        }
        if !agents.is_empty() {
            if let Ok(s) = Self::encode_agent_roster(agents) {
                ctx.add_section("agents", s);
            }
        }
        if let Some(lineage) = lineage {
            if let Ok(s) = Self::encode_lineage(lineage) {
                ctx.add_section("lineage", s);
            }
        }
        if !evidence.is_empty() {
            if let Ok(s) = Self::encode_evidence(evidence) {
                ctx.add_section("evidence", s);
            }
        }

        let toon_str = ctx.build().unwrap_or_default();

        if toon_str.len() < 4096 {
            score += 0.2;
        }

        let brace_count = toon_str.matches('{').count() + toon_str.matches('}').count();
        let bracket_count = toon_str.matches('[').count() + toon_str.matches(']').count();
        if brace_count > 10 || bracket_count > 10 {
            score -= 0.3;
        }

        let special_chars = toon_str
            .chars()
            .filter(|c| !c.is_alphanumeric() && !c.is_whitespace() && *c != '_' && *c != '-' && *c != '.')
            .count();
        let total_chars = toon_str.len();
        if total_chars > 0 {
            let special_ratio = special_chars as f64 / total_chars as f64;
            if special_ratio > 0.3 {
                score -= 0.2;
            }
        }

        score.clamp(0.0, 1.0)
    }

    pub fn build_context_with_fallback(
        task: Option<&Task>,
        mission: Option<&Mission>,
        memories: &[Memory],
        policies: &[PolicyBundle],
        agents: &[Agent],
        lineage: Option<&Lineage>,
        evidence: &[Evidence],
    ) -> Result<ContextOutput, ToonError> {
        let score = Self::suitability_score(
            task,
            mission,
            memories,
            policies,
            agents,
            lineage,
            evidence,
        );

        if score < 0.5 {
            let fallback_data = serde_json::json!({
                "task": task.and_then(|t| serde_json::to_value(t).ok()),
                "mission": mission.and_then(|m| serde_json::to_value(m).ok()),
                "memories": serde_json::to_value(memories).ok(),
                "policies": serde_json::to_value(policies).ok(),
                "agents": serde_json::to_value(agents).ok(),
                "lineage": lineage.and_then(|l| serde_json::to_value(l).ok()),
                "evidence": serde_json::to_value(evidence).ok(),
            });

            match serde_json::to_string_pretty(&fallback_data) {
                Ok(json) => Ok(ContextOutput::Json(json)),
                Err(e) => Err(ToonError::Encoding(e.to_string())),
            }
        } else {
            let toon = Self::build_context(task, mission, memories, policies, agents, lineage, evidence)?;
            Ok(ContextOutput::Toon(toon))
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
