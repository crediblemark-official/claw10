use super::*;

fn mock_context() -> ToolContext {
    use claw10_domain::{AgentId, MissionId, TaskId, WorkerId};
    use uuid::Uuid;
    ToolContext {
        tenant_id: "test".into(),
        mission_id: MissionId(Uuid::now_v7()),
        task_id: TaskId(Uuid::now_v7()),
        agent_id: AgentId(Uuid::now_v7()),
        worker_id: WorkerId(Uuid::now_v7()),
        idempotency_key: "window-test".into(),
        risk_level: "low".into(),
        approval_id: None,
        budget_remaining: 100.0,
        workspace_dir: "/tmp".into(),
    }
}

#[tokio::test]
async fn test_window_detects_xdotool() {
    let tool = WindowTool::new();
    // On CI or systems without X11, this will be false — that's OK,
    // we just verify the detection runs without panicking.
    let _ = tool.has_xdotool;
}

#[tokio::test]
async fn test_window_tool_name_and_description() {
    let tool = WindowTool::new();
    assert_eq!(tool.name(), "window");
    assert!(tool.description().contains("xdotool"));
}

#[tokio::test]
async fn test_window_schema_contains_all_actions() {
    let tool = WindowTool::new();
    let schema = tool.input_schema();
    let actions = schema["properties"]["action"]["enum"]
        .as_array()
        .expect("actions enum");
    let action_names: Vec<&str> = actions
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(action_names.contains(&"list"));
    assert!(action_names.contains(&"focus"));
    assert!(action_names.contains(&"close"));
    assert!(action_names.contains(&"resize"));
    assert!(action_names.contains(&"move"));
    assert!(action_names.contains(&"info"));
    assert!(action_names.contains(&"search"));
    assert_eq!(action_names.len(), 7);
}

#[tokio::test]
async fn test_window_categories() {
    let tool = WindowTool::new();
    let cats = tool.categories();
    assert!(cats.contains(&"window"));
    assert!(cats.contains(&"desktop"));
    assert!(cats.contains(&"system"));
}

#[tokio::test]
async fn test_window_side_effect() {
    let tool = WindowTool::new();
    assert_eq!(
        tool.side_effect_class() as i32,
        SideEffectClass::ControlledWrite as i32
    );
}

#[tokio::test]
async fn test_window_invalid_action() {
    let tool = WindowTool::new();
    let ctx = mock_context();
    let args = json!({ "action": "fly" });
    let result = tool.execute(&ctx, args).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("unknown window action"));
}

#[tokio::test]
async fn test_window_missing_args() {
    let tool = WindowTool::new();
    let ctx = mock_context();
    // focus without window_id
    let args = json!({ "action": "focus" });
    let result = tool.execute(&ctx, args).await;
    // On systems without xdotool, we get ExecutionFailed (tool missing)
    // On systems with xdotool, we get InvalidArguments (no window_id)
    assert!(result.is_err());
}

#[tokio::test]
async fn test_window_parse_geometry() {
    let output = "X=100\nY=200\nWIDTH=800\nHEIGHT=600\nSCREEN=0";
    let geo = WindowTool::parse_geometry(output);
    assert!(geo.is_some());
    let g = geo.unwrap();
    assert_eq!(g["x"], 100);
    assert_eq!(g["y"], 200);
    assert_eq!(g["width"], 800);
    assert_eq!(g["height"], 600);
}

#[tokio::test]
async fn test_window_parse_geometry_empty() {
    assert!(WindowTool::parse_geometry("").is_some());
}

#[tokio::test]
async fn test_window_debug() {
    let tool = WindowTool::new();
    let debug = format!("{tool:?}");
    assert!(debug.contains("has_xdotool"));
}

#[tokio::test]
async fn test_window_list_handles_xdotool_missing() {
    // If xdotool is not installed, cmd_list should return an error
    // rather than panicking. This test always passes.
    let tool = WindowTool::new();
    let ctx = mock_context();
    let args = json!({ "action": "list" });
    match tool.execute(&ctx, args).await {
        Ok(output) => {
            // If xdotool IS installed, we get a valid response
            assert!(output.success);
            assert!(output.data["count"].as_u64().is_some());
        }
        Err(e) => {
            let msg = e.to_string();
            // On systems without xdotool, we expect "xdotool is required"
            assert!(msg.contains("xdotool"), "expected xdotool not found error, got: {msg}");
        }
    }
}
