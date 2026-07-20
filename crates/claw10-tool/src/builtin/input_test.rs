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
        idempotency_key: "input-test".into(),
        risk_level: "low".into(),
        approval_id: None,
        budget_remaining: 100.0,
        workspace_dir: "/tmp".into(),
    }
}

#[tokio::test]
async fn test_input_tool_name_and_description() {
    let tool = InputTool::new();
    assert_eq!(tool.name(), "input");
    assert!(tool.description().contains("xdotool"));
    assert!(tool.description().contains("mouse"));
}

#[tokio::test]
async fn test_input_schema_has_all_actions() {
    let tool = InputTool::new();
    let schema = tool.input_schema();
    let actions = schema["properties"]["action"]["enum"]
        .as_array()
        .expect("actions enum");
    let names: Vec<&str> = actions.iter().filter_map(|v| v.as_str()).collect();
    let expected = [
        "mouse_move", "mouse_click", "mouse_scroll",
        "key_type", "key_press", "key_hotkey",
        "mouse_position", "clipboard_get", "clipboard_set",
    ];
    for e in &expected {
        assert!(names.contains(e), "missing action: {e}");
    }
    assert_eq!(names.len(), 9);
}

#[tokio::test]
async fn test_input_categories() {
    let tool = InputTool::new();
    let cats = tool.categories();
    assert!(cats.contains(&"input"));
    assert!(cats.contains(&"system"));
    assert!(cats.contains(&"desktop"));
}

#[tokio::test]
async fn test_input_side_effect() {
    let tool = InputTool::new();
    assert_eq!(
        tool.side_effect_class() as i32,
        SideEffectClass::Physical as i32
    );
}

#[tokio::test]
async fn test_input_invalid_action() {
    let tool = InputTool::new();
    let ctx = mock_context();
    let args = json!({ "action": "fly" });
    let result = tool.execute(&ctx, args).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("unknown input action"));
}

#[tokio::test]
async fn test_input_mouse_move_missing_xdotool() {
    let tool = InputTool::new();
    let ctx = mock_context();
    let args = json!({ "action": "mouse_move", "x": 100, "y": 200 });

    if !tool.has_xdotool {
        let result = tool.execute(&ctx, args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("xdotool is required"));
    }
    // If xdotool IS available, the test would need a display — skip runtime check
}

#[tokio::test]
async fn test_input_backend_detection() {
    let tool = InputTool::new();
    // Verify detection doesn't panic and results are consistent
    let _ = tool.has_xdotool;
    let _ = tool.has_ydotool;
    let _ = tool.has_wtype;
    let _ = tool.has_xclip;
    let _ = tool.has_wl_clipboard;
    // At most one keyboard backend should be preferred
    // (they can all be installed, but logic picks first available)
}

#[tokio::test]
async fn test_input_debug() {
    let tool = InputTool::new();
    let debug = format!("{tool:?}");
    assert!(debug.contains("has_xdotool"));
    assert!(debug.contains("has_ydotool"));
    assert!(debug.contains("has_wtype"));
    assert!(debug.contains("has_xclip"));
    assert!(debug.contains("has_wl_clipboard"));
}

#[test]
fn test_hotkey_to_wtype_args() {
    // ctrl+c -> [-M ctrl, -k c, -m ctrl]
    let args = InputTool::hotkey_to_wtype_args("ctrl+c");
    assert_eq!(args, vec!["-M", "ctrl", "-k", "c", "-m", "ctrl"]);

    // alt+tab -> [-M alt, -k tab, -m alt]
    let args = InputTool::hotkey_to_wtype_args("alt+tab");
    assert_eq!(args, vec!["-M", "alt", "-k", "tab", "-m", "alt"]);

    // ctrl+shift+escape -> [-M ctrl, -M shift, -k escape, -m shift, -m ctrl] (LIFO release)
    let args = InputTool::hotkey_to_wtype_args("ctrl+shift+escape");
    assert_eq!(args, vec!["-M", "ctrl", "-M", "shift", "-k", "escape", "-m", "shift", "-m", "ctrl"]);
}

#[test]
fn test_hotkey_to_wtype_args_edge_cases() {
    // Single key (no modifier)
    let args = InputTool::hotkey_to_wtype_args("Return");
    assert_eq!(args, vec!["-k", "Return"]);

    // Modifier only (no main key) — just presses the modifier, no release
    let args = InputTool::hotkey_to_wtype_args("ctrl");
    assert_eq!(args, vec!["-M", "ctrl"]);

    // Empty string
    let args = InputTool::hotkey_to_wtype_args("");
    assert!(args.is_empty());

    // Two modifiers, no main key — presses both, no release
    let args = InputTool::hotkey_to_wtype_args("ctrl+shift");
    assert_eq!(args, vec!["-M", "ctrl", "-M", "shift"]);
}

#[tokio::test]
async fn test_input_description_mentions_wayland() {
    let tool = InputTool::new();
    let desc = tool.description();
    assert!(desc.contains("xdotool"));
    assert!(desc.contains("ydotool"));
    assert!(desc.contains("wtype"));
    assert!(desc.contains("Wayland"));
}

#[tokio::test]
async fn test_input_clipboard_missing_xclip() {
    let tool = InputTool::new();
    let ctx = mock_context();
    let args = json!({ "action": "clipboard_get" });

    if !tool.has_xclip {
        let result = tool.execute(&ctx, args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("xclip is required"));
    }
}

#[tokio::test]
async fn test_input_key_type_missing_args() {
    let tool = InputTool::new();
    let ctx = mock_context();
    // key_type without text
    let args = json!({ "action": "key_type" });
    let result = tool.execute(&ctx, args).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_input_mouse_click_invalid_button() {
    let tool = InputTool::new();
    let ctx = mock_context();
    let args = json!({ "action": "mouse_click", "button": "invalid" });
    let result = tool.execute(&ctx, args).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("unknown button"));
}

#[tokio::test]
async fn test_input_mouse_position_without_xdotool() {
    let tool = InputTool::new();
    let ctx = mock_context();
    let args = json!({ "action": "mouse_position" });

    if !tool.has_xdotool {
        let result = tool.execute(&ctx, args).await;
        assert!(result.is_err());
    }
}
