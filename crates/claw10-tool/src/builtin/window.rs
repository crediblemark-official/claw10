use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::registry::Tool;
use crate::result::ToolOutput;
use claw10_domain::SideEffectClass;

/// Manage desktop windows using `xdotool` (primary) and `xprop` (auxiliary).
///
/// Provides human-like window interaction: list, focus, close, resize,
/// move, and search for windows by title/name — all via fast text-based
/// commands rather than expensive screenshots.
///
/// **Actions:**
/// - `list`   — list all visible windows (ID, title, geometry, focused)
/// - `focus`  — activate a window (bring to front)
/// - `close`  — close a window gracefully (or force-kill)
/// - `resize` — resize a window to given width × height
/// - `move`   — move a window to given screen position (x, y)
/// - `info`   — get detailed info (title, geometry, PID, WM_CLASS)
/// - `search` — find windows by title/name pattern
pub struct WindowTool {
    has_xdotool: bool,
}

impl WindowTool {
    #[must_use]
    pub fn new() -> Self {
        Self {
            has_xdotool: Self::check_xdotool(),
        }
    }

    // ── Tool detection ─────────────────────────────────────────

    fn check_xdotool() -> bool {
        std::process::Command::new("which")
            .arg("xdotool")
            .output()
            .ok()
            .is_some_and(|o| o.status.success())
    }

    fn require_xdotool(&self) -> Result<(), ToolError> {
        if !self.has_xdotool {
            return Err(ToolError::ExecutionFailed(
                "xdotool is required for window management. Install it with:\n\
                  sudo apt install xdotool   # Debian/Ubuntu\n\
                  sudo pacman -S xdotool      # Arch\n\
                  brew install xdotool        # macOS (requires XQuartz)"
                    .into(),
            ));
        }
        Ok(())
    }

    // ── Helpers ─────────────────────────────────────────────────

    /// Run an xdotool command asynchronously and return stdout.
    async fn run_xdotool(&self, args: &[&str]) -> Result<String, ToolError> {
        self.require_xdotool()?;
        let output = tokio::process::Command::new("xdotool")
            .args(args)
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("xdotool execution failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::ExecutionFailed(format!(
                "xdotool {} failed: {}",
                args.join(" "),
                stderr.trim(),
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Run any shell command and return stdout lines.
    async fn run_cmd(program: &str, args: &[&str]) -> Result<Vec<String>, ToolError> {
        let output = tokio::process::Command::new(program)
            .args(args)
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("{program} execution failed: {e}")))?;

        if !output.status.success() {
            return Ok(vec![]); // silent fallback
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().map(|l| l.to_string()).collect())
    }

    fn get_str<'a>(args: &'a serde_json::Value, key: &str) -> Result<&'a str, ToolError> {
        args.get(key)
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments(format!("'{key}' is required (string)")))
    }

    fn get_u64(args: &serde_json::Value, key: &str, default: u64) -> u64 {
        args.get(key).and_then(|v| v.as_u64()).unwrap_or(default)
    }

    fn get_i64(args: &serde_json::Value, key: &str, default: i64) -> i64 {
        args.get(key).and_then(|v| v.as_i64()).unwrap_or(default)
    }

    /// Parse window geometry from `xdotool getwindowgeometry --shell` output.
    fn parse_geometry(output: &str) -> Option<serde_json::Value> {
        let mut x = 0i64;
        let mut y = 0i64;
        let mut width = 0u64;
        let mut height = 0u64;

        for line in output.lines() {
            let parts: Vec<&str> = line.split('=').collect();
            if parts.len() != 2 {
                continue;
            }
            match parts[0].trim() {
                "X" => x = parts[1].trim().parse().unwrap_or(0),
                "Y" => y = parts[1].trim().parse().unwrap_or(0),
                "WIDTH" => width = parts[1].trim().parse().unwrap_or(0),
                "HEIGHT" => height = parts[1].trim().parse().unwrap_or(0),
                _ => {}
            }
        }

        Some(json!({
            "x": x, "y": y,
            "width": width, "height": height
        }))
    }

    /// Build a window info JSON object from an xdotool window ID.
    async fn window_info(&self, id: &str, active_id: &str) -> serde_json::Value {
        let name = self
            .run_xdotool(&["getwindowname", id])
            .await
            .unwrap_or_default();
        let title = name.trim().to_string();

        let geo_output = self
            .run_xdotool(&["getwindowgeometry", "--shell", id])
            .await
            .ok();
        let geometry = geo_output.as_ref().and_then(|o| Self::parse_geometry(o));

        json!({
            "id": id.trim(),
            "title": title,
            "geometry": geometry,
            "focused": active_id == id.trim(),
        })
    }

    // ── Actions ─────────────────────────────────────────────────

    /// List all visible windows on the current desktop.
    async fn cmd_list(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        self.require_xdotool()?;

        let all_desktops = args
            .get("all_desktops")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // getactivewindow can fail (no WM, headless) — that's OK, just note it
        let active_id = self
            .run_xdotool(&["getactivewindow"])
            .await
            .unwrap_or_default();
        let active_id = active_id.trim().to_string();

        // Build search args: optionally scope to current desktop
        let mut search_args = vec!["search"];
        if !all_desktops {
            search_args.push("--desktop");
            search_args.push("-1"); // -1 = current desktop
        }
        search_args.push("--name");
        search_args.push(""); // empty matches all
        search_args.push("--limit");
        search_args.push("500");

        // search can fail (no WM, no windows) — return empty list instead of error
        let ids_output = self.run_xdotool(&search_args).await.unwrap_or_default();
        let ids: Vec<&str> = ids_output.lines().filter(|l| !l.trim().is_empty()).collect();

        let mut windows: Vec<serde_json::Value> = Vec::with_capacity(ids.len());
        for id_str in &ids {
            let id = id_str.trim();
            if id.is_empty() {
                continue;
            }
            windows.push(self.window_info(id, &active_id).await);
        }

        Ok(ToolOutput::ok(json!({
            "windows": windows,
            "count": windows.len(),
            "all_desktops": all_desktops,
            "action": "list",
        })))
    }

    /// Focus/activate a window by ID, bringing it to the foreground.
    async fn cmd_focus(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let window_id = Self::get_str(args, "window_id")?;
        self.run_xdotool(&["windowactivate", window_id]).await?;

        let name = self
            .run_xdotool(&["getwindowname", window_id])
            .await
            .unwrap_or_default();
        let title = name.trim().to_string();

        Ok(ToolOutput::ok(json!({
            "action": "focus",
            "window_id": window_id,
            "title": title,
            "success": true,
        })))
    }

    /// Close a window by ID (graceful close, or force-kill).
    async fn cmd_close(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let window_id = Self::get_str(args, "window_id")?;
        let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);

        if force {
            self.run_xdotool(&["windowkill", window_id]).await?;
        } else {
            self.run_xdotool(&["windowclose", window_id]).await?;
        }

        Ok(ToolOutput::ok(json!({
            "action": "close",
            "window_id": window_id,
            "force": force,
            "success": true,
        })))
    }

    /// Resize a window to specified width × height.
    async fn cmd_resize(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let window_id = Self::get_str(args, "window_id")?;
        let width = Self::get_u64(args, "width", 0);
        let height = Self::get_u64(args, "height", 0);

        if width == 0 || height == 0 {
            return Err(ToolError::InvalidArguments(
                "width and height must be > 0".into(),
            ));
        }

        self.run_xdotool(&[
            "windowsize",
            window_id,
            &width.to_string(),
            &height.to_string(),
        ])
        .await?;

        // Optional reposition
        if let Some(x) = args.get("x").and_then(|v| v.as_i64()) {
            if let Some(y) = args.get("y").and_then(|v| v.as_i64()) {
                let _ = self
                    .run_xdotool(&["windowmove", window_id, &x.to_string(), &y.to_string()])
                    .await;
            }
        }

        Ok(ToolOutput::ok(json!({
            "action": "resize",
            "window_id": window_id,
            "width": width,
            "height": height,
            "success": true,
        })))
    }

    /// Move a window to screen position (x, y).
    async fn cmd_move(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let window_id = Self::get_str(args, "window_id")?;
        let x = Self::get_i64(args, "x", 0);
        let y = Self::get_i64(args, "y", 0);

        self.run_xdotool(&[
            "windowmove",
            window_id,
            &x.to_string(),
            &y.to_string(),
        ])
        .await?;

        Ok(ToolOutput::ok(json!({
            "action": "move",
            "window_id": window_id,
            "x": x,
            "y": y,
            "success": true,
        })))
    }

    /// Get detailed info about a specific window.
    async fn cmd_info(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let window_id = Self::get_str(args, "window_id")?;

        let name = self
            .run_xdotool(&["getwindowname", window_id])
            .await
            .unwrap_or_default();
        let title = name.trim().to_string();

        let geo_output = self
            .run_xdotool(&["getwindowgeometry", "--shell", window_id])
            .await
            .ok();
        let geometry = geo_output.as_ref().and_then(|o| Self::parse_geometry(o));

        // PID via xprop
        let pid = Self::run_cmd("xprop", &["-id", window_id, "_NET_WM_PID"])
            .await
            .ok()
            .and_then(|lines| {
                lines
                    .first()
                    .and_then(|l| l.split(" = ").nth(1))
                    .map(|s| s.trim().to_string())
            });

        // WM_CLASS via xprop
        let class = Self::run_cmd("xprop", &["-id", window_id, "WM_CLASS"])
            .await
            .ok()
            .and_then(|lines| lines.first().cloned());

        // Desktop number via xdotool
        let desktop = self
            .run_xdotool(&["get_desktop_for_window", window_id])
            .await
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok());

        Ok(ToolOutput::ok(json!({
            "action": "info",
            "window_id": window_id,
            "title": title,
            "geometry": geometry,
            "pid": pid,
            "class": class,
            "desktop": desktop,
        })))
    }

    /// Search for windows matching a title/name pattern.
    async fn cmd_search(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let name = Self::get_str(args, "name")?;
        let all_desktops = args
            .get("all_desktops")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let active_id = self
            .run_xdotool(&["getactivewindow"])
            .await
            .unwrap_or_default();
        let active_id = active_id.trim().to_string();

        let mut search_args = vec!["search", "--name"];
        if !all_desktops {
            search_args.push("--desktop");
            search_args.push("-1");
        }
        search_args.push(name);
        search_args.push("--limit");
        search_args.push("100");

        let ids_output = self.run_xdotool(&search_args).await?;
        let ids: Vec<&str> = ids_output.lines().filter(|l| !l.trim().is_empty()).collect();

        let mut windows: Vec<serde_json::Value> = Vec::with_capacity(ids.len());
        for id_str in &ids {
            let id = id_str.trim();
            if id.is_empty() {
                continue;
            }
            windows.push(self.window_info(id, &active_id).await);
        }

        Ok(ToolOutput::ok(json!({
            "action": "search",
            "query": name,
            "windows": windows,
            "count": windows.len(),
        })))
    }
}

impl Default for WindowTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WindowTool {
    fn name(&self) -> &'static str {
        "window"
    }

    fn description(&self) -> &'static str {
        "Manage desktop windows via xdotool: list, focus, close, resize, move, get info, search by title/name"
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "focus", "close", "resize", "move", "info", "search"],
                    "description": "Window action to perform"
                },
                "window_id": {
                    "type": "string",
                    "description": "X11 window ID in hex (e.g., 0x1234567) — required for focus, close, resize, move, info"
                },
                "name": {
                    "type": "string",
                    "description": "Window title/name pattern for search action"
                },
                "width": {
                    "type": "integer",
                    "description": "New width in pixels (for resize action)"
                },
                "height": {
                    "type": "integer",
                    "description": "New height in pixels (for resize action)"
                },
                "x": {
                    "type": "integer",
                    "description": "X screen coordinate (for move, or optional reposition during resize)"
                },
                "y": {
                    "type": "integer",
                    "description": "Y screen coordinate (for move, or optional reposition during resize)"
                },
                "force": {
                    "type": "boolean",
                    "description": "Force close via SIGKILL (default: false, uses WM_DELETE_WINDOW)"
                },
                "all_desktops": {
                    "type": "boolean",
                    "description": "List/search across all virtual desktops (default: false = current desktop only)"
                }
            },
            "required": ["action"]
        })
    }

    fn categories(&self) -> Vec<&str> {
        vec!["window", "desktop", "system"]
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
            "list" => self.cmd_list(&args).await,
            "focus" => self.cmd_focus(&args).await,
            "close" => self.cmd_close(&args).await,
            "resize" => self.cmd_resize(&args).await,
            "move" => self.cmd_move(&args).await,
            "info" => self.cmd_info(&args).await,
            "search" => self.cmd_search(&args).await,
            _ => Err(ToolError::InvalidArguments(format!(
                "unknown window action: '{action}'. Valid: list, focus, close, resize, move, info, search"
            ))),
        }
    }
}

impl std::fmt::Debug for WindowTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowTool")
            .field("has_xdotool", &self.has_xdotool)
            .finish()
    }
}

// ── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
#[path = "window_test.rs"]
mod tests;
