use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::registry::Tool;
use crate::result::ToolOutput;
use claw10_domain::SideEffectClass;

/// Capture the desktop (full screen, region, or specific window) and
/// return the image as a base64-encoded string.
/// Also supports OCR (Optical Character Recognition) via Tesseract.
///
/// **Capture backends:** scrot &rarr; maim &rarr; import (ImageMagick) &rarr; xwd (raw X11 dump)
/// **OCR backend:** Tesseract CLI (`tesseract` command)
///
/// **Actions:**
/// - `capture` &mdash; take a screenshot (full screen, region, or window)
/// - `info` &mdash; query screen dimensions and available capture tools
/// - `ocr` &mdash; extract text from an image (base64 or file path) using Tesseract OCR
pub struct ScreenshotTool {
    backend: CaptureBackend,
    has_xdpyinfo: bool,
    has_tesseract: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CaptureBackend {
    Scrot,
    Maim,
    Import,
    Xwd,
    GnomeScreenshot,
    None,
}

impl ScreenshotTool {
    #[must_use]
    pub fn new() -> Self {
        Self {
            backend: Self::detect_backend(),
            has_xdpyinfo: Self::check_available("xdpyinfo"),
            has_tesseract: Self::check_available("tesseract"),
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

    fn detect_backend() -> CaptureBackend {
        if Self::check_available("scrot") {
            CaptureBackend::Scrot
        } else if Self::check_available("maim") {
            CaptureBackend::Maim
        } else if Self::check_available("import") {
            CaptureBackend::Import
        } else if Self::check_available("xwd") {
            CaptureBackend::Xwd
        } else if Self::check_available("gnome-screenshot") {
            CaptureBackend::GnomeScreenshot
        } else {
            CaptureBackend::None
        }
    }

    fn require_backend(&self) -> Result<(), ToolError> {
        if self.backend == CaptureBackend::None {
            let msg = "No screen capture tool found. Install one of:\n\
                        sudo apt install scrot       # fastest, recommended\n\
                        sudo apt install maim        # modern alternative\n\
                        sudo apt install imagemagick # adds import + convert\n\
                        sudo pacman -S scrot         # Arch\n\
                        brew install scrot           # macOS (requires XQuartz)";
            return Err(ToolError::ExecutionFailed(msg.into()));
        }
        if std::env::var("DISPLAY").is_err() {
            let msg = "No X11 display found. $DISPLAY is not set. \
                       Make sure you are running in a desktop session or set DISPLAY=:0";
            return Err(ToolError::ExecutionFailed(msg.into()));
        }
        Ok(())
    }

    fn require_ocr(&self) -> Result<(), ToolError> {
        if !self.has_tesseract {
            let msg = "Tesseract OCR is not installed. Install it with:\n\
                        sudo apt install tesseract-ocr    # Debian/Ubuntu\n\
                        sudo pacman -S tesseract           # Arch\n\
                        brew install tesseract             # macOS";
            return Err(ToolError::ExecutionFailed(msg.into()));
        }
        Ok(())
    }

    /// Full-screen dimensions via xdpyinfo.
    fn screen_dimensions() -> Option<(u64, u64)> {
        let output = std::process::Command::new("xdpyinfo")
            .arg("-display")
            .arg(":0")
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(dims) = line.trim().strip_prefix("dimensions:") {
                let parts: Vec<&str> = dims.trim().split_whitespace().collect();
                if let Some(first) = parts.first() {
                    let mut split = first.split('x');
                    let w: u64 = split.next()?.parse().ok()?;
                    let h: u64 = split.next()?.parse().ok()?;
                    return Some((w, h));
                }
            }
        }
        None
    }

    /// Try to get image dimensions from a file using the `file` command.
    fn image_dimensions(path: &str) -> Option<(u64, u64)> {
        let output = std::process::Command::new("file").args([path]).output().ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let lower = line.to_lowercase();
            if let Some(pos) = lower.find(|c: char| c.is_ascii_digit()) {
                let rest = &lower[pos..];
                let parts: Vec<&str> = rest
                    .splitn(3, |c: char| c == 'x' || c == 'X' || c == ' ')
                    .filter(|s| !s.is_empty())
                    .collect();
                if parts.len() >= 2 {
                    let w = parts[0].trim().parse::<u64>().ok()?;
                    let h_str: String = parts[1].chars().take_while(|c| c.is_ascii_digit()).collect();
                    let h = h_str.parse::<u64>().ok()?;
                    if w > 0 && h > 0 {
                        return Some((w, h));
                    }
                }
            }
        }
        None
    }

    // ── Helpers ─────────────────────────────────────────────────

    fn get_str<'a>(args: &'a serde_json::Value, key: &str) -> Result<&'a str, ToolError> {
        args.get(key)
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments(format!("'{key}' is required (string)")))
    }

    fn base64_encode(data: &[u8]) -> String {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(data)
    }

    /// Run a command with owned string args (avoids lifetime issues with temporaries).
    async fn run_command(&self, cmd: &str, args: &[String]) -> Result<(), ToolError> {
        let output = tokio::process::Command::new(cmd)
            .args(args.iter().map(String::as_str))
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("{cmd} execution failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::ExecutionFailed(format!(
                "{cmd} failed: {}",
                stderr.trim()
            )));
        }
        Ok(())
    }

    /// Capture to a temp file, return (temp_path, mime_type).
    async fn run_capture(
        &self,
        region: Option<(u64, u64, u64, u64)>,
        window_id: Option<&str>,
    ) -> Result<(String, String), ToolError> {
        self.require_backend()?;

        let tmp = tempfile::NamedTempFile::new()
            .map_err(|e| ToolError::ExecutionFailed(format!("tempfile: {e}")))?;
        let path = tmp.path().to_string_lossy().to_string();

        let mime_type: String;

        match self.backend {
            CaptureBackend::Scrot => {
                let mut args: Vec<String> = vec!["-z".into()];
                if let Some((x, y, w, h)) = region {
                    args.push("-a".into());
                    args.push(format!("{x},{y},{w},{h}"));
                }
                args.push(path.clone());
                self.run_command("scrot", &args).await?;
                mime_type = "image/png".into();
            }
            CaptureBackend::Maim => {
                let mut args: Vec<String> = vec!["-u".into()];
                if let Some((x, y, w, h)) = region {
                    let geom = format!("{w}x{h}+{x}+{y}");
                    args.push("-g".into());
                    args.push(geom);
                }
                if let Some(id) = window_id {
                    args.push("-i".into());
                    args.push(id.into());
                }
                args.push(path.clone());
                self.run_command("maim", &args).await?;
                mime_type = "image/png".into();
            }
            CaptureBackend::Import => {
                let mut args: Vec<String> = vec![];
                if let Some((x, y, w, h)) = region {
                    args.push("-window".into());
                    args.push("root".into());
                    args.push("-crop".into());
                    args.push(format!("{w}x{h}+{x}+{y}"));
                } else if let Some(id) = window_id {
                    args.push("-window".into());
                    args.push(id.into());
                } else {
                    args.push("-window".into());
                    args.push("root".into());
                }
                args.push(path.clone());
                self.run_command("import", &args).await?;
                mime_type = "image/png".into();
            }
            CaptureBackend::Xwd => {
                let mut args: Vec<String> = vec!["-out".into(), path.clone()];
                if let Some(id) = window_id {
                    args.push("-id".into());
                    args.push(id.into());
                } else {
                    args.push("-root".into());
                }
                self.run_command("xwd", &args).await?;
                mime_type = "image/x-xwd".into();
            }
            CaptureBackend::GnomeScreenshot => {
                let mut args: Vec<String> = vec!["-f".into(), path.clone()];
                if let Some((x, y, w, h)) = region {
                    args.push("-a".into());
                    args.push(format!("{x},{y},{w},{h}"));
                }
                self.run_command("gnome-screenshot", &args).await?;
                mime_type = "image/png".into();
            }
            CaptureBackend::None => {
                return Err(ToolError::ExecutionFailed("No capture backend available".into()));
            }
        }

        Ok((path, mime_type))
    }

    // ── Actions ─────────────────────────────────────────────────

    async fn cmd_capture(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        self.require_backend()?;

        let region = {
            let has_x = args.get("x").and_then(|v| v.as_u64());
            let has_y = args.get("y").and_then(|v| v.as_u64());
            let has_w = args.get("width").and_then(|v| v.as_u64());
            let has_h = args.get("height").and_then(|v| v.as_u64());
            match (has_x, has_y, has_w, has_h) {
                (Some(x), Some(y), Some(w), Some(h)) if w > 0 && h > 0 => Some((x, y, w, h)),
                _ => None,
            }
        };

        let window_id = args.get("window_id").and_then(|v| v.as_str());

        let (path, mime_type) = self.run_capture(region, window_id).await?;

        let image_data = tokio::fs::read(&path)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("cannot read screenshot: {e}")))?;

        let size_bytes = image_data.len() as u64;

        let dimensions = if let Some((_, _, w, h)) = region {
            Some((w, h))
        } else {
            Self::image_dimensions(&path).or_else(Self::screen_dimensions)
        };

        let b64 = Self::base64_encode(&image_data);
        let _ = tokio::fs::remove_file(&path).await;

        let (width, height) = dimensions.unwrap_or((0, 0));

        Ok(ToolOutput::ok(json!({
            "action": "capture",
            "image_base64": b64,
            "mime_type": mime_type,
            "width": width,
            "height": height,
            "size_bytes": size_bytes,
            "backend": format!("{:?}", self.backend).to_lowercase(),
            "region": region.map(|(x, y, w, h)| json!({"x": x, "y": y, "width": w, "height": h})),
            "window_id": window_id,
        })))
    }

    async fn cmd_info(&self, _args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        let (width, height) = Self::screen_dimensions().unwrap_or((0, 0));

        let tools_available: Vec<&str> = {
            let candidates = ["scrot", "maim", "import", "xwd", "gnome-screenshot"];
            candidates
                .iter()
                .filter(|t| Self::check_available(t))
                .copied()
                .collect()
        };

        Ok(ToolOutput::ok(json!({
            "action": "info",
            "width": width,
            "height": height,
            "backend": format!("{:?}", self.backend).to_lowercase(),
            "tools_available": tools_available,
            "has_xdpyinfo": self.has_xdpyinfo,
            "has_tesseract": self.has_tesseract,
        })))
    }

    /// Extract text from an image using Tesseract OCR via shell command.
    ///
    /// Accepts either:
    /// - `image_base64` — base64-encoded image data (from a previous `capture`)
    /// - `file_path` — path to an image file on disk
    ///
    /// Optional: `language` (default: "eng")
    async fn cmd_ocr(&self, args: &serde_json::Value) -> Result<ToolOutput, ToolError> {
        self.require_ocr()?;

        let language = args
            .get("language")
            .and_then(|v| v.as_str())
            .unwrap_or("eng")
            .to_string();

        // Decode or read image into bytes
        let (image_bytes, source): (Vec<u8>, &str) =
            if let Some(b64) = args.get("image_base64").and_then(|v| v.as_str()) {
                use base64::Engine as _;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .map_err(|e| {
                        ToolError::InvalidArguments(format!("invalid base64: {e}"))
                    })?;
                (bytes, "base64")
            } else if let Some(path) = args.get("file_path").and_then(|v| v.as_str()) {
                let bytes = tokio::fs::read(path).await.map_err(|e| {
                    ToolError::InvalidArguments(format!("cannot read file '{path}': {e}"))
                })?;
                (bytes, "file")
            } else {
                return Err(ToolError::InvalidArguments(
                    "provide 'image_base64' (from screenshot capture) or 'file_path' (path to image)"
                        .into(),
                ));
            };

        // Write to temp file for tesseract CLI
        let tmp_dir = tempfile::tempdir().map_err(|e| {
            ToolError::ExecutionFailed(format!("tempdir: {e}"))
        })?;
        let tmp_path = tmp_dir.path().join("ocr_input.png");
        tokio::fs::write(&tmp_path, &image_bytes).await.map_err(|e| {
            ToolError::ExecutionFailed(format!("cannot write temp image: {e}"))
        })?;

        // Run OCR via `tesseract` CLI — stdout for text, TSV for confidence
        let input_path = tmp_path.to_string_lossy().to_string();
        let tsv_path = tmp_dir.path().join("ocr_output.tsv").to_string_lossy().to_string();

        // 1) Get plain text from stdout
        let text_output = tokio::process::Command::new("tesseract")
            .arg(&input_path)
            .arg("stdout")
            .arg("-l")
            .arg(&language)
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("tesseract exec failed: {e}")))?;

        if !text_output.status.success() {
            let stderr = String::from_utf8_lossy(&text_output.stderr);
            return Err(ToolError::ExecutionFailed(format!(
                "tesseract failed: {}",
                stderr.trim()
            )));
        }

        let text = String::from_utf8_lossy(&text_output.stdout).trim().to_string();

        // 2) Get TSV output for confidence calculation
        let mean_conf = {
            let _ = tokio::process::Command::new("tesseract")
                .arg(&input_path)
                .arg(tmp_dir.path().join("ocr_conf"))
                .arg("-l")
                .arg(&language)
                .arg("tsv")
                .output()
                .await;

            // Parse TSV: column 10 = confidence (0-100), skip header and -1 entries
            if let Ok(tsv_bytes) = tokio::fs::read(&tsv_path).await {
                let tsv = String::from_utf8_lossy(&tsv_bytes);
                let confs: Vec<f64> = tsv
                    .lines()
                    .skip(1) // skip header
                    .filter_map(|line| {
                        let cols: Vec<&str> = line.split('\t').collect();
                        if cols.len() > 10 {
                            let conf: f64 = cols[10].parse().ok()?;
                            if conf >= 0.0 { Some(conf) } else { None }
                        } else {
                            None
                        }
                    })
                    .collect();
                if confs.is_empty() {
                    -1.0
                } else {
                    confs.iter().sum::<f64>() / confs.len() as f64
                }
            } else {
                -1.0
            }
        };

        Ok(ToolOutput::ok(json!({
            "action": "ocr",
            "text": text,
            "length": text.len(),
            "mean_confidence": mean_conf.round() as i32,
            "language": language,
            "source": source,
        })))
    }
}

impl Default for ScreenshotTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ScreenshotTool {
    fn name(&self) -> &'static str {
        "screenshot"
    }

    fn description(&self) -> &'static str {
        "Capture screen (full, region, or window) and return base64 image, or extract text via OCR. \
         Capture backends: scrot > maim > import (ImageMagick) > xwd. \
         OCR backend: Tesseract CLI"
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["capture", "info", "ocr"],
                    "description": "Action: capture (take screenshot), info (screen dimensions), or ocr (extract text from image)"
                },
                "x": {
                    "type": "integer",
                    "description": "Region X offset (for capture with region)"
                },
                "y": {
                    "type": "integer",
                    "description": "Region Y offset (for capture with region)"
                },
                "width": {
                    "type": "integer",
                    "description": "Region width in pixels (for capture with region)"
                },
                "height": {
                    "type": "integer",
                    "description": "Region height in pixels (for capture with region)"
                },
                "window_id": {
                    "type": "string",
                    "description": "X11 window ID in hex (e.g., 0x1234567) to capture a specific window"
                },
                "image_base64": {
                    "type": "string",
                    "description": "Base64-encoded image data for OCR (from a previous screenshot capture)"
                },
                "file_path": {
                    "type": "string",
                    "description": "Path to an image file on disk for OCR"
                },
                "language": {
                    "type": "string",
                    "description": "Tesseract language code (default: 'eng'). E.g., 'eng+fra' for multiple"
                },
                "expected_text": {
                    "type": "string",
                    "description": "Expected text to find on screen via OCR. If provided, the self-correction loop will automatically run OCR after capture and compare the result. Used for verifying that visual operations (e.g., keyboard input, text display) produced the expected output."
                }
            },
            "required": ["action"]
        })
    }

    fn categories(&self) -> Vec<&str> {
        vec!["screenshot", "screen", "visual", "system"]
    }

    fn side_effect_class(&self) -> SideEffectClass {
        SideEffectClass::ReadOnly
    }

    async fn execute(
        &self,
        _context: &ToolContext,
        args: serde_json::Value,
    ) -> Result<ToolOutput, ToolError> {
        let action = Self::get_str(&args, "action").unwrap_or("capture");
        match action {
            "capture" => self.cmd_capture(&args).await,
            "info" => self.cmd_info(&args).await,
            "ocr" => self.cmd_ocr(&args).await,
            _ => Err(ToolError::InvalidArguments(format!(
                "unknown screenshot action: '{action}'. Valid: capture, info, ocr"
            ))),
        }
    }
}

impl std::fmt::Debug for ScreenshotTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScreenshotTool")
            .field("backend", &self.backend)
            .field("has_xdpyinfo", &self.has_xdpyinfo)
            .field("has_tesseract", &self.has_tesseract)
            .finish()
    }
}

// ── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
#[path = "screenshot_test.rs"]
mod tests;
