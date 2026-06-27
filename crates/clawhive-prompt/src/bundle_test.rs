use super::*;
use serde_json::json;

fn sample_bundle() -> PromptBundle {
    PromptBundle {
        system_messages: vec!["You are a helpful agent.".into(), "Follow these rules.".into()],
        context_message: "Mission: Build API\nTask: Design routes".into(),
        response_schema: json!({
            "type": "object",
            "properties": {
                "plan": {"type": "string"}
            }
        }),
        tools: vec![ToolDefinition {
            name: "http".into(),
            description: "HTTP client".into(),
            input_schema: json!({}),
        }],
        metadata: PromptMetadata {
            prompt_bundle_id: "bundle-1".into(),
            prompt_version: "1.0.0".into(),
            agent_id: "agent-1".into(),
            agent_role: "root".into(),
            lifecycle_mode: "ephemeral".into(),
            mission_id: "mission-1".into(),
            task_id: "task-1".into(),
            policy_bundle_id: "policy-1".into(),
            policy_hash: "abc123".into(),
            context_format: ContextFormat::Toon,
        },
    }
}

#[test]
fn test_estimate_input_tokens_non_zero() {
    let bundle = sample_bundle();
    let tokens = bundle.estimate_input_tokens();
    assert!(tokens > 0, "estimated tokens should be > 0");
}

#[test]
fn test_estimate_input_tokens_rough_proportional() {
    let small = PromptBundle {
        system_messages: vec!["short".into()],
        context_message: "short".into(),
        response_schema: json!({}),
        tools: vec![],
        metadata: sample_bundle().metadata,
    };
    let large = PromptBundle {
        system_messages: vec!["A longer system message with more content.".into()],
        context_message: "A much longer context message with several sentences and details.".into(),
        response_schema: json!({"type": "object", "properties": {"field1": {"type": "string"}, "field2": {"type": "number"}}}),
        tools: vec![],
        metadata: sample_bundle().metadata,
    };
    assert!(
        small.estimate_input_tokens() <= large.estimate_input_tokens(),
        "larger input should have >= estimated tokens"
    );
}

#[test]
fn test_bundle_deserialization_round_trip() {
    let bundle = sample_bundle();
    let json = serde_json::to_string(&bundle).unwrap();
    let deserialized: PromptBundle = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.metadata.prompt_bundle_id, "bundle-1");
    assert_eq!(deserialized.system_messages.len(), 2);
    assert_eq!(deserialized.metadata.context_format, ContextFormat::Toon);
}

#[test]
fn test_metadata_fields() {
    let bundle = sample_bundle();
    assert_eq!(bundle.metadata.agent_role, "root");
    assert_eq!(bundle.metadata.lifecycle_mode, "ephemeral");
    assert_eq!(bundle.metadata.prompt_version, "1.0.0");
    assert_eq!(bundle.metadata.policy_bundle_id, "policy-1");
    assert_eq!(bundle.metadata.policy_hash, "abc123");
}

#[test]
fn test_bundle_with_empty_tools() {
    let bundle = PromptBundle {
        tools: vec![],
        ..sample_bundle()
    };
    assert!(bundle.tools.is_empty());
}

#[test]
fn test_tool_definition() {
    let tool = ToolDefinition {
        name: "search".into(),
        description: "Web search".into(),
        input_schema: json!({"query": {"type": "string"}}),
    };
    assert_eq!(tool.name, "search");
    assert!(tool.input_schema.as_object().unwrap().contains_key("query"));
}
