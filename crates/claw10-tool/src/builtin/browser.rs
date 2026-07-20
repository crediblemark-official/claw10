use std::sync::Arc;

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::registry::Tool;
use crate::result::ToolOutput;
use claw10_domain::SideEffectClass;

/// Minimal CDP (Chrome DevTools Protocol) client using raw WebSocket.
/// Replaces the heavy `chromiumoxide` crate with a thin shell-first approach:
/// - Stateless actions (screenshot, pdf, get_html) use Chrome CLI flags
/// - Interactive actions (click, type, execute_js) use raw CDP over WebSocket
struct CdpClient {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    msg_id: u64,
}

impl CdpClient {
    /// Launch Chrome with remote debugging and connect via CDP WebSocket.
    async fn connect(chrome_path: &str, debug_port: u16) -> Result<Self, ToolError> {
        // Kill any existing Chrome on this port
        let _ = tokio::process::Command::new("fuser")
            .args(["-k", &format!("{debug_port}/tcp")])
            .output()
            .await;

        // Launch Chrome headless with remote debugging
        let mut cmd = tokio::process::Command::new(chrome_path);
        cmd.arg("--headless")
            .arg("--disable-gpu")
            .arg("--no-sandbox")
            .arg("--disable-dev-shm-usage")
            .arg(format!("--remote-debugging-port={debug_port}"))
            .arg("about:blank")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        let _ = cmd.spawn();

        // Wait for Chrome to start
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

        // Get WebSocket debugger URL from /json endpoint
        let json_url = format!("http://127.0.0.1:{debug_port}/json");
        let resp = reqwest::get(&json_url)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Chrome debug endpoint: {e}")))?;
        let tabs: Vec<serde_json::Value> = resp
            .json()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("parse Chrome tabs: {e}")))?;

        let ws_url = tabs
            .first()
            .and_then(|t| t.get("webSocketDebuggerUrl"))
            .and_then(|u| u.as_str())
            .ok_or_else(|| ToolError::ExecutionFailed("no WebSocket URL from Chrome".into()))?;

        let (ws, _) = connect_async(ws_url)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("CDP WebSocket: {e}")))?;

        Ok(Self { ws, msg_id: 0 })
    }

    /// Send a CDP command and wait for the result.
    async fn send_command(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, ToolError> {
        self.msg_id += 1;
        let id = self.msg_id;
        let msg = json!({
            "id": id,
            "method": method,
            "params": params,
        });

        let text = serde_json::to_string(&msg)
            .map_err(|e| ToolError::ExecutionFailed(format!("serialize CDP: {e}")))?;
        self.ws
            .send(Message::Text(text.into()))
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("send CDP: {e}")))?;

        // Read responses until we get the one matching our id
        loop {
            let raw = self
                .ws
                .next()
                .await
                .ok_or_else(|| ToolError::ExecutionFailed("CDP stream ended".into()))?
                .map_err(|e| ToolError::ExecutionFailed(format!("read CDP: {e}")))?;

            if let Message::Text(text) = raw {
                let val: serde_json::Value = serde_json::from_str(&text)
                    .map_err(|e| ToolError::ExecutionFailed(format!("parse CDP response: {e}")))?;
                if val.get("id").and_then(|v| v.as_u64()) == Some(id) {
                    if let Some(err) = val.get("error") {
                        return Err(ToolError::ExecutionFailed(format!(
                            "CDP error: {}",
                            err.get("message")
                                .and_then(|m| m.as_str())
                                .unwrap_or("unknown")
                        )));
                    }
                    return Ok(val
                        .get("result")
                        .cloned()
                        .unwrap_or(json!(null)));
                }
            }
        }
    }

    /// Evaluate JavaScript and return the result value.
    async fn evaluate_js(&mut self, js: &str) -> Result<serde_json::Value, ToolError> {
        let result = self
            .send_command("Runtime.evaluate", json!({ "expression": js }))
            .await?;
        Ok(result
            .get("result")
            .and_then(|r| r.get("value"))
            .cloned()
            .unwrap_or(json!(null)))
    }
}

/// Browser automation tool — shell-first approach.
///
/// **Stateless actions** (no CDP needed, use Chrome CLI flags):
/// - `navigate` — visit a URL, return text preview
/// - `get_html` — dump full page HTML
/// - `get_text` — get visible text content
/// - `screenshot` — capture PNG screenshot
/// - `pdf` — generate PDF
///
/// **Interactive actions** (use minimal CDP client via WebSocket):
/// - `click` — click element by CSS selector
/// - `type` — type text into an input field
/// - `execute_js` — run arbitrary JavaScript
/// - `scroll` — scroll the page
/// - `wait` — wait for a timeout or element
/// - `info` — get current URL & title
pub struct BrowserTool {
    state: Arc<Mutex<Option<CdpClient>>>,
    chrome_path: Option<String>,
}

impl BrowserTool {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(None)),
            chrome_path: Self::find_chrome(),
        }
    }

    /// Ensure the CDP client is connected.
    async fn ensure_client(state: &mut Option<CdpClient>, chrome_path: &Option<String>) -> Result<(), ToolError> {
        if state.is_some() {
            return Ok(());
        }
        let path = chrome_path
            .as_deref()
            .ok_or_else(|| ToolError::ExecutionFailed(
                "No Chrome/Chromium found. Install with:\n  sudo apt install chromium-browser\n  sudo pacman -S chromium".into()
            ))?;
        let client = CdpClient::connect(path, 9222).await?;
        *state = Some(client);
        Ok(())
    }

    // ── Chrome discovery ───────────────────────────────────────

    fn find_chrome() -> Option<String> {
        const CANDIDATES: &[&str] = &[
            "/usr/bin/google-chrome",
            "/usr/bin/google-chrome-stable",
            "/usr/bin/chromium",
            "/usr/bin/chromium-browser",
            "/snap/bin/chromium",
            "/usr/bin/chrome",
        ];

        for path in CANDIDATES {
            if std::path::Path::new(path).exists() {
                return Some(path.to_string());
            }
        }

        for name in &["google-chrome-stable", "chromium", "chromium-browser", "chrome"] {
            if let Ok(out) = std::process::Command::new("which").arg(name).output() {
                if out.status.success() {
                    let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !p.is_empty() {
                        return Some(p);
                    }
                }
            }
        }

        None
    }

    fn require_chrome(&self) -> Result<&str, ToolError> {
        self.chrome_path
            .as_deref()
            .ok_or_else(|| ToolError::ExecutionFailed(
                "No Chrome/Chromium found. Install with:\n  sudo apt install chromium-browser\n  sudo pacman -S chromium".into()
            ))
    }

    // ── Command helpers ────────────────────────────────────────

    fn get_str<'a>(args: &'a serde_json::Value, key: &str) -> Result<&'a str, ToolError> {
        args.get(key)
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments(format!("'{key}' is required (string)")))
    }

    // ── Stateless actions (Chrome CLI flags) ───────────────────

    /// Navigate via Chrome CLI: dump DOM and extract title/text.
    async fn cmd_navigate(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let chrome = self.require_chrome()?;
        let url = Self::get_str(args, "url")?;

        // Use --dump-dom to get rendered HTML
        let output = tokio::process::Command::new(chrome)
            .arg("--headless")
            .arg("--disable-gpu")
            .arg("--no-sandbox")
            .arg("--dump-dom")
            .arg(url)
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("chrome --dump-dom: {e}")))?;

        let html = String::from_utf8_lossy(&output.stdout).to_string();

        // Extract title from HTML
        let title = html
            .lines()
            .find(|l| l.contains("<title>"))
            .and_then(|l| {
                let start = l.find("<title>")? + 7;
                let end = l.find("</title>")?;
                Some(l[start..end].trim().to_string())
            })
            .unwrap_or_default();

        // Strip HTML tags for text preview
        let text = strip_html_tags(&html);

        Ok(ToolOutput::ok(json!({
            "url": url,
            "title": title,
            "content_preview": text.chars().take(2000).collect::<String>(),
            "html_length": html.len(),
            "status": "loaded",
        })))
    }

    /// Get full page HTML via Chrome CLI.
    async fn cmd_get_html(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let chrome = self.require_chrome()?;
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("about:blank");

        let output = tokio::process::Command::new(chrome)
            .arg("--headless")
            .arg("--disable-gpu")
            .arg("--no-sandbox")
            .arg("--dump-dom")
            .arg(url)
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("chrome --dump-dom: {e}")))?;

        let html = String::from_utf8_lossy(&output.stdout).to_string();
        let title = extract_title(&html);

        Ok(ToolOutput::ok(json!({
            "url": url,
            "title": title,
            "html": html,
            "html_length": html.len(),
        })))
    }

    /// Get visible text via Chrome CLI.
    async fn cmd_get_text(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let chrome = self.require_chrome()?;
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("about:blank");

        let output = tokio::process::Command::new(chrome)
            .arg("--headless")
            .arg("--disable-gpu")
            .arg("--no-sandbox")
            .arg("--dump-dom")
            .arg(url)
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("chrome --dump-dom: {e}")))?;

        let html = String::from_utf8_lossy(&output.stdout).to_string();
        let title = extract_title(&html);
        let text = strip_html_tags(&html);

        Ok(ToolOutput::ok(json!({
            "url": url,
            "title": title,
            "text": text,
            "text_length": text.len(),
        })))
    }

    /// Screenshot via Chrome CLI flags.
    async fn cmd_screenshot(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let chrome = self.require_chrome()?;
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("about:blank");
        let width = args.get("width").and_then(|v| v.as_u64()).unwrap_or(1920);
        let height = args.get("height").and_then(|v| v.as_u64()).unwrap_or(1080);

        let tmp = tempfile::NamedTempFile::new()
            .map_err(|e| ToolError::ExecutionFailed(format!("tempfile: {e}")))?;
        let path = tmp.path().to_string_lossy().to_string();

        let output = tokio::process::Command::new(chrome)
            .arg("--headless")
            .arg("--disable-gpu")
            .arg("--no-sandbox")
            .arg(format!("--window-size={width},{height}"))
            .arg(format!("--screenshot={path}"))
            .arg(url)
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("chrome --screenshot: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::ExecutionFailed(format!(
                "chrome screenshot failed: {}",
                stderr.trim()
            )));
        }

        // Chrome saves as screenshot.png in CWD, not at the path we specified
        // Check if our path exists, otherwise check current dir
        let image_data = if std::path::Path::new(&path).exists() {
            tokio::fs::read(&path).await
        } else {
            // Chrome sometimes saves to screenshot.png in CWD
            tokio::fs::read("screenshot.png").await
        }.map_err(|e| ToolError::ExecutionFailed(format!("read screenshot: {e}")))?;

        let b64 = base64_encode(&image_data);

        Ok(ToolOutput::ok(json!({
            "url": url,
            "screenshot_base64": b64,
            "size_bytes": image_data.len(),
            "mime_type": "image/png",
        })))
    }

    /// PDF via Chrome CLI flags.
    async fn cmd_pdf(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let chrome = self.require_chrome()?;
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("about:blank");

        let tmp = tempfile::NamedTempFile::new()
            .map_err(|e| ToolError::ExecutionFailed(format!("tempfile: {e}")))?;
        let path = tmp.path().to_string_lossy().to_string();

        let output = tokio::process::Command::new(chrome)
            .arg("--headless")
            .arg("--disable-gpu")
            .arg("--no-sandbox")
            .arg(format!("--print-to-pdf={path}"))
            .arg(url)
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("chrome --print-to-pdf: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::ExecutionFailed(format!(
                "chrome pdf failed: {}",
                stderr.trim()
            )));
        }

        // Check if our path exists, otherwise check CWD
        let pdf_data = if std::path::Path::new(&path).exists() {
            tokio::fs::read(&path).await
        } else {
            tokio::fs::read("output.pdf").await
        }.map_err(|e| ToolError::ExecutionFailed(format!("read pdf: {e}")))?;

        let b64 = base64_encode(&pdf_data);

        Ok(ToolOutput::ok(json!({
            "pdf_base64": b64,
            "size_bytes": pdf_data.len(),
            "mime_type": "application/pdf",
        })))
    }

    // ── Interactive actions (minimal CDP via WebSocket) ────────

    async fn cmd_click(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let selector = Self::get_str(args, "selector")?;
        let mut guard = self.state.lock().await;
        Self::ensure_client(&mut guard, &self.chrome_path).await?;
        let client = guard.as_mut().unwrap();

        // Find element and get its center coordinates
        let js = format!(
            r#"(() => {{
                const el = document.querySelector('{sel}');
                if (!el) return null;
                const r = el.getBoundingClientRect();
                return {{ x: r.x + r.width/2, y: r.y + r.height/2 }};
            }})()"#,
            sel = selector.replace('\'', "\\'")
        );

        let coords = client.evaluate_js(&js).await?;
        let coords = if coords.is_null() {
            return Err(ToolError::ExecutionFailed(format!("element '{selector}' not found")));
        } else {
            coords
        };

        let x = coords.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let y = coords.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);

        // Dispatch mouse events via CDP
        client
            .send_command(
                "Input.dispatchMouseEvent",
                json!({ "type": "mousePressed", "x": x, "y": y, "button": "left", "clickCount": 1 }),
            )
            .await?;
        client
            .send_command(
                "Input.dispatchMouseEvent",
                json!({ "type": "mouseReleased", "x": x, "y": y, "button": "left", "clickCount": 1 }),
            )
            .await?;

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let title: String = client
            .evaluate_js("document.title")
            .await?
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(ToolOutput::ok(json!({
            "action": "click",
            "selector": selector,
            "title": title,
            "success": true,
        })))
    }

    async fn cmd_type(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let selector = Self::get_str(args, "selector")?;
        let text = Self::get_str(args, "text")?;
        let clear_first = args.get("clear_first").and_then(|v| v.as_bool()).unwrap_or(true);

        let mut guard = self.state.lock().await;
        Self::ensure_client(&mut guard, &self.chrome_path).await?;
        let client = guard.as_mut().unwrap();

        if clear_first {
            let js = format!(
                r#"(() => {{
                    const el = document.querySelector('{sel}');
                    if (el) {{ el.value = ''; el.focus(); }}
                }})()"#,
                sel = selector.replace('\'', "\\'")
            );
            let _ = client.evaluate_js(&js).await;
        }

        // Focus the element
        let focus_js = format!(
            r#"document.querySelector('{sel}')?.focus()"#,
            sel = selector.replace('\'', "\\'")
        );
        let _ = client.evaluate_js(&focus_js).await;

        // Type each character via CDP Input.dispatchKeyEvent
        for ch in text.chars() {
            client
                .send_command(
                    "Input.dispatchKeyEvent",
                    json!({ "type": "keyDown", "text": ch.to_string() }),
                )
                .await?;
            client
                .send_command(
                    "Input.dispatchKeyEvent",
                    json!({ "type": "keyUp", "text": ch.to_string() }),
                )
                .await?;
        }

        Ok(ToolOutput::ok(json!({
            "action": "type",
            "selector": selector,
            "text_length": text.len(),
            "clear_first": clear_first,
            "success": true,
        })))
    }

    async fn cmd_execute_js(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let code = Self::get_str(args, "code")?;
        let mut guard = self.state.lock().await;
        Self::ensure_client(&mut guard, &self.chrome_path).await?;
        let client = guard.as_mut().unwrap();

        let result = client.evaluate_js(code).await?;

        Ok(ToolOutput::ok(json!({
            "result": result,
            "success": true,
        })))
    }

    async fn cmd_scroll(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let x = args.get("x").and_then(|v| v.as_i64()).unwrap_or(0);
        let y = args.get("y").and_then(|v| v.as_i64()).unwrap_or(0);

        let mut guard = self.state.lock().await;
        Self::ensure_client(&mut guard, &self.chrome_path).await?;
        let client = guard.as_mut().unwrap();

        let _ = client
            .evaluate_js(&format!("window.scrollBy({x}, {y})"))
            .await;

        Ok(ToolOutput::ok(json!({
            "action": "scroll",
            "x": x, "y": y,
            "success": true,
        })))
    }

    async fn cmd_wait(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let ms = args.get("ms").and_then(|v| v.as_u64()).unwrap_or(1000);

        if let Some(sel) = args.get("selector").and_then(|v| v.as_str()) {
            let mut guard = self.state.lock().await;
            Self::ensure_client(&mut guard, &self.chrome_path).await?;
            let client = guard.as_mut().unwrap();

            let dur = std::time::Duration::from_millis(ms);
            let start = tokio::time::Instant::now();
            loop {
                let js = format!(
                    r#"document.querySelector('{sel}') !== null"#,
                    sel = sel.replace('\'', "\\'")
                );
                if let Ok(val) = client.evaluate_js(&js).await {
                    if val.as_bool() == Some(true) {
                        return Ok(ToolOutput::ok(json!({
                            "action": "wait_for_element",
                            "selector": sel,
                            "found": true,
                        })));
                    }
                }
                if start.elapsed() >= dur {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }

            return Ok(ToolOutput::ok(json!({
                "action": "wait_for_element",
                "selector": sel,
                "found": false,
                "timeout_ms": ms,
            })));
        }

        tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
        Ok(ToolOutput::ok(json!({
            "action": "wait",
            "waited_ms": ms,
            "success": true,
        })))
    }

    async fn cmd_info(&self, _args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let mut guard = self.state.lock().await;
        Self::ensure_client(&mut guard, &self.chrome_path).await?;
        let client = guard.as_mut().unwrap();

        let title: String = client
            .evaluate_js("document.title")
            .await?
            .as_str()
            .unwrap_or("")
            .to_string();

        let url: String = client
            .evaluate_js("window.location.href")
            .await?
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(ToolOutput::ok(json!({
            "title": title,
            "url": url,
        })))
    }
}

impl Default for BrowserTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserTool {
    fn name(&self) -> &'static str {
        "browser"
    }

    fn description(&self) -> &'static str {
        "Automate a headless Chrome browser: navigate, click, type, extract text/HTML, screenshot, execute JS, scroll, wait, generate PDF. \
         Stateless actions use Chrome CLI flags. Interactive actions use minimal CDP via WebSocket."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "navigate", "click", "type", "get_text", "get_html",
                        "screenshot", "execute_js", "scroll", "wait", "info", "pdf"
                    ],
                    "description": "Browser action to perform"
                },
                "url": { "type": "string", "description": "URL for navigate, get_html, get_text, screenshot, pdf" },
                "selector": { "type": "string", "description": "CSS selector for click, type, get_text, wait" },
                "text": { "type": "string", "description": "Text to type" },
                "code": { "type": "string", "description": "JavaScript code for execute_js" },
                "wait_for": { "type": "string", "description": "CSS selector to wait for after navigation" },
                "timeout_ms": { "type": "integer", "description": "Timeout in ms" },
                "full_page": { "type": "boolean", "description": "Full page screenshot" },
                "clear_first": { "type": "boolean", "description": "Clear field before typing" },
                "x": { "type": "integer", "description": "Scroll x amount" },
                "y": { "type": "integer", "description": "Scroll y amount" },
                "ms": { "type": "integer", "description": "Milliseconds to wait" },
                "width": { "type": "integer", "description": "Screenshot viewport width (default: 1920)" },
                "height": { "type": "integer", "description": "Screenshot viewport height (default: 1080)" },
                "landscape": { "type": "boolean", "description": "PDF landscape" },
                "print_background": { "type": "boolean", "description": "PDF print background" }
            },
            "required": ["action"]
        })
    }

    fn categories(&self) -> Vec<&str> {
        vec!["browser", "web", "automation"]
    }

    fn side_effect_class(&self) -> SideEffectClass {
        SideEffectClass::ExternalCommunication
    }

    async fn execute(
        &self,
        _context: &ToolContext,
        args: serde_json::Value,
    ) -> Result<ToolOutput, ToolError> {
        let action = Self::get_str(&args, "action")?;
        match action {
            // Stateless (Chrome CLI flags)
            "navigate"   => self.cmd_navigate(&args).await,
            "get_html"   => self.cmd_get_html(&args).await,
            "get_text"   => self.cmd_get_text(&args).await,
            "screenshot" => self.cmd_screenshot(&args).await,
            "pdf"        => self.cmd_pdf(&args).await,
            // Interactive (minimal CDP via WebSocket)
            "click"      => self.cmd_click(&args).await,
            "type"       => self.cmd_type(&args).await,
            "execute_js" => self.cmd_execute_js(&args).await,
            "scroll"     => self.cmd_scroll(&args).await,
            "wait"       => self.cmd_wait(&args).await,
            "info"       => self.cmd_info(&args).await,
            _ => Err(ToolError::InvalidArguments(format!(
                "unknown browser action: '{action}'. Valid: navigate, click, type, get_text, \
                 get_html, screenshot, execute_js, scroll, wait, info, pdf"
            ))),
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;

    for ch in html.chars() {
        match ch {
            '<' => {
                in_tag = true;
                continue;
            }
            '>' if in_tag => {
                in_tag = false;
                continue;
            }
            _ if in_tag => continue,
            _ => {}
        }

        if in_tag || in_script || in_style {
            continue;
        }

        // Detect script/style boundaries
        let lower = html.to_lowercase();
        if lower.contains("<script") && !lower.contains("</script>") {
            in_script = true;
        }
        if lower.contains("<style") && !lower.contains("</style>") {
            in_style = true;
        }

        result.push(ch);
    }

    // Collapse whitespace
    let mut prev_was_space = false;
    result
        .chars()
        .filter_map(|ch| {
            if ch.is_whitespace() {
                if prev_was_space {
                    None
                } else {
                    prev_was_space = true;
                    Some(' ')
                }
            } else {
                prev_was_space = false;
                Some(ch)
            }
        })
        .collect()
}

fn extract_title(html: &str) -> String {
    html.lines()
        .find(|l| l.to_lowercase().contains("<title>"))
        .and_then(|l| {
            let lower = l.to_lowercase();
            let start = lower.find("<title>")? + 7;
            let end = lower.find("</title>")?;
            Some(l[start..end].trim().to_string())
        })
        .unwrap_or_default()
}
