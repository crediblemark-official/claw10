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
        idempotency_key: "process-test".into(),
        risk_level: "low".into(),
        approval_id: None,
        budget_remaining: 100.0,
        workspace_dir: "/tmp".into(),
    }
}

#[tokio::test]
async fn test_process_tool_name_and_description() {
    let tool = ProcessTool::new();
    assert_eq!(tool.name(), "process");
    assert!(tool.description().contains("ps/kill/proc"));
}

#[tokio::test]
async fn test_process_schema_contains_all_actions() {
    let tool = ProcessTool::new();
    let schema = tool.input_schema();
    let actions = schema["properties"]["action"]["enum"]
        .as_array()
        .expect("actions enum");
    let action_names: Vec<&str> = actions.iter().filter_map(|v| v.as_str()).collect();
    assert!(action_names.contains(&"list"));
    assert!(action_names.contains(&"tree"));
    assert!(action_names.contains(&"kill"));
    assert!(action_names.contains(&"monitor"));
    assert_eq!(action_names.len(), 4);
}

#[tokio::test]
async fn test_process_categories() {
    let tool = ProcessTool::new();
    let cats = tool.categories();
    assert!(cats.contains(&"process"));
    assert!(cats.contains(&"system"));
    assert!(cats.contains(&"monitoring"));
}

#[tokio::test]
async fn test_process_side_effect() {
    let tool = ProcessTool::new();
    assert_eq!(
        tool.side_effect_class() as i32,
        SideEffectClass::ControlledWrite as i32
    );
}

#[tokio::test]
async fn test_process_invalid_action() {
    let tool = ProcessTool::new();
    let ctx = mock_context();
    let args = json!({ "action": "fly" });
    let result = tool.execute(&ctx, args).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("unknown process action"));
}

#[tokio::test]
async fn test_process_kill_missing_pid() {
    let tool = ProcessTool::new();
    let ctx = mock_context();
    let args = json!({ "action": "kill" });
    let result = tool.execute(&ctx, args).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("'pid' is required"));
}

#[tokio::test]
async fn test_process_monitor_missing_pid() {
    let tool = ProcessTool::new();
    let ctx = mock_context();
    let args = json!({ "action": "monitor" });
    let result = tool.execute(&ctx, args).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_process_list_returns_processes() {
    let tool = ProcessTool::new();
    let ctx = mock_context();
    let args = json!({ "action": "list", "limit": 10 });
    let result = tool.execute(&ctx, args).await;

    // This should work on any Linux system
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.success);
    let count = output.data["count"].as_u64().unwrap_or(0);
    assert!(count > 0, "should find at least some processes");
    assert!(output.data["total_running"].as_u64().unwrap_or(0) >= count);
}

#[tokio::test]
async fn test_process_list_with_filter() {
    let tool = ProcessTool::new();
    let ctx = mock_context();
    // Filter for "systemd" or "init" which should exist
    let args = json!({ "action": "list", "filter": "systemd" });
    let result = tool.execute(&ctx, args).await;
    assert!(result.is_ok());
    // May be empty on some systems, but should not error
}

#[tokio::test]
async fn test_process_tree_returns_structure() {
    let tool = ProcessTool::new();
    let ctx = mock_context();
    let args = json!({ "action": "tree", "depth": 3 });
    let result = tool.execute(&ctx, args).await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.success);
    // Tree should at least have PID 1 (init) or some root
    let tree = output.data["tree"].as_array().unwrap();
    assert!(!tree.is_empty(), "tree should have at least one root process");
    // Root should have pid field
    assert!(tree[0].get("pid").is_some(), "tree root should have pid");
}

#[tokio::test]
async fn test_process_parse_ps_line() {
    // Simulate a ps output line
    let line = "1234 1 root 0.0 0.1 12345 678 pts/0 S+ Jan01 00:00:01 /usr/bin/bash --login";
    let parsed = ProcessTool::parse_ps_line(line);
    assert!(parsed.is_some());
    let p = parsed.unwrap();
    assert_eq!(p["pid"], 1234);
    assert_eq!(p["ppid"], 1);
    assert_eq!(p["user"], "root");
    assert_eq!(p["cpu_percent"], 0.0);
    assert_eq!(p["mem_percent"], 0.1);
    assert_eq!(p["vsz_kb"], 12345);
    assert_eq!(p["rss_kb"], 678);
    assert_eq!(p["status"], "S+");
    assert_eq!(p["name"], "/usr/bin/bash");
    assert_eq!(p["command"], "/usr/bin/bash --login");
}

#[tokio::test]
async fn test_process_parse_ps_line_empty() {
    assert!(ProcessTool::parse_ps_line("").is_none());
    assert!(ProcessTool::parse_ps_line("   ").is_none());
}

#[tokio::test]
async fn test_process_debug() {
    let tool = ProcessTool::new();
    let debug = format!("{tool:?}");
    assert!(debug.contains("ProcessTool"));
}

#[tokio::test]
async fn test_process_kill_nonexistent() {
    let tool = ProcessTool::new();
    let ctx = mock_context();
    // PID 99999999 almost certainly doesn't exist
    let args = json!({ "action": "kill", "pid": 99999999, "signal": "TERM" });
    let result = tool.execute(&ctx, args).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("does not exist"));
}
