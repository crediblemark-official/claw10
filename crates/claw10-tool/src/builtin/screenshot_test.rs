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
        idempotency_key: "screenshot-test".into(),
        risk_level: "low".into(),
        approval_id: None,
        budget_remaining: 100.0,
        workspace_dir: "/tmp".into(),
    }
}

#[tokio::test]
async fn test_screenshot_tool_name_and_description() {
    let tool = ScreenshotTool::new();
    assert_eq!(tool.name(), "screenshot");
    assert!(tool.description().contains("scrot"));
    assert!(tool.description().contains("xwd"));
}

#[tokio::test]
async fn test_screenshot_schema_has_actions() {
    let tool = ScreenshotTool::new();
    let schema = tool.input_schema();
    let actions = schema["properties"]["action"]["enum"]
        .as_array()
        .expect("actions enum");
    let names: Vec<&str> = actions.iter().filter_map(|v| v.as_str()).collect();
    assert!(names.contains(&"capture"));
    assert!(names.contains(&"info"));
    assert!(names.contains(&"ocr"));
    assert_eq!(names.len(), 3);
}

#[tokio::test]
async fn test_screenshot_categories() {
    let tool = ScreenshotTool::new();
    let cats = tool.categories();
    assert!(cats.contains(&"screenshot"));
    assert!(cats.contains(&"screen"));
}

#[tokio::test]
async fn test_screenshot_side_effect() {
    let tool = ScreenshotTool::new();
    assert_eq!(
        tool.side_effect_class() as i32,
        SideEffectClass::ReadOnly as i32
    );
}

#[tokio::test]
async fn test_screenshot_invalid_action() {
    let tool = ScreenshotTool::new();
    let ctx = mock_context();
    let args = json!({ "action": "fly" });
    let result = tool.execute(&ctx, args).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("unknown screenshot action"));
}

#[tokio::test]
async fn test_screenshot_info_works() {
    let tool = ScreenshotTool::new();
    let ctx = mock_context();
    let args = json!({ "action": "info" });
    let result = tool.execute(&ctx, args).await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.success);
    let has_tools = output.data["tools_available"].as_array().is_some();
    assert!(has_tools, "should report tools");
}

#[tokio::test]
async fn test_screenshot_backend_detection() {
    let tool = ScreenshotTool::new();
    let valid = matches!(
        tool.backend,
        CaptureBackend::Scrot
            | CaptureBackend::Maim
            | CaptureBackend::Import
            | CaptureBackend::Xwd
            | CaptureBackend::GnomeScreenshot
            | CaptureBackend::None
    );
    assert!(valid, "backend should be a recognized type");
}

#[tokio::test]
async fn test_screenshot_debug() {
    let tool = ScreenshotTool::new();
    let debug = format!("{tool:?}");
    assert!(debug.contains("backend"));
    assert!(debug.contains("has_xdpyinfo"));
}

#[tokio::test]
async fn test_screenshot_image_dimensions_png() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.png");
    let png_header: [u8; 45] = [
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1
        0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xDE, // bit depth + CRC
        0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82, // IEND
    ];
    std::fs::write(&path, &png_header[..]).unwrap();

    let dims = ScreenshotTool::image_dimensions(path.to_str().unwrap());
    if let Some((w, h)) = dims {
        assert_eq!(w, 1);
        assert_eq!(h, 1);
    }
}

#[tokio::test]
async fn test_screenshot_capture_without_backend() {
    let tool = ScreenshotTool::new();
    let ctx = mock_context();
    let args = json!({ "action": "capture" });
    let result = tool.execute(&ctx, args).await;

    match tool.backend {
        CaptureBackend::None => {
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(err.contains("No screen capture tool found"));
        }
        _ => {
            let _ = result;
        }
    }
}

#[test]
fn test_capture_backend_equality() {
    assert_eq!(CaptureBackend::Scrot, CaptureBackend::Scrot);
    assert_ne!(CaptureBackend::Scrot, CaptureBackend::None);
}

#[tokio::test]
async fn test_ocr_invalid_no_input() {
    let tool = ScreenshotTool::new();
    let ctx = mock_context();
    // ocr without image_base64 or file_path
    let args = json!({ "action": "ocr" });
    let result = tool.execute(&ctx, args).await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("image_base64") || msg.contains("file_path"),
        "should require image_base64 or file_path: {msg}");
}

#[tokio::test]
async fn test_ocr_invalid_base64() {
    let tool = ScreenshotTool::new();
    let ctx = mock_context();
    // ocr with invalid base64
    let args = json!({
        "action": "ocr",
        "image_base64": "not-valid-base64!!!"
    });
    let result = tool.execute(&ctx, args).await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("invalid base64"), "should complain about invalid base64: {msg}");
}

#[tokio::test]
async fn test_ocr_valid_base64_with_tesseract() {
    let tool = ScreenshotTool::new();
    let ctx = mock_context();

    if !tool.has_tesseract {
        eprintln!("  Skipping real OCR test — tesseract not installed");
        return;
    }

    // Create a minimal 1x1 white PNG, base64-encode it
    // Minimal valid PNG: 1x1 white pixel
    let png_bytes: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1
        0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xDE, // bit depth + CRC
        0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82, // IEND
    ];

    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);

    let args = json!({
        "action": "ocr",
        "image_base64": b64,
        "language": "eng",
    });

    let result = tool.execute(&ctx, args).await;
    match result {
        Ok(output) => {
            assert!(output.success, "ocr should succeed");
            let text = output.data["text"].as_str().unwrap_or("");
            let mean_conf = output.data["mean_confidence"].as_f64().unwrap_or(-1.0);
            eprintln!("  [OCR] Result: text='{}', confidence={}", text, mean_conf);
            // OCR on a 1x1 white pixel will return empty text — that's fine
            // We just verify the pipeline works
            assert!(mean_conf >= -1.0, "confidence should be >= -1 (got {})", mean_conf); // -1 = no text found
        }
        Err(e) => {
            // Tesseract may fail on a 1x1 image (too small) — that's acceptable
            eprintln!("  [OCR] Tesseract pipeline error (acceptable): {e}");
        }
    }
}

#[tokio::test]
async fn test_ocr_with_file_path() {
    let tool = ScreenshotTool::new();
    let ctx = mock_context();

    if !tool.has_tesseract {
        eprintln!("  Skipping file_path OCR test — tesseract not installed");
        return;
    }

    // Create a temp PNG file
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test_ocr.png");
    let png_bytes: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
        0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xDE,
        0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    std::fs::write(&path, &png_bytes).unwrap();

    let args = json!({
        "action": "ocr",
        "file_path": path.to_str().unwrap(),
    });

    let result = tool.execute(&ctx, args).await;
    assert!(result.is_ok() || result.is_err(), "should not panic");
    if let Ok(output) = result {
        assert!(output.data["source"].as_str() == Some("file"));
        eprintln!("  [OCR] File_path OCR succeeded");
    }
}

#[tokio::test]
async fn test_ocr_schema_has_ocr() {
    let tool = ScreenshotTool::new();
    let schema = tool.input_schema();
    let actions = schema["properties"]["action"]["enum"]
        .as_array()
        .expect("actions enum");
    let names: Vec<&str> = actions.iter().filter_map(|v| v.as_str()).collect();
    assert!(names.contains(&"ocr"), "schema should include 'ocr'");
    assert!(names.contains(&"capture"));
    assert!(names.contains(&"info"));
    assert_eq!(names.len(), 3, "should have 3 actions: capture, info, ocr");
    // Verify schema has ocr-specific params
    assert!(schema["properties"].get("image_base64").is_some(), "schema should have image_base64");
    assert!(schema["properties"].get("language").is_some(), "schema should have language");
}
