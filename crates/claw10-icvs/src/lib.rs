use std::collections::HashMap;

use claw10_domain::policy::{PolicyEffect, PolicyRule};
use icvs::ast::{NodeType, Severity};

#[derive(Debug, thiserror::Error)]
pub enum IcvsError {
    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Missing content in node: {0}")]
    MissingContent(String),

    #[error("Target not found: {0}")]
    TargetNotFound(String),
}

#[derive(Debug, Clone)]
pub struct AgentPrompt {
    pub id: String,
    pub content: String,
    pub severity: PromptSeverity,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptSeverity {
    Must,
    Should,
    May,
}

#[derive(Debug, Clone)]
pub struct IcvsDocument {
    pub nodes: Vec<IcvsNode>,
    pub edges: Vec<IcvsEdge>,
    pub targets: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct IcvsNode {
    pub id: String,
    pub node_type: String,
    pub content: Option<String>,
    pub severity: Option<String>,
    pub condition: Option<String>,
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct IcvsEdge {
    pub source: String,
    pub target: String,
    pub label: Option<String>,
}

pub struct IcvsCompiler;

impl IcvsCompiler {
    pub fn parse(source: &str) -> Result<IcvsDocument, IcvsError> {
        let doc = icvs::parser::parse_document(source)
            .map_err(|e| IcvsError::Parse(e.to_string()))?;

        let report = icvs::validator::validate(&doc)
            .map_err(|e| IcvsError::Validation(e.to_string()))?;
        if !report.is_valid {
            return Err(IcvsError::Validation(
                report
                    .errors
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("; "),
            ));
        }

        let mut nodes = Vec::with_capacity(doc.nodes.len());
        let mut edges = Vec::with_capacity(doc.edges.len());
        let mut targets: HashMap<String, Vec<String>> = HashMap::with_capacity(doc.targets.len());

        for (id, node) in doc.nodes.into_iter() {
            let node_type_str = node.node_type.as_str().to_string();
            let content = node.content;
            let severity = node.severity.map(|s| s.as_str().to_string());
            let condition = node.condition.as_ref().map(|c| format!("{} {} {}", c.variable, c.operator, c.value));
            let mut properties = HashMap::new();
            if let Some(ref sev) = node.severity {
                properties.insert("severity".to_string(), sev.as_str().to_string());
            }

            nodes.push(IcvsNode {
                id,
                node_type: node_type_str,
                content,
                severity,
                condition,
                properties,
            });
        }

        for edge in doc.edges.into_iter() {
            edges.push(IcvsEdge {
                source: edge.source,
                target: edge.target,
                label: edge.label,
            });
        }

        for (name, target) in doc.targets.into_iter() {
            if let Some(resolve) = target.resolve {
                targets.insert(name, resolve);
            }
        }

        Ok(IcvsDocument {
            nodes,
            edges,
            targets,
        })
    }

    pub fn compile_policy(source: &str) -> Result<Vec<PolicyRule>, IcvsError> {
        let doc = icvs::parser::parse_document(source)
            .map_err(|e| IcvsError::Parse(e.to_string()))?;

        let mut rules = Vec::with_capacity(doc.nodes.len());
        for (id, node) in doc.nodes.into_iter() {
            let is_policy_node = matches!(
                node.node_type,
                NodeType::Rule | NodeType::Blocklist | NodeType::Allowlist
            );

            if !is_policy_node {
                continue;
            }

            let content = node
                .content
                .ok_or_else(|| IcvsError::MissingContent(id))?;

            let effect = match node.node_type {
                NodeType::Blocklist => PolicyEffect::ExplicitDeny,
                NodeType::Allowlist => PolicyEffect::Allow,
                NodeType::Rule => match node.severity {
                    Some(Severity::Must) => PolicyEffect::ExplicitDeny,
                    _ => PolicyEffect::Allow,
                },
                _ => PolicyEffect::Allow,
            };

            let rule = PolicyRule {
                id: claw10_domain::policy::PolicyRuleId(uuid::Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext))),
                subject: claw10_domain::policy::PolicySubject::Agent("*".to_string()),
                effect,
                action: "*".to_string(),
                resource: content,
                condition: None,
                priority: 100,
            };

            rules.push(rule);
        }

        Ok(rules)
    }

    pub fn compile_prompt(source: &str, target_name: &str) -> Result<Vec<AgentPrompt>, IcvsError> {
        let doc = icvs::parser::parse_document(source)
            .map_err(|e| IcvsError::Parse(e.to_string()))?;

        let target = doc
            .targets
            .get(target_name)
            .ok_or_else(|| IcvsError::TargetNotFound(target_name.to_string()))?;

        let target_ids = target
            .resolve
            .as_ref()
            .ok_or_else(|| IcvsError::TargetNotFound(target_name.to_string()))?;

        let mut prompts = Vec::new();

        for node_id in target_ids {
            let node = doc
                .nodes
                .get(node_id)
                .ok_or_else(|| IcvsError::TargetNotFound(node_id.clone()))?;

            if let Some(ref content) = node.content {
                let severity = match node.severity {
                    Some(Severity::Must) => PromptSeverity::Must,
                    Some(Severity::Should) => PromptSeverity::Should,
                    _ => PromptSeverity::May,
                };

                let deps: Vec<String> = doc
                    .edges
                    .iter()
                    .filter(|e| e.source == *node_id)
                    .map(|e| e.target.clone())
                    .collect();

                prompts.push(AgentPrompt {
                    id: node_id.clone(),
                    content: content.clone(),
                    severity,
                    dependencies: deps,
                });
            }
        }

        Ok(prompts)
    }

    pub fn validate(source: &str) -> Result<(), IcvsError> {
        let doc = icvs::parser::parse_document(source)
            .map_err(|e| IcvsError::Parse(e.to_string()))?;
        let report = icvs::validator::validate(&doc)
            .map_err(|e| IcvsError::Validation(e.to_string()))?;
        if report.is_valid {
            Ok(())
        } else {
            Err(IcvsError::Validation(
                report
                    .errors
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("; "),
            ))
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;

