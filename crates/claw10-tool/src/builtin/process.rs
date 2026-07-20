use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::registry::Tool;
use crate::result::ToolOutput;
use claw10_domain::SideEffectClass;

/// Manage system processes using `ps`, `kill`, and `/proc`.
///
/// Provides shell-first, text-based process introspection and control:
/// list processes, inspect process hierarchy, kill/stop processes,
/// and monitor detailed process state.
///
/// **Actions:**
/// - `list`    — list processes (optionally filtered by name/command)
/// - `tree`    — show process hierarchy as a tree
/// - `kill`    — send a signal to a process (default: SIGTERM)
/// - `monitor` — get detailed info about a specific process
pub struct ProcessTool;

impl ProcessTool {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    // ── Helpers ─────────────────────────────────────────────────

    fn get_str<'a>(args: &'a serde_json::Value, key: &str) -> Result<&'a str, ToolError> {
        args.get(key)
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments(format!("'{key}' is required (string)")))
    }

    fn get_u64(args: &serde_json::Value, key: &str, default: u64) -> u64 {
        args.get(key).and_then(|v| v.as_u64()).unwrap_or(default)
    }

    /// Parse a single process entry from `ps -o` output.
    ///
    /// Columns: PID, PPID, USER, %CPU, %MEM, VSZ, RSS, TTY, STAT, START, TIME, ARGS
    fn parse_ps_line(line: &str) -> Option<serde_json::Value> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }

        // Split on whitespace. ARGS is the last field and may contain spaces.
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 11 {
            return None;
        }

        // PID is field 0
        let pid: u64 = fields[0].parse().ok()?;
        let ppid: u64 = fields[1].parse().ok()?;

        let user = fields[2].to_string();
        let cpu_pct: f64 = fields[3].parse().unwrap_or(0.0);
        let mem_pct: f64 = fields[4].parse().unwrap_or(0.0);
        let vsz_kb: u64 = fields[5].parse().unwrap_or(0);
        let rss_kb: u64 = fields[6].parse().unwrap_or(0);
        let tty = fields[7].to_string();
        let stat = fields[8].to_string();
        let start = fields[9].to_string();
        let time = fields[10].to_string();

        // ARGS is everything from field 11 onward
        let cmd = if fields.len() > 11 {
            fields[11..].join(" ")
        } else {
            String::new()
        };

        // Extract process name from first part of args
        let name = cmd.split_whitespace().next().unwrap_or(&cmd).to_string();

        Some(json!({
            "pid": pid,
            "ppid": ppid,
            "user": user,
            "name": name,
            "cpu_percent": cpu_pct,
            "mem_percent": mem_pct,
            "vsz_kb": vsz_kb,
            "rss_kb": rss_kb,
            "tty": tty,
            "status": stat,
            "start": start,
            "time": time,
            "command": cmd,
        }))
    }

    /// Run `ps` with custom output format and return parsed processes.
    async fn ps_list(filter: Option<&str>) -> Result<Vec<serde_json::Value>, ToolError> {
        let output = tokio::process::Command::new("ps")
            .args(["-eo", "pid,ppid,user,%cpu,%mem,vsz,rss,tty,stat,start,time,args", "--no-headers", "-w"])
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("ps failed: {e}")))?;

        if !output.status.success() {
            return Err(ToolError::ExecutionFailed(
                "ps command returned non-zero exit status".into(),
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut processes: Vec<serde_json::Value> = Vec::new();

        for line in stdout.lines() {
            if let Some(proc) = Self::parse_ps_line(line) {
                // Apply optional filter
                if let Some(f) = filter {
                    let f_lower = f.to_lowercase();
                    let cmd = proc["command"].as_str().unwrap_or("");
                    let name = proc["name"].as_str().unwrap_or("");
                    let user = proc["user"].as_str().unwrap_or("");
                    if !cmd.to_lowercase().contains(&f_lower)
                        && !name.to_lowercase().contains(&f_lower)
                        && !user.to_lowercase().contains(&f_lower)
                    {
                        continue;
                    }
                }
                processes.push(proc);
            }
        }

        Ok(processes)
    }

    /// Read the contents of a /proc file.
    async fn read_proc_file(pid: u64, file: &str) -> Result<String, ToolError> {
        let path = format!("/proc/{pid}/{file}");
        tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("cannot read {path}: {e}")))
    }

    /// Parse a key: value pair from /proc/{pid}/status
    fn parse_status_value<'a>(content: &'a str, key: &str) -> Option<&'a str> {
        for line in content.lines() {
            if let Some(value) = line.strip_prefix(key).and_then(|s| s.strip_prefix(':')) {
                return Some(value.trim());
            }
        }
        None
    }

    // ── Actions ─────────────────────────────────────────────────

    /// List processes with optional name/command filtering.
    async fn cmd_list(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let filter = args.get("filter").and_then(|v| v.as_str());
        let limit = Self::get_u64(args, "limit", 200) as usize;

        let mut processes = Self::ps_list(filter).await?;

        // Sort by PID ascending
        processes.sort_by(|a, b| {
            a["pid"].as_u64().unwrap_or(0)
                .cmp(&b["pid"].as_u64().unwrap_or(0))
        });

        // Apply limit
        if processes.len() > limit {
            processes.truncate(limit);
        }

        // Get count of total running processes
        let total = Self::ps_list(None).await?.len();

        Ok(ToolOutput::ok(json!({
            "action": "list",
            "processes": processes,
            "count": processes.len(),
            "total_running": total,
            "limit": limit,
            "filter": filter,
        })))
    }

    /// Show process hierarchy as a nested tree.
    async fn cmd_tree(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let filter = args.get("filter").and_then(|v| v.as_str());
        let depth = Self::get_u64(args, "depth", 5) as usize;

        let all_procs = Self::ps_list(None).await?;

        // Build pid → children map
        let mut children_map: std::collections::BTreeMap<u64, Vec<&serde_json::Value>> =
            std::collections::BTreeMap::new();

        for proc in &all_procs {
            let ppid = proc["ppid"].as_u64().unwrap_or(0);
            children_map.entry(ppid).or_default().push(proc);
        }

        // Build tree recursively starting from PID 1 (init)
        fn build_subtree(
            pid: u64,
            children_map: &std::collections::BTreeMap<u64, Vec<&serde_json::Value>>,
            all_procs: &[serde_json::Value],
            depth: usize,
            filter: Option<&str>,
        ) -> Option<serde_json::Value> {
            if depth == 0 {
                return None;
            }

            // Find this process itself
            let proc = all_procs.iter().find(|p| p["pid"].as_u64() == Some(pid))?;

            let name = proc["name"].as_str().unwrap_or("").to_string();
            let command = proc["command"].as_str().unwrap_or("").to_string();

            // Apply filter to root level only (the caller will handle parent filtering)
            let matches_filter = filter.map_or(true, |f| {
                let f = f.to_lowercase();
                command.to_lowercase().contains(&f) || name.to_lowercase().contains(&f)
            });

            // Get children
            let children: Vec<serde_json::Value> = children_map
                .get(&pid)
                .map(|child_procs| {
                    child_procs
                        .iter()
                        .filter_map(|child| {
                            let child_pid = child["pid"].as_u64().unwrap_or(0);
                            build_subtree(
                                child_pid,
                                children_map,
                                all_procs,
                                depth - 1,
                                None, // don't filter children, only parent
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();

            // If the filter is set and neither this process nor any child matches, skip
            let child_matches = !children.is_empty();
            if filter.is_some() && !matches_filter && !child_matches {
                return None;
            }

            Some(json!({
                "pid": pid,
                "name": name,
                "command": command,
                "children": children,
            }))
        }

        // Find root PIDs (usually 1, but also include orphans)
        let mut roots: Vec<u64> = vec![1];

        // Also add any processes whose parent isn't in the list
        let all_pids: std::collections::HashSet<u64> = all_procs
            .iter()
            .filter_map(|p| p["pid"].as_u64())
            .collect();

        for proc in &all_procs {
            let ppid = proc["ppid"].as_u64().unwrap_or(0);
            if ppid > 0 && !all_pids.contains(&ppid) && !roots.contains(&proc["pid"].as_u64().unwrap_or(0)) {
                roots.push(proc["pid"].as_u64().unwrap_or(0));
            }
        }

        let mut tree: Vec<serde_json::Value> = Vec::new();
        for &root_pid in &roots {
            if let Some(node) = build_subtree(
                root_pid,
                &children_map,
                &all_procs,
                depth,
                filter,
            ) {
                tree.push(node);
            }
        }

        Ok(ToolOutput::ok(json!({
            "action": "tree",
            "tree": tree,
            "max_depth": depth,
        })))
    }

    /// Send a signal to a process.
    async fn cmd_kill(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let pid = Self::get_u64(args, "pid", 0);
        if pid == 0 {
            return Err(ToolError::InvalidArguments("'pid' is required and must be > 0".into()));
        }

        let signal = args
            .get("signal")
            .and_then(|v| v.as_str())
            .unwrap_or("TERM");

        // Check process exists first
        let exists = std::path::Path::new(&format!("/proc/{pid}")).exists();
        if !exists {
            return Err(ToolError::ExecutionFailed(format!(
                "process {pid} does not exist"
            )));
        }

        // Try to get process name before killing
        // Note: parse_status_value returns a borrow; convert to owned String while s is alive
        let name = Self::read_proc_file(pid, "status")
            .await
            .ok()
            .and_then(|s| {
                Self::parse_status_value(&s, "Name").map(|v| v.to_string())
            })
            .unwrap_or_default();

        let output = tokio::process::Command::new("kill")
            .args(["-s", signal, &pid.to_string()])
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("kill failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::ExecutionFailed(format!(
                "kill -s {signal} {pid} failed: {}",
                stderr.trim()
            )));
        }

        Ok(ToolOutput::ok(json!({
            "action": "kill",
            "pid": pid,
            "name": name,
            "signal": signal,
            "success": true,
        })))
    }

    /// Get detailed information about a specific process.
    async fn cmd_monitor(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let pid = Self::get_u64(args, "pid", 0);
        if pid == 0 {
            return Err(ToolError::InvalidArguments("'pid' is required and must be > 0".into()));
        }

        let exists = std::path::Path::new(&format!("/proc/{pid}")).exists();
        if !exists {
            return Err(ToolError::ExecutionFailed(format!(
                "process {pid} does not exist"
            )));
        }

        // 1. Get basic info from ps
        let ps_out = tokio::process::Command::new("ps")
            .args([
                "-p",
                &pid.to_string(),
                "-o",
                "pid,ppid,user,%cpu,%mem,vsz,rss,tty,stat,start,time,args",
                "--no-headers",
                "-w",
            ])
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("ps failed: {e}")))?;

        let basic_info = if ps_out.status.success() {
            let stdout = String::from_utf8_lossy(&ps_out.stdout);
            stdout.lines().find_map(Self::parse_ps_line)
        } else {
            None
        };

        // 2. Read /proc/{pid}/status for detailed state
        let status_content = Self::read_proc_file(pid, "status").await.ok();
        let status_info = status_content.as_ref().map(|content| {
            let val = |key: &str| Self::parse_status_value(content, key);
            let vm_peak_kb = val("VmPeak").and_then(|s| {
                s.split_whitespace().next().and_then(|n| n.parse::<u64>().ok())
            });
            let vm_size_kb = val("VmSize").and_then(|s| {
                s.split_whitespace().next().and_then(|n| n.parse::<u64>().ok())
            });
            let vm_rss_kb = val("VmRSS").and_then(|s| {
                s.split_whitespace().next().and_then(|n| n.parse::<u64>().ok())
            });
            json!({
                "name": val("Name"),
                "state": val("State"),
                "uid": val("Uid"),
                "gid": val("Gid"),
                "threads": val("Threads").and_then(|s| s.parse::<u64>().ok()),
                "vm_peak_kb": vm_peak_kb,
                "vm_size_kb": vm_size_kb,
                "vm_rss_kb": vm_rss_kb,
                "fd_size": val("FDSize").and_then(|s| s.parse::<u64>().ok()),
                "voluntary_ctxt_switches": val("voluntary_ctxt_switches")
                    .and_then(|s| s.parse::<u64>().ok()),
                "nonvoluntary_ctxt_switches": val("nonvoluntary_ctxt_switches")
                    .and_then(|s| s.parse::<u64>().ok()),
            })
        });

        // 3. Read command line from /proc
        let cmdline = Self::read_proc_file(pid, "cmdline").await.ok().map(|s| {
            // cmdline is null-separated; replace \0 with space
            s.replace('\0', " ").trim().to_string()
        });

        // 4. Read /proc/{pid}/cwd symlink
        let cwd = tokio::fs::read_link(format!("/proc/{pid}/cwd"))
            .await
            .ok()
            .map(|p| p.to_string_lossy().to_string());

        // 5. Read /proc/{pid}/exe symlink
        let exe = tokio::fs::read_link(format!("/proc/{pid}/exe"))
            .await
            .ok()
            .map(|p| p.to_string_lossy().to_string());

        // 6. Open file descriptors count (ReadDir is not an Iterator, use next_entry loop)
        let fd_count = match tokio::fs::read_dir(format!("/proc/{pid}/fd")).await {
            Ok(mut entries) => {
                let mut count = 0u64;
                loop {
                    match entries.next_entry().await {
                        Ok(Some(_)) => count += 1,
                        Ok(None) => break,
                        Err(_) => break,
                    }
                }
                Some(count)
            }
            Err(_) => None,
        };

        Ok(ToolOutput::ok(json!({
            "action": "monitor",
            "pid": pid,
            "basic": basic_info,
            "status": status_info,
            "cmdline": cmdline,
            "cwd": cwd,
            "executable": exe,
            "open_fds": fd_count,
        })))
    }
}

impl Default for ProcessTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ProcessTool {
    fn name(&self) -> &'static str {
        "process"
    }

    fn description(&self) -> &'static str {
        "Manage system processes using ps/kill/proc: list (filterable), show hierarchy tree, kill/signal, monitor detailed process state"
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "tree", "kill", "monitor"],
                    "description": "Process action to perform"
                },
                "pid": {
                    "type": "integer",
                    "description": "Process ID — required for kill and monitor"
                },
                "filter": {
                    "type": "string",
                    "description": "Case-insensitive filter by process name, command, or user (for list and tree)"
                },
                "signal": {
                    "type": "string",
                    "description": "Signal to send (default: TERM). Common: TERM, KILL, STOP, CONT, HUP, INT"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max results to return (default: 200, for list action)"
                },
                "depth": {
                    "type": "integer",
                    "description": "Max tree depth (default: 5, for tree action)"
                }
            },
            "required": ["action"]
        })
    }

    fn categories(&self) -> Vec<&str> {
        vec!["process", "system", "monitoring"]
    }

    fn side_effect_class(&self) -> SideEffectClass {
        SideEffectClass::ControlledWrite
    }

    async fn execute(
        &self,
        _context: &ToolContext,
        args: serde_json::Value,
    ) -> Result<ToolOutput, ToolError> {
        let action = Self::get_str(&args, "action").unwrap_or("list");
        match action {
            "list"    => self.cmd_list(&args).await,
            "tree"    => self.cmd_tree(&args).await,
            "kill"    => self.cmd_kill(&args).await,
            "monitor" => self.cmd_monitor(&args).await,
            _ => Err(ToolError::InvalidArguments(format!(
                "unknown process action: '{action}'. Valid: list, tree, kill, monitor"
            ))),
        }
    }
}

impl std::fmt::Debug for ProcessTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessTool").finish()
    }
}

// ── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
#[path = "process_test.rs"]
mod tests;
