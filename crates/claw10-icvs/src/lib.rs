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
    /// # Errors
    /// Returns `IcvsError::Parse` if parsing fails, or `IcvsError::Validation` if invalid.
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
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; "),
            ));
        }

        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut targets: HashMap<String, Vec<String>> = HashMap::new();

        for (id, node) in doc.nodes {
            let node_type_str = node.node_type.as_str().to_string();
            let content = node.content;
            let condition = node.condition.map(|c| format!("{} {} {}", c.variable, c.operator, c.value));
            let mut properties = HashMap::new();
            if let Some(ref sev) = node.severity {
                properties.insert("severity".to_string(), sev.as_str().to_string());
            }
            let severity = node.severity.map(|s| s.as_str().to_string());

            nodes.push(IcvsNode {
                id,
                node_type: node_type_str,
                content,
                severity,
                condition,
                properties,
            });
        }

        for edge in doc.edges {
            edges.push(IcvsEdge {
                source: edge.source,
                target: edge.target,
                label: edge.label,
            });
        }

        for (name, target) in doc.targets {
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

    /// # Errors
    /// Returns `IcvsError::Parse` if parsing fails, or `IcvsError::MissingContent` if a node has no content.
    pub fn compile_policy(source: &str) -> Result<Vec<PolicyRule>, IcvsError> {
        let doc = icvs::parser::parse_document(source)
            .map_err(|e| IcvsError::Parse(e.to_string()))?;

        let mut rules = Vec::new();
        for (id, node) in doc.nodes {
            let is_policy_node = matches!(
                node.node_type,
                NodeType::Rule | NodeType::Blocklist | NodeType::Allowlist
            );

            if !is_policy_node {
                continue;
            }

            let content = node
                .content
                .ok_or(IcvsError::MissingContent(id))?;

            let effect = match node.node_type {
                NodeType::Blocklist | NodeType::Rule if matches!(node.severity, Some(Severity::Must)) => PolicyEffect::ExplicitDeny,
                NodeType::Blocklist => PolicyEffect::ExplicitDeny,
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

    /// # Errors
    /// Returns `IcvsError::Parse` if parsing fails, or `IcvsError::TargetNotFound` if a target is not found.
    pub fn compile_prompt(source: &str, target_name: &str) -> Result<Vec<AgentPrompt>, IcvsError> {
        let mut doc = icvs::parser::parse_document(source)
            .map_err(|e| IcvsError::Parse(e.to_string()))?;

        let target = doc
            .targets
            .remove(target_name)
            .ok_or(IcvsError::TargetNotFound(target_name.to_string()))?;

        let target_ids = target
            .resolve
            .ok_or(IcvsError::TargetNotFound(target_name.to_string()))?;

        let mut prompts = Vec::new();
        let mut edges_by_source: HashMap<String, Vec<String>> = HashMap::new();
        for edge in doc.edges {
            edges_by_source.entry(edge.source).or_default().push(edge.target);
        }

        for node_id in target_ids {
            let node = doc
                .nodes
                .remove(&node_id)
                .ok_or_else(|| IcvsError::TargetNotFound(node_id.clone()))?;

            if let Some(content) = node.content {
                let severity = match node.severity {
                    Some(Severity::Must) => PromptSeverity::Must,
                    Some(Severity::Should) => PromptSeverity::Should,
                    _ => PromptSeverity::May,
                };

                let deps = edges_by_source.remove(&node_id).unwrap_or_default();

                prompts.push(AgentPrompt {
                    id: node_id,
                    content,
                    severity,
                    dependencies: deps,
                });
            }
        }

        Ok(prompts)
    }

    /// # Errors
    /// Returns `IcvsError::Parse` if parsing fails, or `IcvsError::Validation` if invalid.
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
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; "),
            ))
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;

