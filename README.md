# ClawHive OS

**Recursive, Persistent, and Ephemeral Agent Swarm Operating System**

ClawHive OS adalah sistem operasi untuk kawanan agen AI yang dapat merekrut agen baru secara rekursif, memiliki siklus hidup persisten maupun ephemeral, serta dikontrol melalui API HTTP, TUI (Terminal User Interface), dan CLI.

**Status:** Core runtime, TUI, API, model router, dan control-plane sudah berfungsi. Proyek ini dioptimalkan untuk **single-user, local-first** deployment.

---

## Fitur yang Sudah Berfungsi

| Area | Fitur | Status |
|---|---|---|
| **CLI** | `serve`, `tui`, `run-agent`, `version`, `setup` | ✅ |
| **Installer** | One-line bash/PowerShell installer | ✅ |
| **TUI** | Workspace selector, chat streaming, model selection, command palette, tool approval, 14 management screens | ✅ |
| **HTTP API** | Health, agents, missions, tasks, spawn, lineage, policy, approvals, workers, lifecycle, scheduler, memory, gateway, skills, artifacts, audit | ✅ |
| **Agent Runtime** | Streaming events, tool execution, session management, context window limit, context pipeline | ✅ |
| **Model Router** | Multi-provider OpenAI-compatible (OpenAI, Anthropic, OpenRouter, NVIDIA, Groq, Together, Ollama, dll.), auto-discovery, fallback | ✅ |
| **Tool System** | Shell, ReadFile, WriteFile, Http | ✅ |
| **Store** | InMemory + Sled abstraction, namespaced store | ✅ |
| **Spawn Broker** | Create/approve/deny spawn, depth/budget/swarm validation, child agent creation | ✅ |
| **Event Bus** | InMemory + NATS (feature flag `nats`) | ✅ |
| **Lifecycle Service** | Hibernate, wake, terminate, migrate, lease, heartbeat, stale detection | ✅ |
| **Worker Service** | Register, heartbeat, drain, offline, quarantine, stale detection | ✅ |
| **Scheduler** | Add/list/remove schedules, due schedules | ✅ |
| **Memory Service** | Store/get/update/delete/verify/transition/query + admission pipeline | ✅ |
| **Policy Service** | Evaluate policy bundles, ICVS compiler | ✅ |
| **Skill Service** | Create/list/get/transition/sign skill lifecycle | ✅ |
| **Artifact Service** | Store/get/list/delete artifacts with SHA-256 content hash | ✅ |
| **Audit Service** | Record and query audit events | ✅ |
| **Gateway** | Webhook, Telegram Bot API, Discord webhook, WhatsApp bridge, Slack, InternalBus | ✅ |
| **Telemetry** | Structured JSON log for Vector observability pipeline | ✅ |

---

## Instalasi

### One-line installer

> `clawhive.dev` belum aktif. Gunakan URL GitHub raw di bawah, atau host script `install.sh` / `install.ps1` di domainmu sendiri.

**Linux / macOS / WSL / VPS:**

```bash
curl -fsSL https://raw.githubusercontent.com/clawhive/clawhive/main/install.sh | sh
```

**Windows PowerShell:**

```powershell
irm https://raw.githubusercontent.com/clawhive/clawhive/main/install.ps1 | iex
```

**Build dari source:**

```bash
git clone https://github.com/clawhive/clawhive.git
cd clawhive
cargo install --path crates/clawhive-cli
```

---

## Quickstart

### 1. Setup awal

```bash
clawhive setup
```

Wizard akan membuat file konfigurasi di `~/.config/clawhive/config.toml` (atau `./clawhive.toml`) dan meminta API key provider LLM pilihanmu.

### 2. Jalankan server + TUI

```bash
clawhive serve --tui
```

- API server berjalan di `http://0.0.0.0:3000`
- TUI terbuka di terminal yang sama

### 3. Jalankan agent headless

```bash
clawhive run-agent --objective "buat file hello.txt"
```

### 4. Dengan persistent store

```bash
clawhive serve --db /tmp/clawhive.sled --tui
```

### 5. Dengan NATS event bus

```bash
NATS_URL=nats://localhost:4222 clawhive serve --features nats -- serve
```

---

## Konfigurasi Provider LLM

File konfigurasi menggunakan format TOML. Contoh `~/.config/clawhive/config.toml`:

```toml
[alias.default]
slot = "openai"
model = "gpt-4o-mini"
api_key = "$OPENAI_API_KEY"

[alias.cheap]
slot = "openai"
model = "gpt-4o-mini"
api_key = "$OPENAI_API_KEY"

[alias.haiku]
slot = "anthropic"
model = "claude-3-5-haiku"
api_key = "$ANTHROPIC_API_KEY"
```

Set API key via environment variable:

```bash
export OPENAI_API_KEY="sk-..."
```

---

## Arsitektur Workspace

Workspace terdiri dari 29 crate:

```
crates/
├── clawhive-cli              # Entry point binary
├── clawhive-control-api      # HTTP API (axum)
├── clawhive-tui              # Terminal UI (ratatui)
├── clawhive-agent            # Agent runtime & executor
├── clawhive-model-router     # Multi-provider LLM routing
├── clawhive-tool             # Tool system
├── clawhive-store            # Store abstraction (InMemory + Sled)
├── clawhive-domain           # Domain types
├── clawhive-spawn            # Spawn broker & validator
├── clawhive-lifecycle        # Agent lifecycle
├── clawhive-worker           # Worker registry
├── clawhive-scheduler        # Schedule service
├── clawhive-memory           # Memory service + admission pipeline
├── clawhive-policy           # Policy engine
├── clawhive-gateway          # Omnichannel gateway
├── clawhive-event            # Event bus (InMemory + NATS)
├── clawhive-prompt           # Prompt assembly & ICVS
├── clawhive-toon             # Context serialization format
├── clawhive-icvs             # ICVS policy/prompt compiler
├── clawhive-auth             # Agent identity & credential (internal, used by spawn)
├── clawhive-mission          # Mission service
├── clawhive-task             # Task service
├── clawhive-lineage          # Lineage tracking
├── clawhive-skill            # Skill lifecycle
├── clawhive-artifact         # Artifact storage
├── clawhive-audit            # Audit service
├── clawhive-context          # Context pipeline
├── clawhive-budget           # Budget service
└── clawhive-telemetry        # Telemetry events
```

---

## Catatan Hardware / Embedded

ClawHive OS saat ini **tidak memiliki hardware subsystem**. Dukungan untuk board seperti Arduino Uno Q, STM32 Nucleo, Raspberry Pi, Android, Aardvark, dan abstraksi `Peripheral` trait belum diimplementasikan. Menambahkan HAL (Hardware Abstraction Layer) merupakan pekerjaan besar tersendiri yang memerlukan crate baru dan adapter per board.

---

## Build & Test

```bash
# Check
cargo check

# Check dengan NATS
cargo check --features nats

# Test
cargo test -- --test-threads=1

# Build release (LTO + strip)
cargo build --release
```

---

## Lisensi

MIT
