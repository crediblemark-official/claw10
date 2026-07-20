# Claw10 OS — Big Plan

> Visi: Seperti manusia yang mengoperasikan komputer untuk menjalankan apapun dan mengoperasikan apapun.

---

## Daftar Isi

1. Visi & Filosofi
2. Arsitektur Saat Ini
3. Kesenjangan: Apa yang Sudah & Belum Ada
4. Roadmap: 7 Phase
5. Spesifikasi Tool
6. Spesifikasi Agent Baru
7. Arsitektur Target
8. Tahap Implementasi Detail
9. Risiko & Mitigasi
10. Referensi Lama

---

## 1. Visi & Filosofi

### 1.1 Visi

Claw10 OS adalah AI agent yang berjalan di terminal Linux, mampu **mengoperasikan komputer seperti manusia**: melihat layar, menggerakkan mouse, mengetik keyboard, menjalankan aplikasi, mengelola jendela, dan menyelesaikan tugas apapun yang bisa dilakukan manusia di depan komputer.

### 1.2 Prinsip Dasar

- **Text-first, Visual-fallback** — Gunakan shell/ps/xdotool dulu; screenshot hanya jika perlu
- **Human-like** — Input sama seperti manusia: klik, type, scroll, drag, shortcut
- **No special API** — Tidak harus ada API khusus; bisa pakai GUI seperti manusia
- **Shell + Visual** — Shell untuk command, visual untuk interaksi GUI
- **Token-efficient** — Screenshot hanya saat benar-benar perlu; prefer text-based inspection
- **Continuous learning** — Setiap tugas yang gagal menjadi pelajaran
- **Goal decomposition** — Tugas besar dipecah menjadi langkah-langkah kecil

### 1.3 Strategi Token Efficiency

Screenshot mahal (1 gambar = ribuan token). Strategi hemat:

| Prioritas | Metode | Token Cost | Kapan Pakai |
|-----------|--------|------------|-------------|
| 1 | xdotool/xprop | Sangat rendah | Cari window, baca title, cek geometry |
| 2 | ps/procfs | Sangat rendah | List proses, monitor status |
| 3 | Accessibility tree (AT-SPI2) | Rendah | Baca struktur UI, cari element |
| 4 | OCR (text-only crop) | Sedang | Baca text spesifik di layar |
| 5 | Screenshot | Tinggi | Hanya saat semua di atas gagal |

**Aturan:**
- Screenshot hanya 1x per task (awal), bukan per action
- Prefer xdotool search/find untuk locate element
- Prefer ps/proc untuk cek process state
- Prefer accessibility tree untuk navigate UI
- Crop region jika harus screenshot (jangan full screen)
- Cache screenshot jika state belum berubah

### 1.4 Perbedaan dari Agent Lain

| Fitur | Agent Lain | Claw10 OS |
|-------|-----------|-----------|
| Input | Keyboard only (shell) | Keyboard + Mouse + Screen |
| Awareness | Command output | Screenshot + Accessibility Tree |
| GUI interaction | Tidak bisa | Klik, type, drag, scroll |
| Process monitoring | Tidak ada | Lihat semua process berjalan |
| Window management | Tidak ada | Switch, resize, close window |
| Learning | Tidak ada | Self-correction via screenshot |

---

## 2. Arsitektur Saat Ini

### 2.1 Struktur Crate (28 crates)

```
claw10/
  crates/
    claw10-agent/         # Agent runtime: turn loop, session, executor
    claw10-artifact/      # Artifact storage & hashing
    claw10-auth/          # Authentication & credential management
    claw10-budget/        # Usage budget tracking & enforcement
    claw10-cli/           # CLI entrypoint: serve, tui, run-agent, setup
    claw10-context/       # Context building & management
    claw10-control-api/   # HTTP API (axum): handlers, middleware, telegram poller
    claw10-domain/        # Core domain types: Agent, Task, Mission, Tool, etc.
    claw10-event/         # Event bus: NATS + in-memory implementations
    claw10-gateway/       # Gateway lifecycle governance
    claw10-icvs/          # Integrity-verifiable content-addressed storage
    claw10-lifecycle/     # Agent lifecycle management
    claw10-lineage/       # Lineage tracking (parent-child relationships)
    claw10-memory/        # Memory system: recall, store, admission control
    claw10-mission/       # Mission management
    claw10-model-router/  # LLM model routing: provider abstraction, config
    claw10-policy/        # Policy enforcement engine
    claw10-prompt/        # Prompt assembly: base kernel, roles, injection, ICVS
    claw10-scheduler/     # Task scheduler with cron support
    claw10-skill/         # Skill system
    claw10-spawn/         # Agent spawning: broker, validator, descendant
    claw10-store/         # Key-value store: sled + in-memory backends
    claw10-task/          # Task management
    claw10-telemetry/     # Observability: structured event logging
    claw10-toon/          # Token-aware context management
    claw10-tool/          # Tool registry + built-in tools (shell, browser, window, process, screenshot, artifact)
    claw10-tui/           # Terminal UI (ratatui): chat, home, screens, model selection
    claw10-worker/        # Worker management & registration
```

### 2.2 Flow Saat Ini

```
User Input
  -> CLI (claw10 serve/tui/run-agent)
  -> Agent Runtime (turn loop)
     - Prompt Assembly (base kernel + roles + injection)
     - Model Call (via model-router)
     - Tool Execution (via tool-registry)
       - ShellTool, BrowserTool, WindowTool, ProcessTool,
         ScreenshotTool, DeclareArtifactTool, SpawnTool
     - Memory Recall/Store (via memory system)
     - Budget Check (via budget service)
     - Event Emission (via event bus)
  -> Response (TUI / CLI / HTTP)
```

### 2.3 Tool yang Sudah Ada

| Tool | Fungsi | Backend | Status |
|------|--------|---------|--------|
| ShellTool | Eksekusi shell command + background process | `sh -c`, tokio::process | Stabil ✅ |
| BrowserTool | Headless Chrome automation | chromiumoxide + CDP | Stabil ✅ |
| WindowTool | Manajemen jendela desktop | xdotool + xprop | Stabil ✅ |
| ProcessTool | Manajemen proses sistem | ps + kill + /proc | Stabil ✅ |
| ScreenshotTool | Screen capture | scrot > maim > import > xwd | Stabil ✅ |
| DeclareArtifactTool | Registrasi artifact | claw10-artifact + KV store | Stabil ✅ |
| SpawnTool | Spawn sub-agent | claw10-spawn + KV store | Stabil ✅ |

### 2.4 Komponen yang Sudah Diintegrasikan

| Komponen | Fungsi | Status |
|----------|--------|--------|
| Lifecycle | Event loop with cancellation | Aktif |
| Model Router | Multi-provider LLM routing (OpenAI, Anthropic, Google, Bedrock, Ollama) | Aktif |
| Prompt Assembly | Base kernel + roles + injection + policy digest | Aktif |
| Budget | Soft/hard limit budget enforcement | Aktif |
| Memory | Session memory + admission control | Aktif |
| Policy | Tool call policy enforcement | Aktif |
| Event Bus | NATS + in-memory event distribution | Aktif |
| Telemetry | Structured JSON logging (Vector-compatible) | Aktif |

---

## 3. Kesenjangan: Apa yang Sudah & Belum Ada

### 3.1 Kesadaran Layar (Screen Awareness)

**Status: SUDAH ADA (dasar)**

ScreenshotTool sudah bisa capture layar, region, dan window. Tapi:
- ✅ Screenshot capture (scrot / maim / import / xwd)
- ❌ Image analysis (belum ada integrasi vision model)
- ❌ OCR untuk membaca text di layar
- ❌ UI element detection (tombol, input field, menu)

### 3.2 Simulasi Input Manusia (Mouse & Keyboard)

**Status: BELUM ADA**

Agent tidak bisa menggerakkan mouse atau mengetik keyboard. Dia hanya bisa menulis ke stdin process.
- Bisa pakai xdotool (X11) atau enigo (cross-platform)
- Ini prioritas tertinggi berikutnya

### 3.3 Manajemen Proses (Process Monitoring)

**Status: SUDAH ADA**

ProcessTool sudah bisa:
- ✅ List semua proses (dengan filter)
- ✅ Monitor detail proses (/proc/pid/status)
- ✅ Kill/stop processes
- ✅ Process tree (parent-child hierarchy)

### 3.4 Manajemen Jendela (Window Management)

**Status: SUDAH ADA**

WindowTool sudah bisa:
- ✅ List semua window (title, geometry, focused state)
- ✅ Focus/switch window
- ✅ Close window (graceful atau force)
- ✅ Resize dan move window
- ✅ Search window by title

### 3.5 Accessibility Tree

**Status: BELUM ADA**

Agent tidak bisa membaca struktur UI dari aplikasi via AT-SPI2.

### 3.6 Self-Correction Loop

**Status: BELUM ADA**

Memory system ada tapi belum ada self-correction loop:
- ❌ Verify action result via text-based check
- ❌ Screenshot sebagai fallback verification
- ❌ Auto-retry with different approach
- ❌ Update memory dengan lessons learned

### 3.7 Goal Decomposition

**Status: BELUM ADA**

Task decomposition belum adaptive:
- ❌ Break complex goal into sub-goals
- ❌ Track progress per sub-goal
- ❌ Re-plan if a sub-goal fails

---

## 4. Roadmap: 7 Phase

### Phase 1: Process & Window Management ✅ (SELESAI)

**Goal:** Agent bisa melihat dan mengelola proses + jendela via text-based commands

| Deliverable | Tool | Backend | Status |
|-------------|------|---------|--------|
| List/filter processes | ProcessTool | ps + /proc | ✅ |
| Process hierarchy tree | ProcessTool | ps --ppid | ✅ |
| Kill/signal processes | ProcessTool | kill | ✅ |
| Monitor process detail | ProcessTool | /proc/pid/status | ✅ |
| List windows | WindowTool | xdotool search | ✅ |
| Focus/switch windows | WindowTool | xdotool windowactivate | ✅ |
| Close windows | WindowTool | xdotool windowclose | ✅ |
| Resize/move windows | WindowTool | xdotool windowsize/move | ✅ |
| Search windows by title | WindowTool | xdotool search --name | ✅ |

**Acceptance criteria:**
- ✅ Bisa list & filter proses berdasarkan nama/user
- ✅ Bisa fokus ke window tertentu
- ✅ Bisa kill process
- ✅ Bisa resize/move window

### Phase 2: Screen Capture ✅ (SELESAI)

**Goal:** Agent bisa melihat layar via screenshot

| Deliverable | Tool | Backend | Status |
|-------------|------|---------|--------|
| Full screen capture | ScreenshotTool | scrot > maim > import > xwd | ✅ |
| Region capture | ScreenshotTool | Crop parameter | ✅ |
| Window capture | ScreenshotTool | Window ID parameter | ✅ |
| Base64 output | ScreenshotTool | base64 crate | ✅ |
| Screen info query | ScreenshotTool | xdpyinfo | ✅ |
| Multi-backend fallback | ScreenshotTool | Auto-detection | ✅ |

**Acceptance criteria:**
- ✅ Bisa capture full screen
- ✅ Bisa capture region (lebih hemat token)
- ✅ Output base64 image
- ✅ Error handling jika scrot/maim tidak ada
- ✅ $DISPLAY check untuk headless environment

### Phase 3: Input Simulation (NEXT — 3-5 hari)

**Goal:** Agent bisa menggerakkan mouse dan keyboard seperti manusia

| Deliverable | Tool | Backend | Prioritas |
|-------------|------|---------|-----------|
| Mouse move & click | InputTool | xdotool (primary) | P0 |
| Keyboard type | InputTool | xdotool (primary) | P0 |
| Keyboard hotkey | InputTool | xdotool (primary) | P0 |
| Clipboard get/set | InputTool | xclip / xsel | P1 |
| Mouse drag & scroll | InputTool | xdotool | P1 |
| Wayland fallback | InputTool | enigo / ydotool | P2 |

**Acceptance criteria:**
- Bisa move mouse ke koordinat (x, y)
- Bisa klik kiri/kanan/tengah
- Bisa type text di aplikasi
- Bisa press hotkey (Ctrl+C, Alt+Tab, dll)
- Baca/tulis clipboard

### Phase 4: Self-Correction Loop (5-7 hari)

**Goal:** Agent bisa verify dan correct tindakannya sendiri

**Deliverables:**
- Text-based verify setelah setiap tool call (xdotool getactivewindow, ps status, xprop)
- Screenshot hanya sebagai fallback jika text verify ambigu
- Compare expected vs actual state
- Auto-retry with different approach (max 3 retries)
- Update memory dengan lessons learned
- Build pattern library dari success/failure

**Acceptance criteria:**
- Agent detect jika action gagal
- Agent retry dengan approach berbeda
- Agent tidak screenshot berlebihan (>1 screenshot per task)
- Memory diupdate dengan failure pattern

### Phase 5: OCR + Image Analysis (5-7 hari)

**Goal:** Agent bisa membaca text dari layar dan menganalisis gambar

| Deliverable | Tool | Backend | Prioritas |
|-------------|------|---------|-----------|
| OCR text extraction | OCRTool | tesseract / leptess | P0 |
| Region-crop before OCR | ScreenshotTool enhancement | Sudah ada | P0 |
| Vision model integration | Model Router | GPT-4V / Claude Vision | P1 |
| UI element detection | Vision analysis | LLM vision (fallback) | P2 |

**Acceptance criteria:**
- OCR >90% accuracy untuk text di layar
- Vision model bisa deskripsikan UI elements
- Region crop sebelum OCR untuk hemat token

### Phase 6: Accessibility Tree (5-7 hari)

**Goal:** Agent bisa membaca struktur UI via AT-SPI2 tanpa screenshot

| Deliverable | Tool | Backend | Prioritas |
|-------------|------|---------|-----------|
| AT-SPI2 connection | AccessibilityTool | atspi crate + zbus | P0 |
| Tree traversal | AccessibilityTool | D-Bus accessibility tree | P0 |
| Element search by role/name | AccessibilityTool | Tree filtering | P1 |
| Element action (click/type) | AccessibilityTool | AT-SPI2 action interface | P1 |
| Wayland strategy | Documentation | grim + slurp + wtype | P2 |

**Acceptance criteria:**
- Bisa fetch accessibility tree dari aplikasi (GTK, Qt, Electron)
- Bisa search element by role (button, textfield, dll)
- Bisa klik element via accessibility
- Graceful degradation jika AT-SPI2 tidak tersedia

### Phase 7: Tool Composition & Autonomous Operation (ongoing)

**Goal:** Agent bisa menjalankan tugas kompleks secara mandiri

**Deliverables:**
- Goal decomposition engine
- Multi-step planning
- Parallel sub-task execution
- End-to-end test: agent buka browser, search, download file
- Pattern library dari ribuan task executions

**Acceptance criteria:**
- Agent pecah goal besar jadi langkah kecil
- Agent recover dari error tanpa human intervention
- Agent selesaikan task multi-step dalam 1 sesi

---

## 5. Spesifikasi Tool

### 5.1 ShellTool ✅ (STABIL)

```
Name: shell
Actions: exec, spawn, poll, kill, list
Backend: sh -c, tokio::process
Fitur: sandboxing (env strip), streaming, dual timeout (total + idle)
```

### 5.2 BrowserTool ✅ (STABIL)

```
Name: browser
Actions: navigate, click, type, get_text, get_html, screenshot, execute_js, scroll, wait, info, pdf
Backend: chromiumoxide + Chrome DevTools Protocol
Fitur: headless, full CDP, page lifecycle management
```

### 5.3 WindowTool ✅ (STABIL)

```
Name: window
Actions: list, focus, close, resize, move, info, search
Backend: xdotool (primary) + xprop (auxiliary)
Fitur:
  - Multi-desktop support (all_desktops parameter)
  - Force close (SIGKILL via windowkill)
  - Optional reposition during resize
  - PID + WM_CLASS via xprop
```

### 5.4 ProcessTool ✅ (STABIL)

```
Name: process
Actions: list, tree, kill, monitor
Backend: ps + kill + /proc
Fitur:
  - Case-insensitive filter by name/command/user
  - Nested process tree (depth configurable)
  - Orphan process handling
  - Detail monitoring via /proc/pid/status
```

### 5.5 ScreenshotTool ✅ (STABIL)

```
Name: screenshot
Actions: capture, info
Backend: scrot > maim > import (ImageMagick) > xwd (X11 raw) > gnome-screenshot
Input:
  - region: optional (x, y, width, height)
  - window_id: optional (hex X11 window ID)
Output:
  - image_base64: base64 encoded image
  - mime_type: "image/png" | "image/x-xwd"
  - width, height: dimensions
  - size_bytes: file size
  - backend: which tool was used
Fitur:
  - Auto-deteksi backend terbaik di constructor
  - DISPLAY env check sebelum capture
  - Graceful fallback chain
  - Temp file cleanup otomatis
```

### 5.6 InputTool 🔜 (BELUM ADA)

```
Name: input
Actions: mouse_move, mouse_click, mouse_scroll, key_press, key_type, key_hotkey, clipboard_get, clipboard_set
Backend (X11): xdotool — click, mousemove, type, key
Backend (Wayland): enigo / ydotool (fallback)
Input:
  - coordinates: (x, y) — for mouse actions
  - button: "left" | "right" | "middle" — for click
  - delta: i32 — for scroll
  - keys: Vec<String> — for key_press/hotkey
  - text: String — for key_type and clipboard_set
```

### 5.7 AccessibilityTool 🔜 (BELUM ADA)

```
Name: accessibility
Actions: tree, search, action, state
Backend: at-spi2 + atspi crate
Input:
  - window_id: target window
  - role: filter by role (button, textfield, etc)
  - name: filter by name
  - element_path: path to specific element
  - perform_action: "click", "type", "focus"
  - text: for type action
Output:
  - tree: accessibility tree
  - elements: Vec<AccessibilityElement>
    - role, name, description, states, children
```

---

## 6. Spesifikasi Agent Baru 🔜 (BELUM ADA)

### 6.1 VisualAgent

Agent khusus untuk interaksi visual.

```
Capabilities:
  - Screenshot analysis (via vision model)
  - OCR text extraction
  - UI element detection
  - Visual navigation

Workflow:
  1. Text-based locate (xdotool search, accessibility tree)
  2. If locate berhasil: skip screenshot
  3. If locate gagal: Screenshot + analyze
  4. Perform action (click/type)
  5. Text-based verify (xdotool getactivewindow)
  6. Screenshot hanya sebagai fallback verification
```

### 6.2 ProcessAgent

Agent khusus untuk manajemen proses.

```
Capabilities:
  - Monitor running processes
  - Start/stop applications
  - Track process output
  - Manage process groups

Workflow:
  1. List running processes
  2. Find target process
  3. Monitor output
  4. Take action (kill, restart, etc)
  5. Verify state
```

### 6.3 DesktopAgent

Agent gabungan untuk operasi desktop lengkap.

```
Capabilities:
  - All VisualAgent capabilities
  - All ProcessAgent capabilities
  - Window management
  - Accessibility navigation
  - Multi-application workflow

Workflow:
  1. Understand goal
  2. Decompose into steps
  3. Execute each step with text-first verification
  4. Screenshot hanya jika perlu
  5. Re-plan if needed
  6. Complete goal
```

---

## 7. Arsitektur Target

### 7.1 High-Level Architecture

```
                    +------------------+
                    |    User Input    |
                    +--------+---------+
                             |
                    +--------v---------+
                    |     Gateway      |
                    | (govern_lifecycle)|
                    +--------+---------+
                             |
              +--------------+--------------+
              |              |              |
     +--------v---+  +------v------+  +----v--------+
     |   Agent    |  |   Agent     |  |   Agent     |
     |  (Shell)   |  |  (Visual)   |  |  (Process)  |
     +--------+---+  +------+------+  +----+--------+
              |              |              |
              +--------------+--------------+
                             |
                    +--------v---------+
                    |   Tool Registry  |
                    +--------+---------+
                             |
     +----------+----------+----------+---------+----------+
     |          |          |          |         |          |
  ShellTool  Browser   WindowTool Process  Screenshot InputTool
     |       Tool        |       Tool      Tool        |
     |          |          |          |         |          |
  +--v--+  +---v----+  +--v---+  +--v----+  +--v---+  +--v---+
  |sh -c|  |chromium|  |xdo-  |  |/proc  |  |scrot/|  |xdo-  |
  |     |  |oxide   |  |tool  |  |ps     |  |maim  |  |tool  |
  +-----+  +--------+  +------+  +-------+  +------+  +------+

  (Future: AccessibilityTool via at-spi2)
```

### 7.2 Agent Turn Loop (Target)

```
User Goal
  -> Gateway
  -> Agent Turn Loop
     1. Recall memory
     2. Text-based state check (xdotool, ps, xprop — murah!)
     3. Decompose goal
     4. For each step:
        a. Plan action
        b. Policy check
        c. Execute tool
        d. Text-based verify (xdotool, ps — murah!)
        e. Screenshot hanya jika text verify gagal
        f. If fail: re-plan and retry
        g. Update memory
     5. Return result
```

### 7.3 Memory Architecture (Target)

```
Memory Sources:
  - Episodic: pengalaman masa lalu (tool calls, results)
  - Semantic: pengetahuan umum (fact database)
  - Procedural: skill & patterns (how-to)
  - Visual: screenshot history & patterns
  - Process: process state & history

Verification:
  - Multi-source: verify across multiple memories
  - Visual verify: screenshot after action
  - Temporal verify: check recency
  - Confidence scoring: rate each memory
```

### 7.4 Tool Structure (Simplified)

Semua tool berada dalam **satu crate** `claw10-tool/src/builtin/` sebagai modul, bukan crate terpisah. Ini menyederhanakan dependency management dan memudahkan refactoring.

```
claw10-tool/
  src/
    builtin/
      shell.rs        # ShellTool ✅
      browser.rs      # BrowserTool ✅
      window.rs       # WindowTool ✅
      process.rs      # ProcessTool ✅
      screenshot.rs   # ScreenshotTool ✅
      artifact.rs     # DeclareArtifactTool ✅
      input.rs        # InputTool 🔜
      accessibility.rs # AccessibilityTool 🔜
    context.rs         # ToolContext
    error.rs           # ToolError
    registry.rs        # ToolRegistry, Tool trait
    result.rs          # ToolOutput
    lib.rs             # Re-exports
```

---

## 8. Tahap Implementasi Detail

### Phase 3: Input Simulation (3-5 hari)

**Implementasi di:** `claw10-tool/src/builtin/input.rs`

**Backend strategy:**
```
X11: xdotool (primary) — click, mousemove, type, key, windowactivate
Wayland: enigo (fallback) — multi-platform
Clipboard: xclip/xsel (X11) or wl-clipboard (Wayland)
```

**Dependencies (minimal):**
```toml
# Tidak ada dependency Rust baru — xdotool via shell commands
# enigo hanya untuk Wayland fallback
```

**Acceptance criteria:**
- Bisa move mouse ke koordinat (x, y)
- Bisa klik kiri/kanan/tengah
- Bisa type text
- Bisa press hotkey (Ctrl+C, Ctrl+V, Alt+Tab)
- Baca/tulis clipboard

### Phase 4: Self-Correction Loop (5-7 hari)

**Modifikasi:** `claw10-agent/src/executor.rs` + `claw10-agent/src/runtime.rs`

**Implementasi:**
1. After setiap tool execution:
   - Text-based verify dulu: xdotool getactivewindow, ps -p, xprop
   - Jika verify sukses: lanjut
   - Jika verify gagal: retry dengan approach berbeda (max 3)
2. Screenshot hanya jika 3x retry gagal
3. Compare expected vs actual state
4. Log perbedaan, update memory
5. Record lesson learned

### Phase 5: OCR (3-5 hari)

**Crate baru:** `claw10-tool/src/builtin/ocr.rs` (atau sub-module screenshot)

**Dependencies:**
```toml
leptess = "0.14"  # Rust binding untuk Tesseract OCR
```

**Implementasi:**
1. Fungsi `ocr_image(path: &str) -> Result<String>`
2. Integrasi dengan ScreenshotTool: region-crop dulu, OCR kemudian
3. Output: extracted text dengan koordinat

**Acceptance criteria:**
- OCR >90% accuracy untuk text ukuran normal
- Bisa spesifik region (tidak perlu OCR seluruh layar)
- Error handling jika tesseract tidak terinstall

### Phase 6: Accessibility (5-7 hari)

**Crate baru:** `claw10-tool/src/builtin/accessibility.rs`

**Dependencies:**
```toml
atspi = "0.19"
zbus = "5"
```

**Implementasi:**
1. Connect ke AT-SPI2 bus D-Bus
2. Get accessible tree dari window terfokus
3. Search element by role/name
4. Perform actions (click, type)
5. Read element states

### Phase 7: Goal Decomposition (ongoing)

**Modifikasi:** `claw10-agent` (planner component)

**Implementasi:**
1. Parse user goal menjadi sub-goals
2. Identify dependencies antar sub-goals
3. Create execution plan
4. Track progress per sub-goal
5. Re-plan on failure

---

## 9. Risiko & Mitigasi

### 9.1 Risiko Teknis

| Risiko | Dampak | Mitigasi |
|--------|--------|----------|
| scrot/maim tidak terinstall | Screenshot gagal | Fallback ke xwd, error message jelas ✅ |
| xdotool tidak ada di X11 | WindowTool gagal | Error message dengan install instructions ✅ |
| xdotool tidak ada | Input simulation gagal | Fallback ke enigo, error jelas |
| enigo tidak kompatibel | Input simulation gagal | Test di multiple DE, fallback ke xdotool |
| AT-SPI2 tidak tersedia | Accessibility gagal | Graceful degradation, fallback ke screenshot |
| Image analysis lambat | Respon lambat | Cache, batch processing, async |
| Tesseract tidak terinstall | OCR gagal | Error jelas, install instructions |
| Memory leak | Security risk | Policy enforcement, sandbox |
| **Wayland compatibility** | X11 tools (xdotool, xprop, xwd) tidak jalan | Deteksi WAYLAND_DISPLAY, fallback ke ydotool/wtype/grim |
| **Container environment** | /proc terbatas, DISPLAY tidak ada | Graceful degradation, skip screen tools |

### 9.2 Risiko UX

| Risiko | Dampak | Mitigasi |
|--------|--------|----------|
| Agent salah klik | Kerusakan data | Confirmation untuk destructive actions |
| Agent infinite loop | Hang | Timeout per action, max retries |
| Agent akses file salah | Privacy risk | Policy deny list, sandbox |
| Agent menjalankan malware | Security | Policy enforcement, skill verification |
| **SSH tanpa X forwarding** | Screen tools silent fail | DISPLAY check, error jelas |

### 9.3 Risiko Performa

| Risiko | Dampak | Mitigasi |
|--------|--------|----------|
| Screenshot besar | Memory usage | Compress, region capture ✅ |
| Banyak proses | CPU usage | Batch processing, limit ✅ |
| Banyak window | Memory usage | Lazy loading, caching |
| OCR setiap frame | CPU tinggi | Cache screenshot, OCR hanya saat perlu |
| Tool chaining lambat | UX jelek | Parallel execution, streaming |

### 9.4 Dependency Risk Matrix

| Dependency | Type | Risk Level | Mitigation |
|------------|------|------------|------------|
| xdotool | System binary | Medium | Error message ✅, enigo fallback planned |
| scrot/maim | System binary | Medium | xwd fallback ✅ |
| Tesseract | System library | Medium | Clear install instructions planned |
| AT-SPI2 / zbus | D-Bus system | High | Graceful degradation planned |
| enigo | Rust crate | Medium | xdotool primary, enigo fallback |
| chromiumoxide | Rust crate | Low | Already integrated ✅ |

---

## 10. Referensi Lama

Dokumen-dokumen berikut sudah dihapus dan isinya diintegrasikan ke dalam dokumen ini:

| Dokumen Lama | Status | Isi yang Diambil |
|-------------|--------|-----------------|
| prd.md | Dihapus | Product requirements -> Section 1-3 |
| features.md | Dihapus | Feature specs -> Section 5-6 |
| agent_swarm.md | Dihapus | Agent architecture -> Section 6 |
| architecture.md | Dihapus | System architecture -> Section 2, 7 |
| technical_specs.md | Dihapus | API specs -> Section 5 |
| shell_first.md | Dihapus | Shell philosophy -> Section 1.2 |
| bug_catalog.md | Dipertahankan | Bug tracking (updated) |

---

*Last updated: 2026-07-19*
*Version: 2.0*
