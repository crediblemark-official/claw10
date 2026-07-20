use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::registry::Tool;
use crate::result::ToolOutput;
use claw10_domain::SideEffectClass;

// ── Background process tracking ────────────────────────────

struct ProcessState {
    pid: u32,
    command: String,
    started_at: std::time::Instant,
    stdout: Arc<Mutex<String>>,
    stderr: Arc<Mutex<String>>,
    exit_code: Arc<Mutex<Option<i32>>>,
}

// ── Tool ───────────────────────────────────────────────────

/// Execute shell commands with streaming output, background process support,
/// and configurable timeouts (idle + total).
///
/// **Actions:**
/// - `exec` — run a command and wait for completion (streaming output)
/// - `spawn` — start a command in the background, returns a PID
/// - `poll` — check status of a background process by PID
/// - `kill` — terminate a background process
/// - `list` — list all tracked background processes
pub struct ShellTool {
    processes: Arc<Mutex<HashMap<u64, ProcessState>>>,
    next_pid: AtomicU64,
}

impl ShellTool {
    #[must_use]
    pub fn new() -> Self {
        Self {
            processes: Arc::new(Mutex::new(HashMap::new())),
            next_pid: AtomicU64::new(1),
        }
    }

    // ── Helpers ───────────────────────────────────────────────

    fn get_str<'a>(args: &'a serde_json::Value, key: &str) -> Result<&'a str, ToolError> {
        args.get(key)
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments(format!("'{key}' is required (string)")))
    }

    fn get_u64<'a>(args: &'a serde_json::Value, key: &str, default: u64) -> u64 {
        args.get(key)
            .and_then(|v| v.as_u64())
            .unwrap_or(default)
    }

    /// Build a sandboxed `Command` with dangerous env vars stripped.
    fn build_command(&self, command: &str, context: &ToolContext) -> Command {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);

        // Strip dangerous / injection-prone env vars
        let dangerous = [
            "LD_PRELOAD",
            "LD_LIBRARY_PATH",
            "LD_AUDIT",
            "LD_DEBUG",
            "LD_ORIGIN_PATH",
            "DYLD_INSERT_LIBRARIES",
            "DYLD_LIBRARY_PATH",
            "BASH_ENV",
            "ENV",
            "SHELLOPTS",
            "IFS",
            "PERL5LIB",
            "PYTHONPATH",
            "RUBYLIB",
            "RUBYOPT",
            "GEM_PATH",
            "CLASSPATH",
        ];
        for var in &dangerous {
            cmd.env_remove(var);
        }

        // Ensure PATH is set
        let default_path = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";
        let path = std::env::var("PATH").unwrap_or_else(|_| default_path.into());
        cmd.env("PATH", path);

        cmd.current_dir(&context.workspace_dir);
        cmd
    }

    /// Spawn reader tasks that collect stdout / stderr into shared buffers.
    fn spawn_readers(
        child: &mut Child,
    ) -> (Arc<Mutex<String>>, Arc<Mutex<String>>, Arc<Mutex<Option<i32>>>) {
        let stdout = child.stdout.take().expect("stdout piped");
        let stderr = child.stderr.take().expect("stderr piped");

        let stdout_buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
        let stderr_buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
        let exit_code: Arc<Mutex<Option<i32>>> = Arc::new(Mutex::new(None));

        // Stdout reader
        {
            let buf = Arc::clone(&stdout_buf);
            tokio::spawn(async move {
                let mut reader = BufReader::new(stdout);
                let mut chunk = vec![];
                loop {
                    match reader.read_until(b'\n', &mut chunk).await {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {
                            let mut lock = buf.lock().await;
                            lock.push_str(&String::from_utf8_lossy(&chunk));
                            chunk.clear();
                        }
                    }
                }
            });
        }

        // Stderr reader
        {
            let buf = Arc::clone(&stderr_buf);
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr);
                let mut chunk = vec![];
                loop {
                    match reader.read_until(b'\n', &mut chunk).await {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {
                            let mut lock = buf.lock().await;
                            lock.push_str(&String::from_utf8_lossy(&chunk));
                            chunk.clear();
                        }
                    }
                }
            });
        }

        (stdout_buf, stderr_buf, exit_code)
    }

    // ── Actions ───────────────────────────────────────────────

    /// Execute a command with streaming output and configurable timeouts.
    async fn cmd_exec(&self, args: &serde_json::Value, context: &ToolContext) -> Result<ToolOutput, ToolError> {
        let command = Self::get_str(args, "command")?;
        let timeout_total = Self::get_u64(args, "timeout_seconds", 30);
        let timeout_idle = Self::get_u64(args, "timeout_idle_seconds", 0);

        let mut cmd = self.build_command(command, context);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn()
            .map_err(|e| ToolError::ExecutionFailed(format!("spawn: {e}")))?;

        let (stdout_buf, stderr_buf, exit_code) = Self::spawn_readers(&mut child);

        let start = std::time::Instant::now();
        let mut last_output = std::time::Instant::now();
        let mut timed_out = false;

        // Poll until the process exits, checking timeouts every 100ms
        loop {
            // Check total timeout
            if start.elapsed() >= std::time::Duration::from_secs(timeout_total) {
                timed_out = true;
                let _ = child.kill().await;
                break;
            }

            // Check idle timeout
            if timeout_idle > 0 && last_output.elapsed() >= std::time::Duration::from_secs(timeout_idle) {
                timed_out = true;
                let _ = child.kill().await;
                break;
            }

            match child.try_wait() {
                Ok(Some(status)) => {
                    // Process exited
                    let mut ec = exit_code.lock().await;
                    *ec = status.code();
                    break;
                }
                Ok(None) => {
                    // Still running — check for new output and sleep
                    // (output is accumulated by background readers)
                    let sb = stdout_buf.lock().await;
                    let eb = stderr_buf.lock().await;
                    if !sb.is_empty() || !eb.is_empty() {
                        last_output = std::time::Instant::now();
                    }
                    drop(sb);
                    drop(eb);
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                Err(e) => {
                    return Err(ToolError::ExecutionFailed(format!("wait: {e}")));
                }
            }
        }

        // Give readers a moment to flush
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let stdout = stdout_buf.lock().await.clone();
        let stderr = stderr_buf.lock().await.clone();
        let ec = *exit_code.lock().await;

        let elapsed = start.elapsed().as_secs_f64();

        Ok(ToolOutput::ok(json!({
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": ec,
            "success": ec == Some(0),
            "timed_out": timed_out,
            "elapsed_secs": elapsed,
            "action": "exec",
        })))
    }

    /// Start a command in the background, return immediately with a PID.
    async fn cmd_spawn(&self, args: &serde_json::Value, context: &ToolContext) -> Result<ToolOutput, ToolError> {
        let command = Self::get_str(args, "command")?;

        let mut cmd = self.build_command(command, context);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn()
            .map_err(|e| ToolError::ExecutionFailed(format!("spawn: {e}")))?;

        let pid = child.id().ok_or_else(|| {
            ToolError::ExecutionFailed("failed to get child PID".into())
        })?;

        let sys_pid = pid;

        let (stdout_buf, stderr_buf, exit_code) = Self::spawn_readers(&mut child);

        // Wait for exit in background
        let ec = Arc::clone(&exit_code);
        tokio::spawn(async move {
            let status = child.wait().await;
            let mut lock = ec.lock().await;
            *lock = status.ok().and_then(|s| s.code());
        });

        let pid_u64 = self.next_pid.fetch_add(1, Ordering::SeqCst);
        let mut procs = self.processes.lock().await;
        procs.insert(pid_u64, ProcessState {
            pid: sys_pid,
            command: command.to_string(),
            started_at: std::time::Instant::now(),
            stdout: stdout_buf,
            stderr: stderr_buf,
            exit_code,
        });

        Ok(ToolOutput::ok(json!({
            "pid": pid_u64,
            "sys_pid": sys_pid,
            "command": command,
            "status": "running",
            "action": "spawn",
        })))
    }

    /// Check on a background process.
    async fn cmd_poll(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let pid = Self::get_u64(args, "pid", 0);
        if pid == 0 {
            return Err(ToolError::InvalidArguments("'pid' is required".into()));
        }

        let mut procs = self.processes.lock().await;

        // Collect all data before any mutable access to procs
        let (sys_pid, command, stdout, stderr, ec, elapsed) = {
            let state = procs.get(&pid).ok_or_else(|| {
                ToolError::InvalidArguments(format!("no background process with pid {pid}"))
            })?;
            (
                state.pid,
                state.command.clone(),
                state.stdout.lock().await.clone(),
                state.stderr.lock().await.clone(),
                *state.exit_code.lock().await,
                state.started_at.elapsed().as_secs_f64(),
            )
        };

        let is_running = ec.is_none();

        // If process has exited, clean up from the map to avoid
        // accumulating finished processes (memory leak).
        if !is_running {
            procs.remove(&pid);
        }

        Ok(ToolOutput::ok(json!({
            "pid": pid,
            "sys_pid": sys_pid,
            "command": command,
            "running": is_running,
            "exit_code": ec,
            "stdout": stdout,
            "stderr": stderr,
            "elapsed_secs": elapsed,
            "action": "poll",
        })))
    }

    /// Terminate a background process.
    async fn cmd_kill(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let pid = Self::get_u64(args, "pid", 0);
        if pid == 0 {
            return Err(ToolError::InvalidArguments("'pid' is required".into()));
        }

        let mut procs = self.processes.lock().await;
        let state = procs.remove(&pid).ok_or_else(|| {
            ToolError::InvalidArguments(format!("no background process with pid {pid}"))
        })?;

        // Kill via system PID. We don't have a direct handle to the
        // Child anymore (the background task owns it), so use OS kill.
        let sys_pid = state.pid as i32;

        // Send SIGTERM first, then SIGKILL after a short delay
        let _ = std::process::Command::new("kill")
            .arg("-TERM")
            .arg(sys_pid.to_string())
            .output();

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Check if still alive, force kill if needed
        let check = std::process::Command::new("kill")
            .arg("-0")
            .arg(sys_pid.to_string())
            .output();
        if let Ok(c) = check {
            if c.status.success() {
                let _ = std::process::Command::new("kill")
                    .arg("-KILL")
                    .arg(sys_pid.to_string())
                    .output();
            }
        }

        let stdout = state.stdout.lock().await.clone();
        let stderr = state.stderr.lock().await.clone();
        let ec = *state.exit_code.lock().await;
        let elapsed = state.started_at.elapsed().as_secs_f64();

        Ok(ToolOutput::ok(json!({
            "pid": pid,
            "sys_pid": state.pid,
            "command": state.command,
            "terminated": true,
            "exit_code": ec,
            "stdout": stdout,
            "stderr": stderr,
            "elapsed_secs": elapsed,
            "action": "kill",
        })))
    }

    /// List all tracked background processes (prunes finished ones).
    async fn cmd_list(&self) -> Result<ToolOutput, ToolError> {
        let mut procs = self.processes.lock().await;

        // Prune finished processes to avoid memory leaks
        procs.retain(|_, s| s.exit_code.blocking_lock().is_none());

        let mut list: Vec<serde_json::Value> = Vec::new();

        for (&pid, state) in procs.iter() {
            let ec = *state.exit_code.lock().await;
            list.push(json!({
                "pid": pid,
                "sys_pid": state.pid,
                "command": state.command,
                "running": ec.is_none(),
                "exit_code": ec,
                "elapsed_secs": state.started_at.elapsed().as_secs_f64(),
            }));
        }

        Ok(ToolOutput::ok(json!({
            "processes": list,
            "count": list.len(),
            "action": "list",
        })))
    }
}

impl Default for ShellTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &'static str {
        "shell"
    }

    fn description(&self) -> &'static str {
        "Execute shell commands with streaming output, background process support (spawn/poll/kill/list), \
         and configurable timeouts (total + idle)"
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["exec", "spawn", "poll", "kill", "list"],
                    "description": "Action: exec (run + wait), spawn (background), poll (check status), kill (terminate), list (all processes)"
                },
                "command": {
                    "type": "string",
                    "description": "Shell command to execute (required for exec and spawn)"
                },
                "timeout_seconds": {
                    "type": "integer",
                    "description": "Total timeout in seconds (default: 30, for exec action)"
                },
                "timeout_idle_seconds": {
                    "type": "integer",
                    "description": "Idle timeout — kill if no output for N seconds (0 = disabled)"
                },
                "pid": {
                    "type": "integer",
                    "description": "Process ID for poll/kill actions"
                }
            },
            "required": ["action"]
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
        context: &ToolContext,
        args: serde_json::Value,
    ) -> Result<ToolOutput, ToolError> {
        let action = Self::get_str(&args, "action").unwrap_or("exec");
        match action {
            "exec"  => self.cmd_exec(&args, context).await,
            "spawn" => self.cmd_spawn(&args, context).await,
            "poll"  => self.cmd_poll(&args).await,
            "kill"  => self.cmd_kill(&args).await,
            "list"  => self.cmd_list().await,
            _ => Err(ToolError::InvalidArguments(format!(
                "unknown shell action: '{action}'. Valid: exec, spawn, poll, kill, list"
            ))),
        }
    }
}

impl std::fmt::Debug for ShellTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShellTool").finish()
    }
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
#[path = "shell_test.rs"]
mod tests;
