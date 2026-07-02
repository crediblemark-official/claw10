use serde_json::{Value, json};

#[must_use]
pub fn mission_proposal_schema() -> Value {
    json!({
        "type": "object",
        "required": ["objective", "scope", "exclusions", "workstreams", "success_criteria", "risks", "termination_conditions"],
        "properties": {
            "objective": {"type": "string"},
            "scope": {"type": "string"},
            "exclusions": {"type": "array", "items": {"type": "string"}},
            "workstreams": {"type": "array", "items": {"type": "object"}},
            "success_criteria": {"type": "array", "items": {"type": "string"}},
            "risks": {"type": "array", "items": {"type": "object"}},
            "termination_conditions": {"type": "array", "items": {"type": "string"}}
        }
    })
}

#[must_use]
pub fn task_graph_proposal_schema() -> Value {
    json!({
        "type": "object",
        "required": ["tasks"],
        "properties": {
            "tasks": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["id", "title", "objective", "dependencies", "required_role", "estimated_cost", "risk"],
                    "properties": {
                        "id": {"type": "string"},
                        "title": {"type": "string"},
                        "objective": {"type": "string"},
                        "dependencies": {"type": "array", "items": {"type": "string"}},
                        "required_role": {"type": "string"},
                        "estimated_cost": {"type": "number"},
                        "risk": {"type": "string"}
                    }
                }
            }
        }
    })
}

#[must_use]
pub fn spawn_proposal_schema() -> Value {
    json!({
        "type": "object",
        "required": ["reason", "expected_benefit", "team", "children", "spawn_controls"],
        "properties": {
            "reason": {"type": "string"},
            "expected_benefit": {"type": "string"},
            "team": {
                "type": "object",
                "required": ["name", "lifecycle", "objective", "termination_condition"],
                "properties": {
                    "name": {"type": "string"},
                    "lifecycle": {"type": "string"},
                    "objective": {"type": "string"},
                    "termination_condition": {"type": "string"}
                }
            },
            "children": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["role", "objective", "lifecycle", "requested_budget", "output_contract"],
                    "properties": {
                        "role": {"type": "string"},
                        "objective": {"type": "string"},
                        "lifecycle": {"type": "string"},
                        "requested_budget": {"type": "number"},
                        "output_contract": {"type": "string"}
                    }
                }
            },
            "spawn_controls": {
                "type": "object",
                "required": ["allow_child_spawn", "requested_max_depth"],
                "properties": {
                    "allow_child_spawn": {"type": "boolean"},
                    "requested_max_depth": {"type": "integer"}
                }
            }
        }
    })
}

#[must_use]
pub fn work_result_schema() -> Value {
    json!({
        "type": "object",
        "required": ["status", "summary", "outputs", "evidence", "confidence"],
        "properties": {
            "status": {"type": "string"},
            "summary": {"type": "string"},
            "outputs": {"type": "array", "items": {"type": "object"}},
            "evidence": {"type": "array", "items": {"type": "object"}},
            "confidence": {"type": "number"},
            "risks": {"type": "array", "items": {"type": "string"}}
        }
    })
}

#[must_use]
pub fn verification_decision_schema() -> Value {
    json!({
        "type": "object",
        "required": ["decision", "reason"],
        "properties": {
            "decision": {"type": "string", "enum": ["accepted", "conditionally_accepted", "revision_required", "rejected"]},
            "reason": {"type": "string"},
            "criteria_results": {"type": "array", "items": {"type": "object"}}
        }
    })
}

#[must_use]
pub fn final_handoff_schema() -> Value {
    json!({
        "type": "object",
        "required": ["agent_id", "task_id", "status", "summary"],
        "properties": {
            "agent_id": {"type": "string"},
            "task_id": {"type": "string"},
            "status": {"type": "string"},
            "summary": {"type": "string"},
            "outputs": {"type": "array", "items": {"type": "object"}},
            "artifacts": {"type": "array", "items": {"type": "object"}},
            "evidence": {"type": "array", "items": {"type": "object"}},
            "open_risks": {"type": "array", "items": {"type": "string"}},
            "memory_candidates": {"type": "array", "items": {"type": "string"}},
            "skill_candidates": {"type": "array", "items": {"type": "string"}}
        }
    })
}

#[must_use]
pub fn checkpoint_schema() -> Value {
    json!({
        "type": "object",
        "required": ["agent_id", "version", "created_at", "responsibilities"],
        "properties": {
            "agent_id": {"type": "string"},
            "version": {"type": "string"},
            "created_at": {"type": "string"},
            "responsibilities": {"type": "array", "items": {"type": "string"}},
            "active_tasks": {"type": "array", "items": {"type": "object"}},
            "active_children": {"type": "array", "items": {"type": "object"}},
            "subscriptions": {"type": "array", "items": {"type": "string"}},
            "budget": {"type": "object"},
            "next_wake_condition": {"type": "string"}
        }
    })
}

#[must_use]
pub fn get_schema(output_type: &str) -> Option<Value> {
    match output_type {
        "MissionProposal" => Some(mission_proposal_schema()),
        "TaskGraphProposal" => Some(task_graph_proposal_schema()),
        "SpawnProposal" => Some(spawn_proposal_schema()),
        "WorkResult" => Some(work_result_schema()),
        "VerificationDecision" => Some(verification_decision_schema()),
        "FinalHandoff" => Some(final_handoff_schema()),
        "Checkpoint" => Some(checkpoint_schema()),
        _ => None,
    }
}
