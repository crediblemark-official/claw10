use claw10_auth::credential::CredentialService;
use claw10_domain::{Agent, AgentState, Credential, Lineage, Task, TaskState};
use claw10_lineage::LineageService;

use crate::error::SpawnError;

/// DescendantManager handles termination propagation from parent to descendants.
/// Implements PRD section 32.2 - Descendant Teardown.
pub struct DescendantManager;

impl DescendantManager {
    /// Handle parent termination by freezing all descendants.
    #[must_use]
    pub fn freeze_descendants<'a>(parent_id: &str, all_agents: &'a [Agent]) -> Vec<&'a Agent> {
        let mut frozen = Vec::new();

        for agent in all_agents {
            if let Some(pid) = &agent.parent_agent_id
                && pid.0.to_string() == parent_id
                && agent.state != AgentState::Terminated
            {
                frozen.push(agent);
            }
        }

        frozen
    }

    /// Recursively collect all descendants of an agent.
    #[must_use]
    pub fn collect_all_descendants<'a>(agent_id: &str, all_agents: &'a [Agent]) -> Vec<&'a Agent> {
        let mut descendants = Vec::new();

        for agent in all_agents {
            if let Some(pid) = &agent.parent_agent_id
                && pid.0.to_string() == agent_id
            {
                descendants.push(agent);
                let sub = Self::collect_all_descendants(&agent.id.0.to_string(), all_agents);
                descendants.extend(sub);
            }
        }

        descendants
    }

    /// Collect descendant agent IDs as strings for fast lookup.
    fn descendant_ids(descendants: &[&Agent]) -> Vec<String> {
        descendants.iter().map(|a| a.id.0.to_string()).collect()
    }

    /// Handle descendant tasks - cancel active tasks, preserve completed ones.
    #[must_use]
    pub fn handle_descendant_tasks(descendants: &[&Agent], tasks: &[Task]) -> Vec<(String, String)> {
        let mut results = Vec::new();
        let ids = Self::descendant_ids(descendants);

        for task in tasks {
            if ids.contains(&task.owner_id.0.to_string()) {
                let action = match task.state {
                    TaskState::Running
                    | TaskState::Claimed
                    | TaskState::Ready
                    | TaskState::Created => "cancelled",
                    _ => "preserved",
                };
                results.push((task.id.0.to_string(), action.to_string()));
            }
        }

        results
    }

    /// Revoke all credentials belonging to descendant agents.
    pub fn revoke_descendant_credentials(
        descendants: &[&Agent],
        credentials: &mut [Credential],
    ) -> Vec<String> {
        let ids: Vec<String> = descendants
            .iter()
            .map(|a| a.identity_id.0.to_string())
            .collect();
        let mut revoked = Vec::new();

        for credential in credentials.iter_mut() {
            if ids.contains(&credential.identity_id.0.to_string())
                && credential.revoked_at.is_none()
            {
                CredentialService::revoke_credential(credential);
                revoked.push(credential.id.0.to_string());
            }
        }

        revoked
    }

    /// Mark all descendant entries in the lineage as terminated.
    pub fn cleanup_lineage_entries(descendants: &[&Agent], lineage: &mut Lineage) -> Vec<String> {
        let mut cleaned = Vec::new();

        for agent in descendants {
            LineageService::terminate_entry(lineage, &agent.id);
            cleaned.push(agent.id.0.to_string());
        }

        cleaned
    }

    /// Full teardown pipeline: freeze → tasks → credentials → lineage.
    /// Returns a summary of all actions taken.
    pub fn full_teardown(
        parent: &Agent,
        all_agents: &[Agent],
        tasks: &[Task],
        credentials: &mut [Credential],
        lineage: &mut Lineage,
    ) -> Result<serde_json::Value, SpawnError> {
        let descendants = Self::collect_all_descendants(&parent.id.0.to_string(), all_agents);

        if descendants.is_empty() {
            return Ok(serde_json::json!({
                "parent_id": parent.id.0.to_string(),
                "status": "no_descendants",
            }));
        }

        let task_results = Self::handle_descendant_tasks(&descendants, tasks);
        let revoked = Self::revoke_descendant_credentials(&descendants, credentials);
        let lineage_cleaned = Self::cleanup_lineage_entries(&descendants, lineage);

        let cancelled = task_results
            .iter()
            .filter(|(_, a)| a == "cancelled")
            .count();
        let preserved = task_results
            .iter()
            .filter(|(_, a)| a == "preserved")
            .count();

        Ok(serde_json::json!({
            "parent_id": parent.id.0.to_string(),
            "descendants_frozen": descendants.len(),
            "tasks_affected": task_results.len(),
            "tasks_cancelled": cancelled,
            "tasks_preserved": preserved,
            "credentials_revoked": revoked.len(),
            "lineage_entries_cleaned": lineage_cleaned.len(),
            "status": "teardown_complete",
        }))
    }

    /// Generate termination summary for parent kill operation.
    #[must_use]
    pub fn termination_summary(
        parent: &Agent,
        all_agents: &[Agent],
        tasks: &[Task],
    ) -> serde_json::Value {
        let descendants = Self::collect_all_descendants(&parent.id.0.to_string(), all_agents);
        let task_results = Self::handle_descendant_tasks(&descendants, tasks);

        serde_json::json!({
            "parent_id": parent.id.0.to_string(),
            "descendants_frozen": descendants.len(),
            "tasks_affected": task_results.len(),
            "tasks_cancelled": task_results.iter().filter(|(_, a)| a == "cancelled").count(),
            "tasks_preserved": task_results.iter().filter(|(_, a)| a == "preserved").count(),
        })
    }
}
