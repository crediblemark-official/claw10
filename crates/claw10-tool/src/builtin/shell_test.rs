use super::*;

fn mock_context(workspace: &str) -> ToolContext {
    use claw10_domain::{AgentId, MissionId, TaskId, WorkerId};
    use uuid::Uuid;
    ToolContext {
        tenant_id: "default".into(),
        mission_id: MissionId(Uuid::now_v7()),
        task_id: TaskId(Uuid::now_v7()),
        agent_id: AgentId(Uuid::now_v7()),
        worker_id: WorkerId(Uuid::now_v7()),
        idempotency_key: "test".into(),
        risk_level: "low".into(),
        approval_id: None,
        budget_remaining: 100.0,
        workspace_dir: workspace.into(),
    }
}

#[tokio::test]
async fn test_shell_sandboxing() {
    let tool = ShellTool::new();
    let temp_dir = tempfile::tempdir().unwrap();
    let context = mock_context(temp_dir.path().to_str().unwrap());

    // Test 1: Safe env vars are preserved, dangerous ones are removed
    let args_env = json!({
        "action": "exec",
        "command": "env"
    });

    let result = tool.execute(&context, args_env).await.unwrap();
    let output = result.data;
    let stdout = output["stdout"].as_str().unwrap();

    // Safe essential vars should be present
    assert!(stdout.contains("PATH="), "PATH must be preserved");

    // Dangerous vars must NOT be present
    assert!(!stdout.contains("LD_PRELOAD="), "LD_PRELOAD must be removed");
    assert!(!stdout.contains("LD_LIBRARY_PATH="), "LD_LIBRARY_PATH must be removed");
    assert!(!stdout.contains("BASH_ENV="), "BASH_ENV must be removed");

    // Test 2: Working directory is set correctly
    let args_pwd = json!({
        "action": "exec",
        "command": "pwd"
    });

    let result = tool.execute(&context, args_pwd).await.unwrap();
    let output = result.data;
    let stdout = output["stdout"].as_str().unwrap().trim();

    let canonical_temp = std::fs::canonicalize(temp_dir.path()).unwrap();
    let canonical_pwd = std::fs::canonicalize(stdout).unwrap();

    assert_eq!(canonical_temp, canonical_pwd);
}

#[tokio::test]
async fn test_shell_streaming() {
    let tool = ShellTool::new();
    let context = mock_context("/tmp");

    // Command that produces multiple lines
    let args = json!({
        "action": "exec",
        "command": "echo line1 && echo line2 && echo line3"
    });

    let result = tool.execute(&context, args).await.unwrap();
    let output = result.data;
    let stdout = output["stdout"].as_str().unwrap();

    assert!(stdout.contains("line1"), "should contain line1");
    assert!(stdout.contains("line2"), "should contain line2");
    assert!(stdout.contains("line3"), "should contain line3");
    assert_eq!(output["exit_code"], 0);
    assert!(output["success"].as_bool().unwrap());
}

#[tokio::test]
async fn test_shell_failure() {
    let tool = ShellTool::new();
    let context = mock_context("/tmp");

    let args = json!({
        "action": "exec",
        "command": "exit 42"
    });

    let result = tool.execute(&context, args).await.unwrap();
    let output = result.data;

    assert_eq!(output["exit_code"], 42);
    assert!(!output["success"].as_bool().unwrap());
}

#[tokio::test]
async fn test_shell_timeout() {
    let tool = ShellTool::new();
    let context = mock_context("/tmp");

    // Command that sleeps longer than timeout
    let args = json!({
        "action": "exec",
        "command": "sleep 10 && echo done",
        "timeout_seconds": 1
    });

    let result = tool.execute(&context, args).await;
    assert!(result.is_ok(), "timeout should return ok with partial output");

    let output = result.unwrap().data;
    assert!(output["timed_out"].as_bool().unwrap_or(false), "should report timed_out");
}

#[tokio::test]
async fn test_shell_spawn_poll_kill() {
    let tool = ShellTool::new();
    let context = mock_context("/tmp");

    // Spawn a long-running process
    let spawn_args = json!({
        "action": "spawn",
        "command": "echo started && sleep 30 && echo done"
    });

    let spawn_result = tool.execute(&context, spawn_args).await.unwrap();
    let pid = spawn_result.data["pid"].as_u64().unwrap();
    assert!(pid > 0, "spawn should return a pid");

    // Poll immediately — should still be running with "started" output
    let poll_args = json!({
        "action": "poll",
        "pid": pid
    });

    // Give it a moment to output
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let poll_result = tool.execute(&context, poll_args).await.unwrap();
    let data = poll_result.data;
    assert!(data["running"].as_bool().unwrap_or(false), "should be running");
    assert!(data["stdout"].as_str().unwrap_or("").contains("started"), "should have 'started' in stdout");

    // Kill it
    let kill_args = json!({
        "action": "kill",
        "pid": pid
    });

    let kill_result = tool.execute(&context, kill_args).await.unwrap();
    assert!(kill_result.data["terminated"].as_bool().unwrap_or(false), "should be terminated");

    // List should be empty (killed process is removed from map)
    let list_args = json!({
        "action": "list"
    });

    let list_result = tool.execute(&context, list_args).await.unwrap();
    assert_eq!(list_result.data["count"].as_u64().unwrap_or(99), 0, "no processes should remain");
}
