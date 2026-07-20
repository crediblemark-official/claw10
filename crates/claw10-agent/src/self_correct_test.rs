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
        idempotency_key: "verify-test".into(),
        risk_level: "low".into(),
        approval_id: None,
        budget_remaining: 100.0,
        workspace_dir: "/tmp".into(),
    }
}

fn make_registry() -> Arc<ToolRegistry> {
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(claw10_tool::builtin::ShellTool::new()));
    reg.register(Box::new(claw10_tool::builtin::WindowTool::new()));
    reg.register(Box::new(claw10_tool::builtin::ProcessTool::new()));
    Arc::new(reg)
}

#[tokio::test]
async fn test_verify_shell_success() {
    let engine = VerificationEngine::new(make_registry());
    let ctx = mock_context();

    let result = ToolOutput::ok(json!({
        "stdout": "hello",
        "stderr": "",
        "exit_code": 0,
        "success": true,
        "timed_out": false,
    }));

    let v = engine.verify("shell", &json!({"action": "exec"}), &result, &ctx).await;
    assert_eq!(v, VerificationResult::Success);
}

#[tokio::test]
async fn test_verify_shell_exit_code() {
    let engine = VerificationEngine::new(make_registry());
    let ctx = mock_context();

    let result = ToolOutput::ok(json!({
        "stdout": "",
        "stderr": "command not found",
        "exit_code": 127,
        "success": false,
        "timed_out": false,
    }));

    let v = engine.verify("shell", &json!({"action": "exec"}), &result, &ctx).await;
    assert!(matches!(v, VerificationResult::Failed { .. }));
    if let VerificationResult::Failed { reason, .. } = v {
        assert!(reason.contains("127"));
        assert!(reason.contains("command not found"));
    }
}

#[tokio::test]
async fn test_verify_shell_timed_out() {
    let engine = VerificationEngine::new(make_registry());
    let ctx = mock_context();

    let result = ToolOutput::ok(json!({
        "stdout": "",
        "stderr": "",
        "exit_code": null,
        "success": false,
        "timed_out": true,
    }));

    let v = engine.verify("shell", &json!({"action": "exec"}), &result, &ctx).await;
    assert!(matches!(v, VerificationResult::Failed { .. }));
}

#[tokio::test]
async fn test_verify_process_kill_success() {
    let engine = VerificationEngine::new(make_registry());
    let ctx = mock_context();

    let result = ToolOutput::ok(json!({
        "action": "kill",
        "pid": 99999999,
        "success": true,
    }));

    let v = engine.verify("process", &json!({"action": "kill", "pid": 99999999}), &result, &ctx).await;
    // PID 99999999 doesn't exist, so verify passes (process is already gone)
    assert_eq!(v, VerificationResult::Success);
}

#[tokio::test]
async fn test_verify_screenshot_zero_bytes() {
    let engine = VerificationEngine::new(make_registry());
    let ctx = mock_context();

    let result = ToolOutput::ok(json!({
        "size_bytes": 0,
        "image_base64": "",
    }));

    let v = engine.verify("screenshot", &json!({"action": "capture"}), &result, &ctx).await;
    assert!(matches!(v, VerificationResult::Failed { .. }));
}

#[tokio::test]
async fn test_enrich_failure() {
    let mut tracker = RetryTracker::new();
    tracker.record_failure("Exit code 1".into());
    tracker.record_failure("Timeout".into());

    let result = ToolOutput::ok(json!({"stdout": ""}));
    let verification = VerificationResult::Failed {
        reason: "Command timed out".into(),
        suggestion: Some("Try increasing timeout".into()),
    };

    let enriched = VerificationEngine::enrich_failure(&result, &verification, &tracker);
    assert_eq!(enriched["_verification"]["status"], "failed");
    assert_eq!(enriched["_verification"]["attempts"], 2);
    assert_eq!(enriched["_verification"]["failure_history"][0], "Exit code 1");
}

#[tokio::test]
async fn test_enrich_success() {
    let result = ToolOutput::ok(json!({"stdout": "done"}));
    let enriched = VerificationEngine::enrich_success(&result);
    assert_eq!(enriched["_verification"]["status"], "success");
}

#[test]
fn test_retry_tracker() {
    let mut tracker = RetryTracker::new();
    assert!(tracker.can_retry());
    assert_eq!(tracker.attempt, 0);

    tracker.record_failure("Error 1".into());
    assert_eq!(tracker.attempt, 1);
    assert!(tracker.is_retrying);
    assert!(tracker.can_retry());

    tracker.record_failure("Error 2".into());
    tracker.record_failure("Error 3".into());
    assert_eq!(tracker.attempt, 3);
    assert!(!tracker.can_retry());
    assert!(!tracker.is_retrying); // resolve() not called, but can_retry is false

    assert_eq!(tracker.failure_history.len(), 3);
}

#[test]
fn test_verification_result_status_string() {
    assert_eq!(VerificationResult::Success.to_status_string(), "success");
    assert_eq!(
        VerificationResult::Failed { reason: "err".into(), suggestion: None }.to_status_string(),
        "failed"
    );
    assert_eq!(
        VerificationResult::RequiresScreenshot { reason: "ambig".into() }.to_status_string(),
        "ambiguous"
    );
}

#[test]
fn test_verification_result_is_success() {
    assert!(VerificationResult::Success.is_success());
    assert!(!VerificationResult::Failed { reason: "err".into(), suggestion: None }.is_success());
}
