use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::registry::Tool;
use crate::result::ToolOutput;
use claw10_domain::SideEffectClass;

/// Simulate human input: mouse movement, clicks, keyboard typing, and hotkeys.
///
/// **Backends:**
/// - `xdotool` — X11 (primary, most complete)
/// - `ydotool` — Wayland (mouse + keyboard via uinput)
/// - `wtype`   — Wayland (keyboard-only via virtual-keyboard protocol)
///
/// Backend selection: auto-detected at startup. xdotool preferred on X11,
/// ydotool/wtype used on Wayland. Clipboard: xclip (X11) / wl-clipboard (Wayland).
///
/// **Actions:**
/// - `mouse_move`     — move cursor to absolute (x, y) or relative (dx, dy)
/// - `mouse_click`    — click button (left, middle, right, double-click)
/// - `mouse_scroll`   — scroll wheel (positive = up, negative = down)
/// - `key_type`       — type a string of text
/// - `key_press`      — press/release a single key (e.g., Return, Escape)
/// - `key_hotkey`     — press a combination (e.g., ctrl+c, alt+tab)
/// - `mouse_position` — get current cursor position
/// - `clipboard_get`  — read clipboard content
/// - `clipboard_set`  — write text to clipboard
pub struct InputTool {
    // Keyboard/mouse backends
    has_xdotool: bool,
    has_ydotool: bool,
    has_wtype: bool,
    // Clipboard backends
    has_xclip: bool,
    has_wl_clipboard: bool, // wl-copy / wl-paste
}

impl InputTool {
    #[must_use]
    pub fn new() -> Self {
        Self {
            has_xdotool: Self::check_available("xdotool"),
            has_ydotool: Self::check_available("ydotool"),
            has_wtype: Self::check_available("wtype"),
            has_xclip: Self::check_available("xclip"),
            has_wl_clipboard: Self::check_available("wl-copy"),
        }
    }

    // ── Backend detection ──────────────────────────────────────

    fn check_available(name: &str) -> bool {
        std::process::Command::new("which")
            .arg(name)
            .output()
            .ok()
            .is_some_and(|o| o.status.success())
    }

    /// Whether any keyboard input backend is available.
    fn has_keyboard_backend(&self) -> bool {
        self.has_xdotool || self.has_ydotool || self.has_wtype
    }

    /// Whether any mouse backend is available (xdotool or ydotool).
    fn has_mouse_backend(&self) -> bool {
        self.has_xdotool || self.has_ydotool
    }

    /// Whether any clipboard backend is available.
    fn has_clipboard_backend(&self) -> bool {
        self.has_xclip || self.has_wl_clipboard
    }

    fn require_keyboard(&self) -> Result<(), ToolError> {
        if !self.has_keyboard_backend() {
            let msg = "No keyboard input backend found. Install one of:\n\
                        sudo apt install xdotool       # X11 (recommended)\n\
                        sudo apt install ydotool        # Wayland (uinput)\n\
                        sudo apt install wtype          # Wayland (virtual-keyboard)";
            return Err(ToolError::ExecutionFailed(msg.into()));
        }
        Ok(())
    }

    fn require_mouse(&self) -> Result<(), ToolError> {
        if !self.has_mouse_backend() {
            let msg = "No mouse input backend found. Install one of:\n\
                        sudo apt install xdotool       # X11 (recommended)\n\
                        sudo apt install ydotool        # Wayland (uinput)";
            return Err(ToolError::ExecutionFailed(msg.into()));
        }
        Ok(())
    }

    fn require_clipboard(&self) -> Result<(), ToolError> {
        if !self.has_clipboard_backend() {
            let msg = "No clipboard backend found. Install one of:\n\
                        sudo apt install xclip          # X11\n\
                        sudo apt install wl-clipboard   # Wayland";
            return Err(ToolError::ExecutionFailed(msg.into()));
        }
        Ok(())
    }

    // ── Helpers ─────────────────────────────────────────────────

    fn get_str<'a>(args: &'a serde_json::Value, key: &str) -> Result<&'a str, ToolError> {
        args.get(key)
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments(format!("'{key}' is required (string)")))
    }

    fn get_i64(args: &serde_json::Value, key: &str, default: i64) -> i64 {
        args.get(key).and_then(|v| v.as_i64()).unwrap_or(default)
    }

    fn get_u64(args: &serde_json::Value, key: &str, default: u64) -> u64 {
        args.get(key).and_then(|v| v.as_u64()).unwrap_or(default)
    }

    /// Run an xdotool command asynchronously.
    /// Caller must ensure xdotool is available (check has_xdotool).
    async fn run_xdotool(&self, args: &[&str]) -> Result<String, ToolError> {
        let output = tokio::process::Command::new("xdotool")
            .args(args)
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("xdotool failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::ExecutionFailed(format!(
                "xdotool {} failed: {}",
                args.join(" "),
                stderr.trim()
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Run any shell command and return trimmed stdout, or error.
    async fn run_cmd(cmd: &str, args: &[&str]) -> Result<String, ToolError> {
        let output = tokio::process::Command::new(cmd)
            .args(args)
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("{cmd} failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::ExecutionFailed(format!(
                "{cmd} failed: {}",
                stderr.trim()
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    // ── Actions ─────────────────────────────────────────────────

    /// Move mouse cursor to absolute (x, y) or relative (dx, dy).
    /// Backend: xdotool (X11) or ydotool (Wayland).
    async fn cmd_mouse_move(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        self.require_mouse()?;
        let x = Self::get_i64(args, "x", 0);
        let y = Self::get_i64(args, "y", 0);
        let relative = args.get("relative").and_then(|v| v.as_bool()).unwrap_or(false);

        if self.has_xdotool {
            if relative {
                self.run_xdotool(&["mousemove_relative", &x.to_string(), &y.to_string()])
                    .await?;
            } else {
                self.run_xdotool(&["mousemove", &x.to_string(), &y.to_string()])
                    .await?;
            }
        } else if self.has_ydotool {
            // ydotool: mousemove --absolute -x X -y Y  (default is relative)
            let x_str = x.to_string();
            let y_str = y.to_string();
            let mut yd_args: Vec<&str> = vec!["mousemove"];
            if !relative {
                yd_args.push("--absolute");
            }
            yd_args.extend(["-x", &x_str, "-y", &y_str]);
            Self::run_cmd("ydotool", &yd_args).await?;
        }

        Ok(ToolOutput::ok(json!({
            "action": "mouse_move",
            "x": x,
            "y": y,
            "relative": relative,
            "backend": if self.has_xdotool { "xdotool" } else { "ydotool" },
            "success": true,
        })))
    }

    /// Click mouse button.
    /// Backend: xdotool (X11) or ydotool (Wayland).
    async fn cmd_mouse_click(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        self.require_mouse()?;
        let button = Self::get_str(args, "button").unwrap_or("left");
        let repeat = Self::get_u64(args, "repeat", 1).max(1).min(10);

        if self.has_xdotool {
            let btn_num = match button {
                "left" | "1" => "1",
                "middle" | "2" => "2",
                "right" | "3" => "3",
                "scroll_up" | "4" => "4",
                "scroll_down" | "5" => "5",
                other => return Err(ToolError::InvalidArguments(format!(
                    "unknown button: '{other}'. Valid: left, middle, right, scroll_up, scroll_down"
                ))),
            };
            if repeat > 1 {
                self.run_xdotool(&["click", "--repeat", &repeat.to_string(), btn_num])
                    .await?;
            } else {
                self.run_xdotool(&["click", btn_num]).await?;
            }
        } else if self.has_ydotool {
            // ydotool: click -B btn_code (1=left, 2=right, 3=middle)
            // ydotool uses Linux input event codes from <linux/input-event-codes.h>
            // BTN_LEFT=0x110, BTN_RIGHT=0x111, BTN_MIDDLE=0x112
            let btn_code = match button {
                "left" | "1" => "0x110",
                "right" | "3" => "0x111",
                "middle" | "2" => "0x112",
                other => return Err(ToolError::InvalidArguments(format!(
                    "ydotool: unknown button '{other}'. Valid: left, middle, right"
                ))),
            };
            for _ in 0..repeat {
                Self::run_cmd("ydotool", &["click", "-B", btn_code]).await?;
            }
        }

        Ok(ToolOutput::ok(json!({
            "action": "mouse_click",
            "button": button,
            "repeat": repeat,
            "backend": if self.has_xdotool { "xdotool" } else { "ydotool" },
            "success": true,
        })))
    }

    /// Scroll mouse wheel. Positive delta = scroll up, negative = scroll down.
    async fn cmd_mouse_scroll(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        self.require_mouse()?;
        let delta = Self::get_i64(args, "delta", 1);

        if self.has_xdotool {
            let abs_delta = delta.unsigned_abs();
            let button = if delta > 0 { "4" } else { "5" };
            if abs_delta > 1 {
                self.run_xdotool(&["click", "--repeat", &abs_delta.to_string(), button])
                    .await?;
            } else {
                self.run_xdotool(&["click", button]).await?;
            }
        } else if self.has_ydotool {
            // ydotool: mousemove --wheel -d delta  (positive = up)
            let delta_str = delta.to_string();
            Self::run_cmd("ydotool", &["mousemove", "--wheel", "-d", &delta_str])
                .await?;
        }

        Ok(ToolOutput::ok(json!({
            "action": "mouse_scroll",
            "delta": delta,
            "backend": if self.has_xdotool { "xdotool" } else { "ydotool" },
            "success": true,
        })))
    }

    /// Type a string of text.
    /// Backend: xdotool (X11), ydotool (Wayland), or wtype (Wayland keyboard-only).
    async fn cmd_key_type(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        self.require_keyboard()?;
        let text = Self::get_str(args, "text")?;
        let delay_ms = Self::get_u64(args, "delay_ms", 12);

        let backend = if self.has_xdotool {
            self.run_xdotool(&["type", "--delay", &delay_ms.to_string(), text])
                .await?;
            "xdotool"
        } else if self.has_ydotool {
            // ydotool: type --delay <ms> <text>
            Self::run_cmd("ydotool", &["type", "--delay", &delay_ms.to_string(), text])
                .await?;
            "ydotool"
        } else if self.has_wtype {
            // wtype: -d <ms> <text>
            let delay_str = delay_ms.to_string();
            Self::run_cmd("wtype", &["-d", &delay_str, text])
                .await?;
            "wtype"
        } else {
            unreachable!("require_keyboard checked");
        };

        Ok(ToolOutput::ok(json!({
            "action": "key_type",
            "text_length": text.len(),
            "delay_ms": delay_ms,
            "backend": backend,
            "success": true,
        })))
    }

    /// Press and release a single key (e.g., Return, Escape, Tab, BackSpace).
    /// Backend: xdotool (X11), ydotool (Wayland), or wtype (Wayland).
    async fn cmd_key_press(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        self.require_keyboard()?;
        let key = Self::get_str(args, "key")?;
        let hold = args.get("hold").and_then(|v| v.as_bool()).unwrap_or(false);

        let backend = if self.has_xdotool {
            if hold {
                self.run_xdotool(&["keydown", key]).await?;
            } else {
                self.run_xdotool(&["key", key]).await?;
            }
            "xdotool"
        } else if self.has_ydotool {
            // ydotool uses key names like RETURN, ESCAPE, etc.
            // For hold: keydown <key>; for press: key <key>
            if hold {
                Self::run_cmd("ydotool", &["keydown", key]).await?;
            } else {
                Self::run_cmd("ydotool", &["key", key]).await?;
            }
            "ydotool"
        } else if self.has_wtype {
            // wtype: -k <key> (press+release) or -P <key> (press) / -p <key> (release)
            if hold {
                Self::run_cmd("wtype", &["-P", key]).await?;
            } else {
                Self::run_cmd("wtype", &["-k", key]).await?;
            }
            "wtype"
        } else {
            unreachable!("require_keyboard checked");
        };

        Ok(ToolOutput::ok(json!({
            "action": "key_press",
            "key": key,
            "hold": hold,
            "backend": backend,
            "success": true,
        })))
    }

    /// Press a hotkey combination (e.g., "ctrl+c", "alt+tab", "ctrl+shift+esc").
    /// Backend: xdotool (X11), ydotool (Wayland), or wtype (Wayland).
    async fn cmd_key_hotkey(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        self.require_keyboard()?;
        let keys = Self::get_str(args, "keys")?;
        let hold = args.get("hold").and_then(|v| v.as_bool()).unwrap_or(false);

        let backend = if self.has_xdotool {
            if hold {
                self.run_xdotool(&["keydown", keys]).await?;
            } else {
                self.run_xdotool(&["key", keys]).await?;
            }
            "xdotool"
        } else if self.has_ydotool {
            if hold {
                Self::run_cmd("ydotool", &["keydown", keys]).await?;
            } else {
                Self::run_cmd("ydotool", &["key", keys]).await?;
            }
            "ydotool"
        } else if self.has_wtype {
            // wtype: -M ctrl -k c -m ctrl  (press modifier, press key, release modifier)
            // Convert "ctrl+c" format to wtype args
            let wtype_args = Self::hotkey_to_wtype_args(keys);
            let wtype_refs: Vec<&str> = wtype_args.iter().map(String::as_str).collect();
            Self::run_cmd("wtype", &wtype_refs).await?;
            "wtype"
        } else {
            unreachable!("require_keyboard checked");
        };

        Ok(ToolOutput::ok(json!({
            "action": "key_hotkey",
            "keys": keys,
            "hold": hold,
            "backend": backend,
            "success": true,
        })))
    }

    /// Convert hotkey string (e.g. "ctrl+c") to wtype arguments.
    /// Result: ["-M", "ctrl", "-k", "c", "-m", "ctrl"]
    /// Modifiers are released in reverse order (LIFO) to match keyboard physics.
    fn hotkey_to_wtype_args(keys: &str) -> Vec<String> {
        let mut args = Vec::new();
        let parts: Vec<&str> = keys.split('+').collect();
        let mut modifiers = Vec::new();
        let mut main_key = "";

        for part in &parts {
            match *part {
                "ctrl" | "alt" | "shift" | "super" | "meta" => {
                    modifiers.push(part.to_string());
                }
                _ => {
                    main_key = part;
                }
            }
        }

        // Press all modifiers (in order)
        for m in &modifiers {
            args.push("-M".to_string());
            args.push(m.clone());
        }

        // Press the main key
        if !main_key.is_empty() {
            args.push("-k".to_string());
            args.push(main_key.to_string());
        }

        // Release all modifiers in REVERSE order (LIFO) — only if we pressed a key
        if !main_key.is_empty() {
            for m in modifiers.iter().rev() {
                args.push("-m".to_string());
                args.push(m.clone());
            }
        }

        args
    }

    /// Get current mouse cursor position.
    /// Backend: xdotool (X11) only. ydotool does not support getmouselocation.
    async fn cmd_mouse_position(&self, _args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        if !self.has_xdotool {
            return Err(ToolError::ExecutionFailed(
                "mouse_position requires xdotool. ydotool and wtype do not support getmouselocation.\n\
                 Install xdotool: sudo apt install xdotool".into()
            ));
        }

        let output = self.run_xdotool(&["getmouselocation"]).await?;
        let mut x: i64 = 0;
        let mut y: i64 = 0;
        for part in output.split_whitespace() {
            if let Some(val) = part.strip_prefix("x:") {
                x = val.parse().unwrap_or(0);
            } else if let Some(val) = part.strip_prefix("y:") {
                y = val.parse().unwrap_or(0);
            }
        }
        Ok(ToolOutput::ok(json!({
            "action": "mouse_position",
            "x": x,
            "y": y,
            "backend": "xdotool",
            "success": true,
        })))
    }

    /// Read clipboard content.
    /// Backend: xclip (X11) or wl-paste (Wayland).
    async fn cmd_clipboard_get(&self, _args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        self.require_clipboard()?;
        let (text, backend) = if self.has_xclip {
            let output = Self::run_cmd("xclip", &["-o", "-selection", "clipboard"]).await?;
            (output, "xclip")
        } else if self.has_wl_clipboard {
            // wl-paste can hang without a Wayland clipboard manager — use timeout
            let output = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                Self::run_cmd("wl-paste", &["--no-newline"]),
            ).await
            .map_err(|_| ToolError::ExecutionFailed(
                "wl-paste timed out after 10s — no Wayland clipboard manager available".into()
            ))??;
            (output, "wl-clipboard")
        } else {
            unreachable!("require_clipboard checked");
        };

        Ok(ToolOutput::ok(json!({
            "action": "clipboard_get",
            "text": text,
            "length": text.len(),
            "backend": backend,
            "success": true,
        })))
    }

    /// Write text to clipboard.
    /// Backend: xclip (X11) or wl-copy (Wayland).
    async fn cmd_clipboard_set(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        self.require_clipboard()?;
        let text = Self::get_str(args, "text")?;

        if self.has_xclip {
            let mut child = tokio::process::Command::new("xclip")
                .args(["-selection", "clipboard"])
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| ToolError::ExecutionFailed(format!("xclip spawn: {e}")))?;

            if let Some(mut stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                stdin.write_all(text.as_bytes()).await.map_err(|e| {
                    ToolError::ExecutionFailed(format!("xclip write: {e}"))
                })?;
                stdin.flush().await.ok();
                drop(stdin);
            }

            // Polling-based timeout to prevent hang without clipboard manager
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            let status = loop {
                match child.try_wait().map_err(|e| {
                    ToolError::ExecutionFailed(format!("xclip try_wait: {e}"))
                })? {
                    Some(status) => break status,
                    None => {
                        if std::time::Instant::now() > deadline {
                            let _ = child.start_kill();
                            return Err(ToolError::ExecutionFailed(
                                "xclip timed out after 10s — no clipboard manager available".into()
                            ));
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            };
            if !status.success() {
                return Err(ToolError::ExecutionFailed(
                    "xclip exited with non-zero status".into()
                ));
            }
        } else if self.has_wl_clipboard {
            // wl-copy: write text to Wayland clipboard
            let mut child = tokio::process::Command::new("wl-copy")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| ToolError::ExecutionFailed(format!("wl-copy spawn: {e}")))?;

            if let Some(mut stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                stdin.write_all(text.as_bytes()).await.map_err(|e| {
                    ToolError::ExecutionFailed(format!("wl-copy write: {e}"))
                })?;
                stdin.flush().await.ok();
                drop(stdin);
            }

            let status = child.wait().await.map_err(|e| {
                ToolError::ExecutionFailed(format!("wl-copy wait: {e}"))
            })?;
            if !status.success() {
                return Err(ToolError::ExecutionFailed(
                    "wl-copy exited with non-zero status".into()
                ));
            }
        }

        Ok(ToolOutput::ok(json!({
            "action": "clipboard_set",
            "length": text.len(),
            "backend": if self.has_xclip { "xclip" } else { "wl-clipboard" },
            "success": true,
        })))
    }
}

impl Default for InputTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for InputTool {
    fn name(&self) -> &'static str {
        "input"
    }

    fn description(&self) -> &'static str {
        "Simulate human input: mouse move/click/scroll, keyboard type/press/hotkey, clipboard get/set. Backend: xdotool (X11) / ydotool+wtype (Wayland)"
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "mouse_move", "mouse_click", "mouse_scroll",
                        "key_type", "key_press", "key_hotkey",
                        "mouse_position", "clipboard_get", "clipboard_set"
                    ],
                    "description": "Input action to perform"
                },
                "x": { "type": "integer", "description": "X coordinate for mouse_move" },
                "y": { "type": "integer", "description": "Y coordinate for mouse_move" },
                "relative": { "type": "boolean", "description": "Use relative coordinates for mouse_move (default: false)" },
                "button": { "type": "string", "description": "Mouse button: left, middle, right, scroll_up, scroll_down (for mouse_click)" },
                "repeat": { "type": "integer", "description": "Click repeat count (default: 1, max: 10)" },
                "delta": { "type": "integer", "description": "Scroll amount: positive=up, negative=down (for mouse_scroll)" },
                "text": { "type": "string", "description": "Text to type (for key_type) or clipboard content (for clipboard_set)" },
                "key": { "type": "string", "description": "Key name: Return, Escape, Tab, BackSpace, Shift_L, etc. (for key_press)" },
                "keys": { "type": "string", "description": "Hotkey combination: ctrl+c, alt+tab, ctrl+shift+esc (for key_hotkey)" },
                "delay_ms": { "type": "integer", "description": "Delay between keystrokes in ms (default: 12, for key_type)" },
                "hold": { "type": "boolean", "description": "Hold the key down (keydown/keyup) instead of press (default: false)" }
            },
            "required": ["action"]
        })
    }

    fn categories(&self) -> Vec<&str> {
        vec!["input", "system", "desktop"]
    }

    fn side_effect_class(&self) -> SideEffectClass {
        SideEffectClass::Physical
    }

    async fn execute(
        &self,
        _context: &ToolContext,
        args: serde_json::Value,
    ) -> Result<ToolOutput, ToolError> {
        let action = Self::get_str(&args, "action").unwrap_or("mouse_position");
        match action {
            "mouse_move"      => self.cmd_mouse_move(&args).await,
            "mouse_click"     => self.cmd_mouse_click(&args).await,
            "mouse_scroll"    => self.cmd_mouse_scroll(&args).await,
            "key_type"        => self.cmd_key_type(&args).await,
            "key_press"       => self.cmd_key_press(&args).await,
            "key_hotkey"      => self.cmd_key_hotkey(&args).await,
            "mouse_position"  => self.cmd_mouse_position(&args).await,
            "clipboard_get"   => self.cmd_clipboard_get(&args).await,
            "clipboard_set"   => self.cmd_clipboard_set(&args).await,
            _ => Err(ToolError::InvalidArguments(format!(
                "unknown input action: '{action}'. Valid: mouse_move, mouse_click, mouse_scroll, \
                 key_type, key_press, key_hotkey, mouse_position, clipboard_get, clipboard_set"
            ))),
        }
    }
}

impl std::fmt::Debug for InputTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputTool")
            .field("has_xdotool", &self.has_xdotool)
            .field("has_ydotool", &self.has_ydotool)
            .field("has_wtype", &self.has_wtype)
            .field("has_xclip", &self.has_xclip)
            .field("has_wl_clipboard", &self.has_wl_clipboard)
            .finish()
    }
}

// ── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
#[path = "input_test.rs"]
mod tests;
