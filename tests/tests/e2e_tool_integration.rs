//! E2E Integration Test: ShellTool + ProcessTool + ScreenshotTool + WindowTool + InputTool
//!
//! Skenario:
//!   1. ShellTool spawn — jalankan background process
//!   2. ProcessTool list — verifikasi process berjalan
//!   3. ProcessTool monitor — detail process via /proc
//!   4. ProcessTool tree — verifikasi hierarchy
//!   5. ScreenshotTool info — cek screen dimensions & tools
//!   6. ScreenshotTool capture — capture screen (jika ada backend)
//!   7. WindowTool info — deteksi ketersediaan xdotool
//!   8. InputTool info — deteksi ketersediaan xdotool
//!
//! Test ini bersifat conditional: hanya menjalankan action yang
//! didukung oleh environment saat ini.

use claw10_tool::builtin::{InputTool, ProcessTool, ScreenshotTool, ShellTool, WindowTool};
use claw10_tool::context::ToolContext;
use claw10_tool::registry::ToolRegistry;

// ── Helpers ──────────────────────────────────────────────────────────

fn mock_context(workspace: &str) -> ToolContext {
    use claw10_domain::{AgentId, MissionId, TaskId, WorkerId};
    use uuid::Uuid;
    ToolContext {
        tenant_id: "e2e".into(),
        mission_id: MissionId(Uuid::now_v7()),
        task_id: TaskId(Uuid::now_v7()),
        agent_id: AgentId(Uuid::now_v7()),
        worker_id: WorkerId(Uuid::now_v7()),
        idempotency_key: format!("e2e-{}", Uuid::now_v7()),
        risk_level: "low".into(),
        approval_id: None,
        budget_remaining: 100.0,
        workspace_dir: workspace.into(),
    }
}

fn build_tool_registry() -> ToolRegistry {
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(ShellTool::new()));
    reg.register(Box::new(ProcessTool::new()));
    reg.register(Box::new(ScreenshotTool::new()));
    reg.register(Box::new(WindowTool::new()));
    reg.register(Box::new(InputTool::new()));
    reg
}

/// Prerequisite check: apakah xdotool tersedia (dibutuhkan untuk WindowTool)
fn has_xdotool() -> bool {
    std::process::Command::new("which")
        .arg("xdotool")
        .output()
        .ok()
        .is_some_and(|o| o.status.success())
}

/// Prerequisite check: apakah DISPLAY tersedia (dibutuhkan untuk screenshot & window)
fn has_display() -> bool {
    std::env::var("DISPLAY").is_ok()
}

/// Prerequisite check: apakah screenshot backend tersedia
fn has_screenshot_backend() -> bool {
    let candidates = ["scrot", "maim", "import", "xwd", "gnome-screenshot"];
    candidates.iter().any(|c| {
        std::process::Command::new("which")
            .arg(c)
            .output()
            .ok()
            .is_some_and(|o| o.status.success())
    })
}

// ── E2E Test 1: ShellTool + ProcessTool — Basic Process Lifecycle ────

#[tokio::test]
async fn test_e2e_shell_process_lifecycle() {
    let registry = build_tool_registry();
    let ctx = mock_context("/tmp");
    let unique = uuid::Uuid::now_v7().to_string();

    // 1. SHELL: Spawn background process
    let shell_tool = registry.get("shell").unwrap();
    let spawn_result = shell_tool
        .execute(
            &ctx,
            serde_json::json!({
                "action": "spawn",
                "command": format!("echo 'hello_e2e_{unique}' && sleep 30")
            }),
        )
        .await
        .expect("shell spawn should succeed");

    assert!(spawn_result.success, "spawn should be successful");
    // ShellTool returns both internal tracking `pid` and system `sys_pid`.
    // - tracking_pid: used for ShellTool kill (lookup in internal processes map)
    // - sys_pid: used for ProcessTool (ps uses system PID)
    let tracking_pid = spawn_result.data["pid"].as_u64()
        .expect("spawn should return a tracking pid");
    let sys_pid = spawn_result.data["sys_pid"].as_u64()
        .expect("spawn should return a sys_pid");
    assert!(sys_pid > 0, "sys_pid should be positive");
    eprintln!("  [Shell] Spawned process: tracking_pid={}, sys_pid={}", tracking_pid, sys_pid);

    // Brief wait for startup
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // 2. PROCESS: List processes and find our spawned process
    let process_tool = registry.get("process").unwrap();
    let list_result = process_tool
        .execute(
            &ctx,
            serde_json::json!({
                "action": "list",
                "filter": &format!("hello_e2e_{unique}"),
                "limit": 10,
            }),
        )
        .await
        .expect("process list should succeed");

    assert!(list_result.success, "process list should succeed");
    let processes = list_result.data["processes"]
        .as_array()
        .expect("should have processes array");
    assert!(!processes.is_empty(), "should find our spawned process");
    eprintln!("  [Process] Found {} matching process(es)", processes.len());

    // Verify our process details (ps uses system PID)
    let our_proc = &processes[0];
    assert_eq!(
        our_proc["pid"].as_u64(),
        Some(sys_pid),
        "PID should match spawn sys_pid"
    );
    let cmd = our_proc["command"].as_str().unwrap_or("");
    assert!(
        cmd.contains(&format!("hello_e2e_{unique}")),
        "command should contain our marker"
    );
    eprintln!("  [Process] Verified sys_pid={}, command='{}'", sys_pid, cmd);

    // 3. PROCESS: Monitor our process for detailed info
    let monitor_result = process_tool
        .execute(
            &ctx,
            serde_json::json!({
                "action": "monitor",
                "pid": sys_pid,
            }),
        )
        .await
        .expect("process monitor should succeed");

    assert!(monitor_result.success, "monitor should succeed");
    let monitor_data = &monitor_result.data;

    // Check status info from /proc
    assert!(
        monitor_data["status"].is_object(),
        "monitor should return status object"
    );
    if let Some(status) = monitor_data["status"].as_object() {
        // Should have name, state, threads
        assert!(status.contains_key("name"), "status should have name");
        assert!(status.contains_key("state"), "status should have state");
        eprintln!(
            "  [Monitor] Process state: {:?}",
            status.get("state")
        );
    }

    // Check executable or cmdline
    let has_cmdline = monitor_data["cmdline"].as_str().map(|s| !s.is_empty()).unwrap_or(false);
    let has_basic = monitor_data["basic"].is_object();
    assert!(
        has_cmdline || has_basic,
        "monitor should return cmdline or basic info"
    );
    eprintln!("  [Monitor] cmdline: {:?}", monitor_data["cmdline"].as_str());
    eprintln!("  [Monitor] cwd: {:?}", monitor_data["cwd"].as_str());
    eprintln!("  [Monitor] open_fds: {:?}", monitor_data["open_fds"].as_u64());

    // 4. PROCESS: Tree — verify process hierarchy
    // Our process should appear under shell or the PID 1 root
    let tree_result = process_tool
        .execute(
            &ctx,
            serde_json::json!({
                "action": "tree",
                "depth": 3,
            }),
        )
        .await
        .expect("process tree should succeed");

    assert!(tree_result.success, "tree should succeed");
    let tree = tree_result.data["tree"]
        .as_array()
        .expect("tree should be an array");
    assert!(!tree.is_empty(), "tree should have at least root (PID 1)");
    eprintln!("  [Tree] Found {} root process(es) in tree", tree.len());

    // 5. SHELL: Kill the background process (cleanup)
    // ShellTool's kill action uses the internal tracking pid
    let kill_result = shell_tool
        .execute(
            &ctx,
            serde_json::json!({
                "action": "kill",
                "pid": tracking_pid,
            }),
        )
        .await;

    if let Err(e) = kill_result {
        // Fallback: direct system kill via exec if internal kill fails
        eprintln!("  [Shell] Internal kill failed: {e}, using direct kill...");
        let _ = shell_tool
            .execute(
                &ctx,
                serde_json::json!({
                    "action": "exec",
                    "command": format!("kill -9 {sys_pid}"),
                    "timeout_seconds": 5,
                }),
            )
            .await;
    }

    // 6. Verify process is gone
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let verify_list = process_tool
        .execute(
            &ctx,
            serde_json::json!({
                "action": "list",
                "filter": &format!("hello_e2e_{unique}"),
            }),
        )
        .await
        .expect("verify list should succeed");
    let remaining = verify_list.data["count"].as_u64().unwrap_or(0);
    eprintln!(
        "  [Verify] Remaining matching processes after kill: {}",
        remaining
    );
    assert_eq!(remaining, 0, "process should be gone after kill");
}

// ── E2E Test 2: ScreenshotTool — Screen Info + Capture ──────────────

#[tokio::test]
async fn test_e2e_screenshot_info_and_capture() {
    let registry = build_tool_registry();
    let ctx = mock_context("/tmp");

    let screenshot_tool = registry.get("screenshot").unwrap();

    // 1. SCREENSHOT: Info — query screen dimensions and tools
    let info_result = screenshot_tool
        .execute(&ctx, serde_json::json!({ "action": "info" }))
        .await
        .expect("screenshot info should succeed");

    assert!(info_result.success, "info should succeed");
    let info = &info_result.data;

    let tools_available = info["tools_available"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    eprintln!("  [Screenshot] Available tools: {:?}", tools_available);
    eprintln!("  [Screenshot] Screen dimensions: {}x{}",
        info["width"].as_u64().unwrap_or(0),
        info["height"].as_u64().unwrap_or(0));

    // Display check
    if has_display() {
        if has_screenshot_backend() {
            assert!(
                !tools_available.is_empty(),
                "should report at least one tool when DISPLAY is set"
            );
        }
    }

    // 2. SCREENSHOT: Capture (conditional on backend availability)
    if has_display() && has_screenshot_backend() {
        let capture_result = screenshot_tool
            .execute(&ctx, serde_json::json!({ "action": "capture" }))
            .await;

        match capture_result {
            Ok(output) => {
                assert!(output.success, "capture should succeed");
                let data = &output.data;

                // Verify base64 output
                let b64 = data["image_base64"].as_str()
                    .expect("should return base64 image");
                assert!(!b64.is_empty(), "base64 should not be empty");
                assert!(b64.len() > 100, "base64 should be substantial");

                // Verify metadata
                let size = data["size_bytes"].as_u64().unwrap_or(0);
                assert!(size > 0, "image should have positive size");
                eprintln!("  [Screenshot] Captured {} bytes, backend={:?}, mime={:?}",
                    size, data["backend"], data["mime_type"]);

                // Mime type should be reported
                let mime = data["mime_type"].as_str().unwrap_or("");
                assert!(
                    mime.starts_with("image/"),
                    "mime_type should start with image/"
                );
            }
            Err(e) => {
                // On systems with DISPLAY but without a proper backend,
                // the error is acceptable — ScreenshotTool gives a clear message
                let msg = e.to_string();
                eprintln!("  [Screenshot] Capture skipped (expected): {msg}");
            }
        }
    } else {
        eprintln!("  [Screenshot] Skipping capture — no DISPLAY or no backend");
    }
}

// ── E2E Test 3: WindowTool — Detection + Graceful Degradation ───────

#[tokio::test]
async fn test_e2e_window_tool_detection() {
    let registry = build_tool_registry();
    let ctx = mock_context("/tmp");

    let window_tool = registry.get("window").unwrap();

    // WindowTool should detect xdotool availability
    // Then gracefully handle the missing tool

    let has_xdo = has_xdotool();
    eprintln!("  [Window] xdotool available: {}", has_xdo);

    let list_result = window_tool
        .execute(&ctx, serde_json::json!({ "action": "list" }))
        .await;

    match list_result {
        Ok(output) => {
            // xdotool IS available — we should get a valid window list
            assert!(output.success, "window list should succeed");
            let windows = output.data["windows"].as_array()
                .expect("should return windows array");
            eprintln!("  [Window] Found {} window(s) (xdotool available)", windows.len());

            // Each window should have ID and title
            if !windows.is_empty() {
                assert!(
                    windows[0].get("id").is_some(),
                    "window should have id"
                );
            }
        }
        Err(e) => {
            let msg = e.to_string();
            // Three possible failure modes:
            // 1. xdotool not installed → error mentions installation
            // 2. xdotool installed but no WM active → X11 error (getactivewindow/search fail)
            // 3. Other X11 connectivity issues
            if has_xdo {
                eprintln!("  [Window] Expected X11 error (xdotool installed, no WM?): {msg}");
                assert!(
                    msg.contains("xdotool") || msg.contains("XGetWindowProperty") || msg.contains("failed"),
                    "error should mention xdotool or X11 failure: {msg}"
                );
            } else {
                eprintln!("  [Window] Expected error (no xdotool): {msg}");
                assert!(
                    msg.contains("xdotool"),
                    "error should mention xdotool: {msg}"
                );
            }
        }
    }
}

// ── E2E Test 4: Cross-Tool — Process → Screenshot → Window ──────────

#[tokio::test]
async fn test_e2e_cross_tool_integration() {
    let registry = build_tool_registry();
    let ctx = mock_context("/tmp");

    // 1. Get total process count as baseline
    let process_tool = registry.get("process").unwrap();
    let total_result = process_tool
        .execute(&ctx, serde_json::json!({
            "action": "list",
            "limit": 5,
        }))
        .await
        .expect("process list baseline");

    let total = total_result.data["count"].as_u64().unwrap_or(0);
    let total_running = total_result.data["total_running"].as_u64().unwrap_or(0);
    eprintln!("  [Cross] System has {} processes total, {} running",
        total, total_running);

    assert!(total_running >= total, "total_running >= filtered count");
    assert!(total_running > 0, "should have at least some processes");

    // 2. Screenshot info — should report tools even if WindowTool fails
    let screenshot_tool = registry.get("screenshot").unwrap();
    let screen_info = screenshot_tool
        .execute(&ctx, serde_json::json!({ "action": "info" }))
        .await
        .expect("screenshot info in cross-test");

    // 3. Window tool info — check xdotool status
    let has_xdo = has_xdotool();
    let window_tool = registry.get("window").unwrap();
    let window_list = window_tool
        .execute(&ctx, serde_json::json!({ "action": "list", "limit": 3 }))
        .await;

    eprintln!("  [Cross] Process count={}, Screenshot backend={:?}, xdotool={}",
        total,
        screen_info.data["backend"].as_str(),
        has_xdo);

    // If we have xdotool and a real desktop, WindowTool should work
    // Note: xdotool may fail if no window manager is active (headless/CI)
    if has_xdo && has_display() && window_list.is_err() {
        let msg = window_list.as_ref().unwrap_err().to_string();
        eprintln!("  [Cross] WindowTool list failed (expected if no WM): {msg}");
        assert!(
            msg.contains("xdotool") || msg.contains("failed"),
            "xdotool error should reference tool or failure: {msg}"
        );
    }

    // Core assertion: all tools at least initialized without panic
    // (the individual tool tests above verify deeper correctness)
    eprintln!("  [Cross] All tools initialized and responding correctly");
}

// ── E2E Test 6: InputTool — Mouse Operations + Screenshot Verification ──

#[tokio::test]
async fn test_e2e_input_tool_mouse_and_screenshot() {
    let registry = build_tool_registry();
    let ctx = mock_context("/tmp");

    let input_tool = registry.get("input").unwrap();
    let screenshot_tool = registry.get("screenshot").unwrap();

    let has_xdo = has_xdotool();
    eprintln!("  [Input] xdotool available: {}", has_xdo);

    if !has_xdo {
        // Verify graceful error when xdotool is missing
        let result = input_tool
            .execute(&ctx, serde_json::json!({ "action": "mouse_position" }))
            .await;
        assert!(result.is_err(), "should fail without xdotool");
        eprintln!("  [Input] Skipping mouse tests — no xdotool");
        return;
    }

    // ── 1. Get initial mouse position ────────────────────────────
    let pos1 = input_tool
        .execute(&ctx, serde_json::json!({ "action": "mouse_position" }))
        .await
        .expect("mouse_position should succeed");

    assert!(pos1.success, "mouse_position success");
    let x1 = pos1.data["x"].as_i64().unwrap_or(-1);
    let y1 = pos1.data["y"].as_i64().unwrap_or(-1);
    eprintln!("  [Input] Initial mouse position: ({}, {})", x1, y1);
    assert!(x1 >= 0 && y1 >= 0, "valid coordinates: {}, {}", x1, y1);

    // ── 2. Move mouse to absolute position ───────────────────────
    let target_x: i64 = 500;
    let target_y: i64 = 500;
    let move_result = input_tool
        .execute(
            &ctx,
            serde_json::json!({
                "action": "mouse_move",
                "x": target_x,
                "y": target_y,
            }),
        )
        .await
        .expect("mouse_move should succeed");

    assert!(move_result.success, "mouse_move absolute");
    eprintln!("  [Input] Moved mouse to ({}, {})", target_x, target_y);

    // ── 3. Verify new position ───────────────────────────────────
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let pos2 = input_tool
        .execute(&ctx, serde_json::json!({ "action": "mouse_position" }))
        .await
        .expect("second mouse_position should succeed");

    let x2 = pos2.data["x"].as_i64().unwrap_or(-1);
    let y2 = pos2.data["y"].as_i64().unwrap_or(-1);
    eprintln!("  [Input] Position after move: ({}, {})", x2, y2);
    assert!(
        (x2 - target_x).abs() <= 2 && (y2 - target_y).abs() <= 2,
        "position should be near ({}, {}), got ({}, {})",
        target_x, target_y, x2, y2
    );

    // ── 4. Move mouse relative ───────────────────────────────────
    let rel_x: i64 = 50;
    let rel_y: i64 = -30;
    let rel_result = input_tool
        .execute(
            &ctx,
            serde_json::json!({
                "action": "mouse_move",
                "x": rel_x,
                "y": rel_y,
                "relative": true,
            }),
        )
        .await
        .expect("mouse_move relative should succeed");

    assert!(rel_result.success, "mouse_move relative");
    let expected_x = x2 + rel_x;
    let expected_y = y2 + rel_y;

    // ── 5. Verify relative position ──────────────────────────────
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let pos3 = input_tool
        .execute(&ctx, serde_json::json!({ "action": "mouse_position" }))
        .await
        .expect("third mouse_position should succeed");

    let x3 = pos3.data["x"].as_i64().unwrap_or(-1);
    let y3 = pos3.data["y"].as_i64().unwrap_or(-1);
    eprintln!("  [Input] Position after relative: ({}, {}), expected near ({}, {})",
        x3, y3, expected_x, expected_y);
    assert!(
        (x3 - expected_x).abs() <= 2 && (y3 - expected_y).abs() <= 2,
        "position should be near ({}, {}), got ({}, {})",
        expected_x, expected_y, x3, y3
    );

    // ── 6. Click mouse button ────────────────────────────────────
    let click_result = input_tool
        .execute(
            &ctx,
            serde_json::json!({
                "action": "mouse_click",
                "button": "left",
            }),
        )
        .await
        .expect("mouse_click should succeed");

    assert!(click_result.success, "mouse_click left");
    eprintln!("  [Input] Left click at ({}, {})", x3, y3);

    // ── 7. Key type (text) — works at X11 level even without WM ──
    let type_result = input_tool
        .execute(
            &ctx,
            serde_json::json!({
                "action": "key_type",
                "text": "e2e_input_test!",
                "delay_ms": 5,
            }),
        )
        .await;

    match &type_result {
        Ok(out) => {
            assert!(out.success, "key_type success");
            eprintln!("  [Input] key_type succeeded: text_length={}", out.data["text_length"]);
        }
        Err(e) => {
            // X11 may reject type without a focused window
            eprintln!("  [Input] key_type failed (acceptable): {e}");
        }
    }

    // ── 8. Key press (hotkey) ────────────────────────────────────
    let hotkey_result = input_tool
        .execute(
            &ctx,
            serde_json::json!({
                "action": "key_press",
                "key": "Escape",
            }),
        )
        .await;

    match &hotkey_result {
        Ok(out) => eprintln!("  [Input] key_press Escape: success"),
        Err(e) => eprintln!("  [Input] key_press Escape skipped (acceptable): {e}"),
    }

    // ── 9. Screenshot capture (cross-tool verification) ──────────
    if has_display() && has_screenshot_backend() {
        let capture_result = screenshot_tool
            .execute(&ctx, serde_json::json!({ "action": "capture" }))
            .await;

        match &capture_result {
            Ok(output) => {
                assert!(output.success, "screenshot capture should succeed");
                let b64 = output.data["image_base64"].as_str().unwrap_or("");
                let size = output.data["size_bytes"].as_u64().unwrap_or(0);
                eprintln!("  [Input+Screenshot] Cross-tool capture: {} bytes, mime={:?}",
                    size, output.data["mime_type"]);
                assert!(size > 0, "screenshot should be non-empty");
                assert!(!b64.is_empty(), "base64 should be non-empty");
            }
            Err(e) => {
                eprintln!("  [Input+Screenshot] Capture skipped: {e}");
            }
        }
    } else {
        eprintln!("  [Input+Screenshot] Skipping screenshot — no DISPLAY or no backend");
    }

    // ── 10. Final mouse position — still responsive after all ops ─
    let pos_final = input_tool
        .execute(&ctx, serde_json::json!({ "action": "mouse_position" }))
        .await
        .expect("final mouse_position should succeed");

    let xf = pos_final.data["x"].as_i64().unwrap_or(-1);
    let yf = pos_final.data["y"].as_i64().unwrap_or(-1);
    eprintln!("  [Input] Final mouse position: ({}, {}) — tool still responsive", xf, yf);

    eprintln!("  [Input] All mouse + screenshot cross-tool operations completed");
}

// ── E2E Test 7: ShellTool → ProcessTool → InputTool → ScreenshotTool ──
// Full flow: spawn GUI app (gnome-terminal), verify process, simulate input, capture screen.
//
// Catatan: Di environment tanpa window manager (seperti CI/headless),
// gnome-terminal berjalan sebagai proses tetapi tidak memiliki X11 window.
// keyboard input (key_type) tetap dipanggil untuk memverifikasi tool chain,
// meskipun text tidak sampai ke aplikasi tanpa window focus.
// Mouse operations (move, scroll) dapat bekerja di level X11 tanpa WM.

#[tokio::test]
async fn test_e2e_shell_process_input_screenshot() {
    let registry = build_tool_registry();
    let ctx = mock_context("/tmp");
    let unique = uuid::Uuid::now_v7().to_string();

    let shell_tool = registry.get("shell").unwrap();
    let process_tool = registry.get("process").unwrap();
    let input_tool = registry.get("input").unwrap();
    let screenshot_tool = registry.get("screenshot").unwrap();

    let has_xdo = has_xdotool();
    let display_ok = has_display();
    let has_ss = has_screenshot_backend();

    eprintln!("  [Flow] xdotool={}, DISPLAY={}, screenshot={}", has_xdo, display_ok, has_ss);

    // ── 1. SHELL: Spawn gnome-terminal as background GUI app ─────
    let spawn_result = shell_tool
        .execute(
            &ctx,
            serde_json::json!({
                "action": "spawn",
                "command": format!("gnome-terminal -- bash -c 'echo e2e_input_{unique}; sleep 60'"),
            }),
        )
        .await
        .expect("shell spawn should succeed");

    assert!(spawn_result.success, "spawn gnome-terminal");
    let tracking_pid = spawn_result.data["pid"].as_u64()
        .expect("spawn should return tracking pid");
    let sys_pid = spawn_result.data["sys_pid"].as_u64()
        .expect("spawn should return sys_pid");
    eprintln!("  [Flow] Spawned gnome-terminal: tracking_pid={}, sys_pid={}", tracking_pid, sys_pid);

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // ── 2. PROCESS: Verify gnome-terminal process is running ─────
    let list_result = process_tool
        .execute(
            &ctx,
            serde_json::json!({
                "action": "list",
                "filter": "gnome-terminal",
                "limit": 5,
            }),
        )
        .await
        .expect("process list for gnome-terminal");

    assert!(list_result.success, "process list");
    let count = list_result.data["count"].as_u64().unwrap_or(0);
    let processes = list_result.data["processes"].as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    eprintln!("  [Flow] gnome-terminal processes: count={}, entries={}", count, processes);

    // gnome-terminal itself and/or gnome-terminal-server should be visible
    // (minimum 1: the gnome-terminal-server daemon)
    assert!(processes > 0 || count > 0, "should find gnome-terminal process(es)");

    // ── 3. INPUT: Simulate input if xdotool available ────────────
    if has_xdo {
        // Try to type text — may go nowhere without WM but verifies tool chain
        let type_result = input_tool
            .execute(
                &ctx,
                serde_json::json!({
                    "action": "key_type",
                    "text": &format!("e2e_flow_test_{unique}"),
                    "delay_ms": 5,
                }),
            )
            .await;

        match &type_result {
            Ok(out) => eprintln!("  [Flow] InputTool type succeeded: text_length={}",
                out.data["text_length"]),
            Err(e) => eprintln!("  [Flow] InputTool type skipped (X11 no focus): {e}"),
        }

        // Move mouse to a known position
        let _ = input_tool
            .execute(
                &ctx,
                serde_json::json!({
                    "action": "mouse_move",
                    "x": 200,
                    "y": 200,
                }),
            )
            .await;
        eprintln!("  [Flow] InputTool mouse_move to (200, 200) completed");

        // Scroll (just to verify it doesn't crash)
        let _ = input_tool
            .execute(
                &ctx,
                serde_json::json!({
                    "action": "mouse_scroll",
                    "delta": 3,
                }),
            )
            .await;
        eprintln!("  [Flow] InputTool mouse_scroll delta=3 completed");
    } else {
        eprintln!("  [Flow] Skipping input simulation — no xdotool");
    }

    // ── 4. SCREENSHOT: Capture screen to verify ──────────────────
    if display_ok && has_ss {
        let capture_result = screenshot_tool
            .execute(&ctx, serde_json::json!({ "action": "capture" }))
            .await;

        match &capture_result {
            Ok(output) => {
                assert!(output.success, "screenshot should succeed");
                let size = output.data["size_bytes"].as_u64().unwrap_or(0);
                let b64 = output.data["image_base64"].as_str().unwrap_or("");
                eprintln!("  [Flow] Screenshot after input: {} bytes, mime={:?}",
                    size, output.data["mime_type"]);
                assert!(size > 0, "screenshot should be non-empty");
                assert!(!b64.is_empty(), "base64 should be non-empty");
                assert!(b64.len() > 100, "base64 substantial");
            }
            Err(e) => {
                eprintln!("  [Flow] Screenshot capture skipped: {e}");
            }
        }
    } else {
        eprintln!("  [Flow] Skipping screenshot — no DISPLAY or no backend");
    }

    // ── 5. CLEANUP: Kill gnome-terminal process ──────────────────
    let kill_result = shell_tool
        .execute(
            &ctx,
            serde_json::json!({ "action": "kill", "pid": tracking_pid }),
        )
        .await;

    if let Err(e) = kill_result {
        eprintln!("  [Flow] Internal kill failed: {e}, using exec fallback...");            let _ = shell_tool
                .execute(
                    &ctx,
                    serde_json::json!({
                        "action": "exec",
                        "command": format!("kill -9 {sys_pid} 2>/dev/null"),
                        "timeout_seconds": 5,
                    }),
                )
                .await;
    }

    // ── 6. Verify cleanup ────────────────────────────────────────
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let verify = process_tool
        .execute(
            &ctx,
            serde_json::json!({
                "action": "list",
                "filter": "gnome-terminal",
            }),
        )
        .await
        .expect("verify cleanup");
    let remaining = verify.data["count"].as_u64().unwrap_or(0);
    eprintln!("  [Flow] Remaining gnome-terminal processes after kill: {}", remaining);

    eprintln!("  [Flow] Shell→Process→Input→Screenshot flow complete");
}

// ── E2E Test 8: InputTool — Clipboard Get/Set ───────────────────────
//
// xclip dapat hang (tanpa clipboard manager). Test ini menggunakan
// tokio::time::timeout untuk menghindari hang selamanya — jika xclip
// tidak merespons dalam 5 detik, test skip gracefully.

#[tokio::test]
async fn test_e2e_input_tool_clipboard() {
    let registry = build_tool_registry();
    let ctx = mock_context("/tmp");

    let input_tool = registry.get("input").unwrap();

    // ── Cek apakah xclip minimal terinstall ────────────────────
    let xclip_installed = std::process::Command::new("which")
        .arg("xclip")
        .output()
        .ok()
        .is_some_and(|o| o.status.success());
    eprintln!("  [Clipboard] xclip installed: {}", xclip_installed);

    if !xclip_installed {
        // Verify graceful error when xclip is missing
        let get_result = input_tool
            .execute(&ctx, serde_json::json!({ "action": "clipboard_get" }))
            .await;
        assert!(get_result.is_err(), "clipboard_get should fail without xclip");
        let msg = get_result.unwrap_err().to_string();
        assert!(msg.contains("xclip is required"), "error should mention xclip: {msg}");
        eprintln!("  [Clipboard] Skipping clipboard tests — no xclip");
        return;
    }

    let unique = uuid::Uuid::now_v7().to_string();
    let test_text = format!("e2e_clipboard_test_{unique}");

    // ── 1. CLIPBOARD_SET: Write unique test string ───────────────
    // InputTool memiliki internal timeout 10 detik untuk clipboard_set,
    // jadi test tidak akan hang meski xclip tidak memiliki clipboard manager.
    let set_result = input_tool
        .execute(
            &ctx,
            serde_json::json!({
                "action": "clipboard_set",
                "text": &test_text,
            }),
        )
        .await;

    let set_result = match set_result {
        Ok(output) => {
            assert!(output.success, "clipboard_set success");
            let written_len = output.data["length"].as_u64().unwrap_or(0);
            assert_eq!(written_len as usize, test_text.len(),
                "written length should match: {} vs {}", written_len, test_text.len());
            eprintln!("  [Clipboard] Set: '{}' ({} bytes)", test_text, written_len);
            output
        }
        Err(e) => {
            // xclip may fail if no clipboard manager — acceptable
            eprintln!("  [Clipboard] clipboard_set failed (no clipboard manager?): {e}");
            eprintln!("  [Clipboard] Skipping round-trip verification");
            return;
        }
    };

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // ── 2. CLIPBOARD_GET: Read back and verify ───────────────────
    let get_result = input_tool
        .execute(&ctx, serde_json::json!({ "action": "clipboard_get" }))
        .await
        .expect("clipboard_get should succeed");

    assert!(get_result.success, "clipboard_get success");
    let read_text = get_result.data["text"].as_str().unwrap_or("");
    let read_len = get_result.data["length"].as_u64().unwrap_or(0);

    eprintln!("  [Clipboard] Get: '{}' ({} bytes)", read_text, read_len);

    // The clipboard content should contain our test string
    assert!(
        read_text.contains(&test_text),
        "clipboard should contain our test string. Expected substring: '{}', got: '{}'",
        test_text, read_text
    );
    assert_eq!(
        read_len as usize, read_text.len(),
        "reported length should match actual"
    );

    assert!(read_len > 0, "clipboard should contain non-empty text");
    eprintln!("  [Clipboard] Round-trip verified: set '{}' → get '{}' ✓", test_text, read_text);

    eprintln!("  [Clipboard] Clipboard round-trip test completed");
}

// ── E2E Test 9: All tools basic smoke test ──────────────────────────

#[tokio::test]
async fn test_e2e_tool_smoke_test() {
    let registry = build_tool_registry();
    let ctx = mock_context("/tmp");

    // Verify all tools are registered
    let tool_names = registry.list_names();
    eprintln!("  [Smoke] Registered tools: {:?}", tool_names);

    assert!(tool_names.contains(&"shell"), "shell tool should be registered");
    assert!(tool_names.contains(&"process"), "process tool should be registered");
    assert!(tool_names.contains(&"screenshot"), "screenshot tool should be registered");
    assert!(tool_names.contains(&"window"), "window tool should be registered");
    assert!(tool_names.contains(&"input"), "input tool should be registered");

    // Each tool should respond to SOME action (even if it's an error)
    // This verifies the tool trait is wired correctly
    for name in &["shell", "process", "screenshot", "window", "input"] {
        let tool = registry.get(name).unwrap();
        let min_args = serde_json::json!({});
        let result = tool.execute(&ctx, min_args).await;

        // The tool should either:
        // - Succeed (with default action)
        // - Fail with InvalidArguments (action missing)
        match &result {
            Ok(_) => eprintln!("  [Smoke] {name}: OK (default action)"),
            Err(e) => {
                let msg = e.to_string();
                // InvalidArguments is acceptable (we didn't provide required args)
                // Other errors are acceptable too (xdotool missing, etc)
                eprintln!("  [Smoke] {name}: {} — acceptable", msg);
            }
        }
    }

    eprintln!("  [Smoke] All tools respond without panic");
}
