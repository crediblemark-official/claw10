use super::*;
use serde_json::json;
use crate::bundle::{ToolDefinition, PolicyIrInput};

fn make_request() -> PromptBuildRequest {
    PromptBuildRequest {
        agent: crate::bundle::AgentPromptInput {
            id: "agent-1".into(),
            role: "root".into(),
            lifecycle_mode: "ephemeral".into(),
            organization_id: "org-1".into(),
            memory_scopes: vec!["global".into()],
        },
        mission: crate::bundle::MissionPromptInput {
            id: "mission-1".into(),
            objective: "Build a web dashboard".into(),
            scope: Some("Frontend only".into()),
            status: "active".into(),
            risk_level: "low".into(),
        },
        task: crate::bundle::TaskPromptInput {
            id: "task-1".into(),
            objective: "Design API routes".into(),
            status: "assigned".into(),
            deadline: None,
            acceptance_criteria: vec!["routes defined".into()],
            required_evidence: vec!["route list".into()],
        },
        memories: vec![crate::bundle::MemoryPromptInput {
            content: "Use Axum for HTTP".into(),
            memory_type: "semantic".into(),
            confidence: 0.9,
        }],
        team: vec![crate::bundle::TeamMemberInput {
            id: "agent-2".into(),
            role: "specialist".into(),
            status: "active".into(),
            objective: Some("Build auth".into()),
        }],
        budget: crate::bundle::BudgetPromptInput {
            allocated: 100.0,
            spent: 30.0,
            remaining: 70.0,
            reserved: 10.0,
        },
        tools: vec![ToolDefinition {
            name: "http_client".into(),
            description: "Make HTTP requests".into(),
            input_schema: json!({}),
        }],
        policy_ir: PolicyIrInput {
            id: "policy-1".into(),
            hash: "abc123".into(),
            rules: String::new(),
        },
        model_profile: "claude-opus".into(),
        output_contract: OutputContractInput {
            output_type: "MissionProposal".into(),
            schema: json!({}),
        },
    }
}

#[test]
fn test_build_prompt_bundle() {
    let mut assembler = PromptAssembler::new();
    let request = make_request();
    let bundle = assembler.build(request).unwrap();

    assert!(!bundle.system_messages.is_empty(), "should have system messages");
    assert!(bundle.system_messages.iter().any(|m| m.contains("Root Agent")), "should contain root role prompt");
    assert!(bundle.system_messages.iter().any(|m| m.contains("logical agent")), "should contain kernel prompt");
    assert!(bundle.system_messages.iter().any(|m| m.contains("ephemeral")), "should contain lifecycle prompt");
    assert!(!bundle.context_message.is_empty(), "should have context");
    assert!(bundle.context_message.contains("Build a web dashboard"), "context should contain mission objective");
}

#[test]
fn test_build_with_director_role() {
    let mut assembler = PromptAssembler::new();
    let mut request = make_request();
    request.agent.role = "director".into();
    let bundle = assembler.build(request).unwrap();
    assert!(bundle.system_messages.iter().any(|m| m.contains("Director Agent")));
}

#[test]
fn test_validate_response_valid() {
    let response = json!({
        "objective": "Build dashboard",
        "scope": "Frontend",
        "exclusions": [],
        "workstreams": [],
        "success_criteria": ["done"],
        "risks": [],
        "termination_conditions": []
    });
    let contract = OutputContractInput {
        output_type: "MissionProposal".into(),
        schema: contracts::mission_proposal_schema(),
    };
    let outcome = PromptAssembler::validate_response(&response, &contract);
    assert!(outcome.valid);
}

#[test]
fn test_validate_response_invalid() {
    let response = json!({"objective": "Build dashboard"});
    let contract = OutputContractInput {
        output_type: "MissionProposal".into(),
        schema: contracts::mission_proposal_schema(),
    };
    let outcome = PromptAssembler::validate_response(&response, &contract);
    assert!(!outcome.valid);
}

#[test]
fn test_toon_context() {
    let request = make_request();
    let context = build_toon_context(&request);
    assert!(context.starts_with("[TOON v1]"));
    assert!(context.contains("[mission]"));
    assert!(context.contains("[task]"));
    assert!(context.contains("[memory]"));
    assert!(context.contains("[team]"));
    assert!(context.contains("[budget]"));
}

#[test]
fn test_build_with_version() {
    let mut assembler = PromptAssembler::new().with_version("2.0.0");
    let request = make_request();
    let bundle = assembler.build(request).unwrap();
    assert_eq!(bundle.metadata.prompt_version, "2.0.0");
}

#[test]
fn test_json_context_format_when_empty() {
    let mut assembler = PromptAssembler::new();
    let mut request = make_request();
    request.memories = vec![];
    request.team = vec![];
    request.task.objective = "".into();
    let bundle = assembler.build(request).unwrap();
    assert_eq!(bundle.metadata.context_format, crate::bundle::ContextFormat::Json);
}

#[test]
fn test_build_includes_injection_prompt() {
    let mut assembler = PromptAssembler::new();
    let request = make_request();
    let bundle = assembler.build(request).unwrap();
    assert!(
        bundle.system_messages.iter().any(|m| m.contains("injection") || m.contains("Instruction")),
        "injection safety prompt should be included"
    );
}

#[test]
fn test_build_includes_policy_digest() {
    let mut assembler = PromptAssembler::new();
    let request = make_request();
    let bundle = assembler.build(request).unwrap();
    assert!(
        bundle.system_messages.iter().any(|m| m.contains("Policy Digest") || m.contains("policy")),
        "policy digest should be included"
    );
}

#[test]
fn test_build_with_persistent_lifecycle() {
    let mut assembler = PromptAssembler::new();
    let mut request = make_request();
    request.agent.lifecycle_mode = "persistent".into();
    let bundle = assembler.build(request).unwrap();
    assert!(
        bundle.system_messages.iter().any(|m| m.contains("persistent")),
        "should contain persistent lifecycle prompt"
    );
}

#[test]
fn test_build_with_all_roles() {
    for role in &[
        "root", "director", "planner", "orchestrator", "manager",
        "specialist", "research", "browser", "coding", "data",
        "communication", "device", "critic", "verifier", "judge",
        "security_guardian", "memory_curator", "skill_engineer",
        "cost_controller", "recovery", "watcher", "maintenance",
    ] {
        let mut assembler = PromptAssembler::new();
        let mut request = make_request();
        request.agent.role = role.to_string();
        let bundle = assembler.build(request).unwrap();
        assert!(
            bundle.system_messages.iter().any(|m| m.contains("Agent")),
            "role {} should produce a valid bundle",
            role
        );
    }
}

#[test]
fn test_build_with_tools_included() {
    let mut assembler = PromptAssembler::new();
    let request = make_request();
    let bundle = assembler.build(request).unwrap();
    assert_eq!(bundle.tools.len(), 1);
    assert_eq!(bundle.tools[0].name, "http_client");
}

#[test]
fn test_bundle_metadata_populated() {
    let mut assembler = PromptAssembler::new();
    let request = make_request();
    let bundle = assembler.build(request).unwrap();
    assert_eq!(bundle.metadata.agent_id, "agent-1");
    assert_eq!(bundle.metadata.agent_role, "root");
    assert_eq!(bundle.metadata.mission_id, "mission-1");
    assert_eq!(bundle.metadata.task_id, "task-1");
    assert_eq!(bundle.metadata.policy_bundle_id, "policy-1");
    assert_eq!(bundle.metadata.policy_hash, "abc123");
}

#[test]
fn test_json_context_output() {
    let request = make_request();
    let context = build_json_context(&request);
    let parsed: serde_json::Value = serde_json::from_str(&context).unwrap();
    assert_eq!(parsed["agent"]["role"], "root");
    assert_eq!(parsed["mission"]["objective"], "Build a web dashboard");
    assert_eq!(parsed["task"]["objective"], "Design API routes");
    assert_eq!(parsed["budget"]["allocated"], 100.0);
}

#[test]
fn test_estimate_input_tokens_via_assembler() {
    let mut assembler = PromptAssembler::new();
    let request = make_request();
    let bundle = assembler.build(request).unwrap();
    let estimated = bundle.estimate_input_tokens();
    assert!(estimated > 0, "estimated tokens should be positive");
    let total_bytes = bundle.system_messages.iter().map(|s| s.len()).sum::<usize>()
        + bundle.context_message.len();
    assert!(
        estimated >= (total_bytes / 4) as u32,
        "token estimate {} seems too low for {} bytes",
        estimated,
        total_bytes
    );
}
