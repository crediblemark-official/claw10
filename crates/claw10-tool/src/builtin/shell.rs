use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::registry::Tool;
use crate::result::ToolOutput;
use claw10_domain::SideEffectClass;

pub struct ShellTool;

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &'static str {
        "shell"
    }

    fn description(&self) -> &'static str {
        "Execute a shell command with arguments"
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to execute"
                },
                "timeout_seconds": {
                    "type": "integer",
                    "description": "Timeout in seconds",
                    "default": 30
                }
            },
            "required": ["command"]
        })
    }

    fn categories(&self) -> Vec<&str> {
        vec!["shell"]
    }

    fn side_effect_class(&self) -> SideEffectClass {
        SideEffectClass::ControlledWrite
    }

    async fn execute(
        &self,
        _context: &ToolContext,
        args: serde_json::Value,
    ) -> Result<ToolOutput, ToolError> {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("command is required".into()))?;
        let timeout = args
            .get("timeout_seconds")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(30);

        let default_path = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";
        let path = std::env::var("PATH").unwrap_or_else(|_| default_path.into());

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(timeout),
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .env_clear()
                .env("PATH", path)
                .current_dir(&_context.workspace_dir)
                .output(),
        )
        .await
        .map_err(|_| ToolError::Timeout(timeout))?
        .map_err(|e| ToolError::ExecutionFailed(format!("shell execution failed: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(ToolOutput::ok(json!({
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": output.status.code(),
            "success": output.status.success(),
        })))
    }
}

#[cfg(test)]
mod tests {
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
        let tool = ShellTool;
        let temp_dir = tempfile::tempdir().unwrap();
        let context = mock_context(temp_dir.path().to_str().unwrap());

        // Test 1: Env variables are cleared (except PATH)
        let args_env = json!({
            "command": "env"
        });

        let result = tool.execute(&context, args_env).await.unwrap();
        let output = result.data;
        let stdout = output["stdout"].as_str().unwrap();

        // Typical host variables should be missing
        assert!(!stdout.contains("HOME="));
        assert!(!stdout.contains("USER="));

        // PATH must be present
        assert!(stdout.contains("PATH="));

        // Test 2: Working directory is set correctly
        let args_pwd = json!({
            "command": "pwd"
        });

        let result = tool.execute(&context, args_pwd).await.unwrap();
        let output = result.data;
        let stdout = output["stdout"].as_str().unwrap().trim();

        // `pwd` might return a symlink-resolved path (like /private/var vs /var on macOS)
        // so we check if they canonicalize to the same place, or just do a basic check.
        let canonical_temp = std::fs::canonicalize(temp_dir.path()).unwrap();
        let canonical_pwd = std::fs::canonicalize(stdout).unwrap();

        assert_eq!(canonical_temp, canonical_pwd);
    }
}
