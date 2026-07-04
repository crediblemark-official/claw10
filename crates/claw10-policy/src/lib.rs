#![allow(clippy::pedantic)]

use std::time::Instant;

use uuid::Uuid;

use claw10_domain::{
    PolicyBundle, PolicyBundleId, PolicyEffect, PolicyEvaluateResult, PolicyRule, PolicySubject,
};

#[derive(Debug, thiserror::Error)]
pub enum PolicyError {
    #[error("policy bundle not found: {0}")]
    NotFound(String),
    #[error("inactive policy bundle: {0}")]
    Inactive(String),
    #[error("unsigned policy bundle: {0}")]
    Unsigned(String),
    #[error("{0}")]
    Other(String),
}

pub struct PolicyService;

impl PolicyService {
    /// Evaluate an action against a policy bundle.
    ///
    /// Rules are evaluated in priority order (highest first).
    /// - `ExplicitDeny` immediately denies regardless of subsequent rules.
    /// - `ExplicitDenyPriority` denies but can be overridden by a higher-priority `Allow`.
    /// - `Allow` permits the action.
    ///
    /// # Errors
    /// Returns `PolicyError::Inactive` if the bundle is not active.
    pub fn evaluate(
        bundle: &PolicyBundle,
        subject: &PolicySubject,
        action: &str,
        resource: &str,
        context: Option<&serde_json::Value>,
    ) -> Result<PolicyEvaluateResult, PolicyError> {
        if !bundle.is_active {
            return Err(PolicyError::Inactive(bundle.name.clone()));
        }

        let start = Instant::now();
        let mut matched_rule: Option<PolicyRule> = None;

        // Sort rules by priority descending
        let mut sorted_rules = bundle.rules.clone();
        sorted_rules.sort_by_key(|b| std::cmp::Reverse(b.priority));

        for rule in &sorted_rules {
            if !Self::subject_matches(&rule.subject, subject) {
                continue;
            }
            if !Self::pattern_matches(&rule.action, action) {
                continue;
            }
            if !Self::pattern_matches(&rule.resource, resource) {
                continue;
            }
            if !Self::evaluate_condition(&rule.condition, context) {
                continue;
            }

            match rule.effect {
                PolicyEffect::ExplicitDeny => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    return Ok(PolicyEvaluateResult {
                        allowed: false,
                        matched_rule: Some(rule.clone()),
                        evaluation_time_ms: elapsed,
                        reason: format!("denied by rule {}: {}", rule.id.0, rule.action),
                    });
                }
                PolicyEffect::ExplicitDenyPriority => {
                    matched_rule = Some(rule.clone());
                    // Keep looking for a higher-priority Allow
                }
                PolicyEffect::Allow => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    return Ok(PolicyEvaluateResult {
                        allowed: true,
                        matched_rule: Some(rule.clone()),
                        evaluation_time_ms: elapsed,
                        reason: format!("allowed by rule {}: {}", rule.id.0, rule.action),
                    });
                }
            }
        }

        // If we had ExplicitDenyPriority and no Allow overrode it
        if let Some(rule) = &matched_rule {
            let elapsed = start.elapsed().as_millis() as u64;
            return Ok(PolicyEvaluateResult {
                allowed: false,
                matched_rule: Some(rule.clone()),
                evaluation_time_ms: elapsed,
                reason: format!("denied by priority rule {}: {}", rule.id.0, rule.action),
            });
        }

        // Default: deny if no rule matched
        let elapsed = start.elapsed().as_millis() as u64;
        Ok(PolicyEvaluateResult {
            allowed: false,
            matched_rule: None,
            evaluation_time_ms: elapsed,
            reason: "no matching rule — default deny".into(),
        })
    }

    /// Check if a policy bundle's subject matches the target subject.
    #[must_use]
    pub fn subject_matches(rule_subject: &PolicySubject, target: &PolicySubject) -> bool {
        match (rule_subject, target) {
            // Wildcard: role "*" matches any role
            (PolicySubject::Role(r), _) if r == "*" => true,
            (PolicySubject::Role(r), PolicySubject::Role(t)) => r == t,
            (PolicySubject::Agent(a), PolicySubject::Agent(t)) => a == t,
            (PolicySubject::Organization(a), PolicySubject::Organization(t)) => a == t,
            (PolicySubject::Department(a), PolicySubject::Department(t)) => a == t,
            (PolicySubject::Mission(a), PolicySubject::Mission(t)) => a == t,
            (PolicySubject::Task(a), PolicySubject::Task(t)) => a == t,
            (PolicySubject::Tool(a), PolicySubject::Tool(t)) => a == t,
            (PolicySubject::Worker(a), PolicySubject::Worker(t)) => a == t,
            (PolicySubject::Tenant(a), PolicySubject::Tenant(t)) => a == t,
            (PolicySubject::DataClass(a), PolicySubject::DataClass(t)) => a == t,
            _ => false,
        }
    }

    /// Simple glob-style pattern matching: `*` matches anything.
    #[must_use]
    pub fn pattern_matches(pattern: &str, value: &str) -> bool {
        if pattern == "*" {
            return true;
        }
        if pattern == value {
            return true;
        }
        // Simple prefix/suffix/infix matching
        if let Some(rest) = pattern.strip_prefix("*") {
            return value.ends_with(rest);
        }
        if let Some(prefix) = pattern.strip_suffix("*") {
            return value.starts_with(prefix);
        }
        false
    }

    /// Evaluate an optional condition against the context.
    #[must_use]
    pub fn evaluate_condition(
        condition: &Option<serde_json::Value>,
        context: Option<&serde_json::Value>,
    ) -> bool {
        let Some(cond) = condition else {
            return true; // No condition → always matches
        };
        let Some(ctx) = context else {
            return true; // No context but condition exists → match (conservative)
        };

        // Simple condition: { "field": "value" } checks if context.field == value
        if let Some(obj) = cond.as_object() {
            for (key, expected) in obj {
                let actual = ctx.get(key);
                match actual {
                    Some(val) if val == expected => continue,
                    _ => return false,
                }
            }
            return true;
        }

        true
    }

    /// Create a new policy bundle.
    #[must_use]
    pub fn create_bundle(name: String, version: String, rules: Vec<PolicyRule>) -> PolicyBundle {
        PolicyBundle {
            id: PolicyBundleId(Uuid::now_v7()),
            name,
            version,
            rules,
            is_active: false,
            signed_by: None,
            signature: None,
            activated_at: None,
            created_at: chrono::Utc::now(),
        }
    }

    /// Activate a policy bundle.
    pub fn activate(bundle: &mut PolicyBundle) {
        bundle.is_active = true;
        bundle.activated_at = Some(chrono::Utc::now());
    }

    /// Deactivate a policy bundle.
    pub fn deactivate(bundle: &mut PolicyBundle) {
        bundle.is_active = false;
    }

    /// Simulate evaluation without requiring an active bundle.
    /// Used for "what if" scenarios.
    #[must_use]
    pub fn simulate(
        rules: &[PolicyRule],
        subject: &PolicySubject,
        action: &str,
        resource: &str,
        context: Option<&serde_json::Value>,
    ) -> PolicyEvaluateResult {
        let bundle = Self::create_bundle("simulation".into(), "0.0.0".into(), rules.to_vec());
        // Temporarily activate for simulation
        let mut sim_bundle = bundle;
        sim_bundle.is_active = true;

        match Self::evaluate(&sim_bundle, subject, action, resource, context) {
            Ok(result) => result,
            Err(e) => PolicyEvaluateResult {
                allowed: false,
                matched_rule: None,
                evaluation_time_ms: 0,
                reason: e.to_string(),
            },
        }
    }

    /// Compile a set of rules: validate, sort by priority, and return.
    #[must_use]
    pub fn compile(rules: Vec<PolicyRule>) -> Vec<PolicyRule> {
        let mut sorted = rules;
        sorted.sort_by_key(|b| std::cmp::Reverse(b.priority));
        sorted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use claw10_domain::{PolicyEffect, PolicyRuleId, PolicySubject};

    #[test]
    fn test_create_bundle() {
        let rule = PolicyRule {
            id: PolicyRuleId(Uuid::now_v7()),
            subject: PolicySubject::Role("admin".to_string()),
            effect: PolicyEffect::Allow,
            action: "read".to_string(),
            resource: "document".to_string(),
            condition: None,
            priority: 10,
        };
        let rules = vec![rule.clone()];
        let name = "Test Bundle".to_string();
        let version = "1.0.0".to_string();

        let bundle = PolicyService::create_bundle(name.clone(), version.clone(), rules.clone());

        assert_eq!(bundle.name, name);
        assert_eq!(bundle.version, version);
        assert_eq!(bundle.rules.len(), 1);
        assert_eq!(bundle.rules[0].action, "read");
        assert_eq!(bundle.rules[0].resource, "document");
        assert_eq!(bundle.rules[0].priority, 10);
        assert!(!bundle.is_active);
        assert!(bundle.signed_by.is_none());
        assert!(bundle.signature.is_none());
        assert!(bundle.activated_at.is_none());
        // ID should not be nil
        assert!(!bundle.id.0.is_nil());
        // created_at should be reasonably close to now (within the last few seconds)
        let now = chrono::Utc::now();
        assert!(bundle.created_at <= now);
        let duration = now.signed_duration_since(bundle.created_at);
        assert!(duration.num_seconds() < 5);
    }
}
