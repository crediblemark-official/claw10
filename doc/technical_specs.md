# 39. API Requirements

## 39.1 Primary Endpoints

```text
POST   /v1/missions
GET    /v1/missions/{id}
POST   /v1/missions/{id}/pause
POST   /v1/missions/{id}/resume
POST   /v1/missions/{id}/cancel

POST   /v1/tasks
GET    /v1/tasks/{id}
POST   /v1/tasks/{id}/retry
POST   /v1/tasks/{id}/submit-evidence

POST   /v1/agents
GET    /v1/agents/{id}
POST   /v1/agents/{id}/pause
POST   /v1/agents/{id}/hibernate
POST   /v1/agents/{id}/wake
POST   /v1/agents/{id}/terminate
POST   /v1/agents/{id}/migrate

POST   /v1/spawn-requests
GET    /v1/spawn-requests/{id}
POST   /v1/spawn-requests/{id}/approve
POST   /v1/spawn-requests/{id}/deny

GET    /v1/lineages/{id}
GET    /v1/agents/{id}/legacy

POST   /v1/policies/compile
POST   /v1/policies/simulate
POST   /v1/policies/activate

GET    /v1/approvals
POST   /v1/approvals/{id}/approve
POST   /v1/approvals/{id}/deny

GET    /v1/workers
POST   /v1/workers/{id}/drain

GET    /v1/costs
GET    /v1/audit/events
```

---

# 40. Functional Requirements

## 40.1 Organization

| ID     | Requirement                                                         |
| ------ | ------------------------------------------------------------------- |
| FR-001 | Sistem harus mendukung tenant dan organization.                     |
| FR-002 | Organization harus memiliki departments, roles, goals, dan budgets. |
| FR-003 | Human dan agent dapat berada dalam organization yang sama.          |
| FR-004 | Semua reporting line harus dapat dilacak.                           |

## 40.2 Agent Lifecycle

| ID     | Requirement                                                   |
| ------ | ------------------------------------------------------------- |
| FR-010 | Agent harus mendukung ephemeral dan persistent mode.          |
| FR-011 | Persistent identity tidak boleh bergantung pada satu process. |
| FR-012 | Persistent Agent harus mendukung hibernation.                 |
| FR-013 | Persistent Agent harus mendukung checkpoint dan resume.       |
| FR-014 | Agent harus dapat dimigrasikan ke worker lain.                |
| FR-015 | Agent harus memiliki termination rule.                        |
| FR-016 | Terminated Agent harus menghasilkan legacy trace.             |

## 40.3 Recursive Spawn

| ID     | Requirement                                                        |
| ------ | ------------------------------------------------------------------ |
| FR-020 | Agent harus dapat mengusulkan child agent.                         |
| FR-021 | Spawn hanya dapat dilakukan Spawn Broker.                          |
| FR-022 | Child permission tidak boleh melebihi delegable permission parent. |
| FR-023 | Child budget harus berasal dari budget parent atau mission.        |
| FR-024 | Sistem harus membatasi spawn depth.                                |
| FR-025 | Sistem harus membatasi children per agent.                         |
| FR-026 | Sistem harus mendeteksi duplicate objective.                       |
| FR-027 | Sistem harus mendeteksi recursive spawn loop.                      |
| FR-028 | Child Agent harus memiliki parent dan lineage.                     |
| FR-029 | Child Agent dapat ephemeral atau persistent.                       |

## 40.4 Long-Running Agents

| ID     | Requirement                                                                    |
| ------ | ------------------------------------------------------------------------------ |
| FR-030 | Persistent Agent harus dapat berjalan berbulan-bulan.                          |
| FR-031 | Persistent Agent harus mendukung recurring budget.                             |
| FR-032 | Persistent Agent harus mendukung schedule dan subscriptions.                   |
| FR-033 | Persistent Agent harus melakukan policy renewal.                               |
| FR-034 | Persistent Agent harus melakukan credential rotation.                          |
| FR-035 | Persistent Agent harus mendukung session rotation.                             |
| FR-036 | Persistent Agent harus memiliki periodic review.                               |
| FR-037 | Persistent Agent tanpa tanggung jawab aktif harus dihibernasi atau dihentikan. |

## 40.5 Missions and Tasks

| ID     | Requirement                                                |
| ------ | ---------------------------------------------------------- |
| FR-040 | Mission harus memiliki objective, scope, budget, dan risk. |
| FR-041 | Task harus memiliki output contract.                       |
| FR-042 | Task harus memiliki acceptance criteria.                   |
| FR-043 | Task claim harus atomik.                                   |
| FR-044 | Side effect harus memiliki idempotency key.                |
| FR-045 | Task tidak boleh selesai tanpa required evidence.          |

## 40.6 Governance

| ID     | Requirement                                            |
| ------ | ------------------------------------------------------ |
| FR-050 | Semua tool calls harus melewati policy engine.         |
| FR-051 | Policy harus dikompilasi ke Internal Policy IR.        |
| FR-052 | Explicit deny harus mengalahkan allow.                 |
| FR-053 | Persistent Agent creation dapat diwajibkan approval.   |
| FR-054 | Permission increase harus membutuhkan approval.        |
| FR-055 | Operator harus dapat menghentikan seluruh descendants. |

## 40.7 Memory and Skills

| ID     | Requirement                                              |
| ------ | -------------------------------------------------------- |
| FR-060 | Memory harus memiliki source, scope, dan confidence.     |
| FR-061 | Memory baru harus melewati admission pipeline.           |
| FR-062 | Child memory tidak langsung menjadi organization memory. |
| FR-063 | Agen dapat membuat candidate skill.                      |
| FR-064 | Skill harus diuji dan ditandatangani.                    |
| FR-065 | Skill tidak boleh menambah privilege.                    |

## 40.8 Execution

| ID     | Requirement                                                            |
| ------ | ---------------------------------------------------------------------- |
| FR-070 | Sistem harus mendukung local, sandbox, remote, cloud, dan edge worker. |
| FR-071 | Worker harus memiliki identity dan heartbeat.                          |
| FR-072 | Network harus deny by default.                                         |
| FR-073 | Runtime harus menggunakan scoped credential.                           |
| FR-074 | Operator harus dapat menghentikan tool execution.                      |
| FR-075 | Runtime cleanup harus idempotent.                                      |

## 40.9 Operator Interface

| ID     | Requirement                                                |
| ------ | ---------------------------------------------------------- |
| FR-080 | Ratatui harus menampilkan agent, task, swarm, dan lineage. |
| FR-081 | Ratatui harus menampilkan persistent agent state.          |
| FR-082 | Ratatui harus menampilkan spawn requests.                  |
| FR-083 | Ratatui harus mendukung approval dan termination.          |
| FR-084 | Ratatui tidak boleh menampilkan secret.                    |
| FR-085 | Ratatui harus berfungsi melalui SSH.                       |

## 40.10 Observability

| ID     | Requirement                                                   |
| ------ | ------------------------------------------------------------- |
| FR-090 | Semua service harus menghasilkan structured events.           |
| FR-091 | Vector harus melakukan redaction.                             |
| FR-092 | Events harus memiliki lineage ID dan trace ID.                |
| FR-093 | Telemetry failure tidak boleh mengubah task state.            |
| FR-094 | Operator harus dapat mencari event berdasarkan agent lineage. |

---

# 41. Non-Functional Requirements

## 41.1 Security

| ID      | Requirement                                            |
| ------- | ------------------------------------------------------ |
| NFR-001 | Semua service communication harus terenkripsi.         |
| NFR-002 | Remote worker harus menggunakan mutual authentication. |
| NFR-003 | Secrets tidak boleh masuk prompt, log, atau memory.    |
| NFR-004 | Policy failure harus fail closed.                      |
| NFR-005 | Sandbox harus memakai non-root execution.              |
| NFR-006 | Tenant isolation harus diuji.                          |

## 41.2 Reliability

| ID      | Requirement                                                 |
| ------- | ----------------------------------------------------------- |
| NFR-010 | Logical Agent state harus bertahan setelah runtime restart. |
| NFR-011 | Persistent Agent harus dapat resume dari checkpoint.        |
| NFR-012 | Event consumer harus idempotent.                            |
| NFR-013 | Spawn dan termination harus idempotent.                     |
| NFR-014 | Worker failure tidak boleh menghapus lineage.               |
| NFR-015 | Backup restore harus diuji.                                 |

## 41.3 Performance

| ID      | Requirement                                    |
| ------- | ---------------------------------------------- |
| NFR-020 | Cached policy evaluation p95 di bawah 100 ms.  |
| NFR-021 | Spawn decision p95 di bawah 500 ms.            |
| NFR-022 | Local child provisioning p95 di bawah 3 detik. |
| NFR-023 | TUI live update p95 di bawah 3 detik.          |
| NFR-024 | Kill propagation p95 di bawah 5 detik.         |

## 41.4 Scalability

| ID      | Requirement                                         |
| ------- | --------------------------------------------------- |
| NFR-030 | MVP mendukung 20 active agents.                     |
| NFR-031 | V1 mendukung 200 registered agents.                 |
| NFR-032 | V1 mendukung 50 persistent agents per organization. |
| NFR-033 | Distributed mode mendukung 1.000 concurrent tasks.  |
| NFR-034 | Worker dapat ditambah tanpa restart control plane.  |

## 41.5 Maintainability

| ID      | Requirement                                           |
| ------- | ----------------------------------------------------- |
| NFR-040 | Core domain tidak boleh bergantung pada provider SDK. |
| NFR-041 | Semua integrations harus melalui adapter.             |
| NFR-042 | State machines harus memiliki tests.                  |
| NFR-043 | Policy compiler harus memiliki golden tests.          |
| NFR-044 | Breaking API changes membutuhkan major version.       |

---

# 42. Threat Model

| Ancaman                | Mitigasi                                          |
| ---------------------- | ------------------------------------------------- |
| Runaway spawning       | Depth, size, budget, TTL, duplicate detection     |
| Privilege escalation   | Delegable permission intersection                 |
| Persistent agent drift | Periodic review, policy renewal, checkpoint audit |
| Memory poisoning       | Admission pipeline and provenance                 |
| Malicious skill        | Scan, sandbox, signing                            |
| Prompt injection       | Content isolation and tool policy                 |
| Infinite conversations | Turn, time, and cost limits                       |
| Orphan agents          | Orphan Reaper                                     |
| Credential leakage     | Short-lived scoped leases                         |
| False completion       | Evidence and verification                         |
| Cross-tenant leakage   | Tenant-scoped identity and storage                |
| Duplicate side effects | Idempotency keys                                  |
| Policy tampering       | Signed Policy IR                                  |
| Compromised worker     | Quarantine and credential revocation              |
| Approval fatigue       | Risk-based grouping and clear previews            |

---

# 43. Orphan Reaper

Orphan Reaper mendeteksi agent yang:

* tidak memiliki active parent;
* tidak memiliki active mission;
* kehilangan runtime lease;
* tidak mengirim heartbeat;
* tetap aktif setelah task selesai.

Prosedur:

1. freeze;
2. revoke;
3. request final checkpoint;
4. preserve trace;
5. stop runtime;
6. wipe workspace;
7. create forced termination record.

---

# 44. Cost Management

Budget hierarchy:

```text
Tenant
→ Organization
→ Department
→ Mission
→ Agent
→ Team
→ Task
→ Tool Invocation
```

Controls:

* cost reservation;
* soft limit;
* hard limit;
* recurring budget;
* model downgrade proposal;
* spawn tax;
* idle hibernation;
* circuit breaker.

## 44.1 Spawn Tax

Setiap child menambah coordination cost.

```text
expected_value_of_child >
    provisioning_cost
    + model_cost
    + coordination_cost
    + verification_cost
```

---

# 45. Product Metrics

## 45.1 Quality

* mission completion rate;
* verification pass rate;
* revision count;
* false completion rate;
* memory rejection rate;
* skill failure rate.

## 45.2 Swarm Efficiency

* agents spawned per accepted task;
* unused child rate;
* duplicate child rate;
* coordination cost;
* average spawn depth;
* child contribution rate.

## 45.3 Long-Running Reliability

* persistent agent uptime;
* successful resume rate;
* checkpoint recovery rate;
* stale persistent agent count;
* policy renewal success;
* credential rotation success.

## 45.4 Safety

* blocked unsafe actions;
* privilege escalation attempts;
* orphan agents;
* secret leakage count;
* policy bypass count;
* forced termination count.

## 45.5 Cost

* cost per mission;
* cost per accepted task;
* cost per persistent agent per month;
* idle compute cost;
* context compression saving.

---

# 46. MVP Scope

## Included

* Rust modular monolith;
* single tenant;
* one organization;
* 20 active agents;
* ephemeral and persistent modes;
* two spawn levels;
* Root, Planner, Orchestrator, Specialist, Critic, Verifier;
* local and sandbox workers;
* deterministic scheduler;
* task and mission state machines;
* Spawn Broker;
* lineage;
* legacy trace;
* checkpoint and hibernation;
* PostgreSQL;
* NATS JetStream;
* basic memory;
* candidate skills;
* TOON and JSON context;
* ICVS adapter and internal Policy IR;
* Ratatui control center;
* Vector logs and metrics;
* approval;
* budget;
* kill switch.

## Excluded

* unrestricted hardware control;
* multi-region control plane;
* public marketplace;
* autonomous financial transactions;
* automatic skill activation;
* advanced multi-tenant production;
* unlimited persistent teams.

---

# 47. Roadmap

## Phase 0: Foundation

* domain model;
* threat model;
* architecture decisions;
* repository;
* CI;
* coding standards.

## Phase 1: Control Kernel

* identity;
* organization;
* mission;
* task;
* PostgreSQL;
* audit;
* CLI.

## Phase 2: Agent Runtime

* model adapter;
* agent genome;
* logical agent;
* runtime instance;
* local worker;
* tool interface.

## Phase 3: Recursive Swarm

* Spawn Broker;
* child identity;
* lineage;
* context inheritance;
* depth limits;
* descendant termination.

## Phase 4: Dual Lifecycle

* ephemeral lifecycle;
* persistent lifecycle;
* hibernation;
* heartbeat;
* checkpoint;
* resume;
* migration.

## Phase 5: Governance

* ICVS adapter;
* Policy IR;
* risk engine;
* approval;
* budget;
* kill switch.

## Phase 6: Memory and Skills

* episodic memory;
* semantic memory;
* memory curator;
* skill candidate;
* sandbox validation;
* signed registry.

## Phase 7: Ratatui

* overview;
* agents;
* persistent agents;
* tasks;
* swarms;
* lineage;
* approvals;
* policies;
* costs.

## Phase 8: Observability

* Vector pipeline;
* redaction;
* logs;
* metrics;
* OpenTelemetry tracing;
* alerts.

## Phase 9: Distributed Workers

* remote worker;
* cloud worker;
* edge worker;
* mTLS;
* draining;
* migration.

## Phase 10: Production Hardening

* multi-tenant isolation;
* backup;
* disaster recovery;
* load testing;
* chaos testing;
* security testing;
* signed releases.

---

# 48. Repository Structure

```text
claw10/
├── Cargo.toml
├── crates/
│   ├── claw10-domain/
│   ├── claw10-control-api/
│   ├── claw10-auth/
│   ├── claw10-organization/
│   ├── claw10-mission/
│   ├── claw10-task/
│   ├── claw10-scheduler/
│   ├── claw10-agent/
│   ├── claw10-lifecycle/
│   ├── claw10-spawn/
│   ├── claw10-lineage/
│   ├── claw10-policy/
│   ├── claw10-icvs/
│   ├── claw10-context/
│   ├── claw10-toon/
│   ├── claw10-model-router/
│   ├── claw10-memory/
│   ├── claw10-skill/
│   ├── claw10-tool/
│   ├── claw10-worker/
│   ├── claw10-artifact/
│   ├── claw10-budget/
│   ├── claw10-audit/
│   ├── claw10-telemetry/
│   ├── claw10-gateway/
│   ├── claw10-cli/
│   └── claw10-tui/
├── policies/
├── genomes/
├── migrations/
├── proto/
├── vector/
├── deploy/
├── examples/
├── docs/
├── sdk/
└── tests/
    ├── unit/
    ├── property/
    ├── integration/
    ├── policy/
    ├── security/
    ├── lifecycle/
    ├── replay/
    └── chaos/
```

---

# 49. Technology Stack

| Area             | Teknologi                                  |
| ---------------- | ------------------------------------------ |
| Core             | Rust                                       |
| Async runtime    | Tokio                                      |
| API              | Axum                                       |
| TUI              | Ratatui                                    |
| Terminal input   | Crossterm                                  |
| CLI              | Clap                                       |
| Database         | PostgreSQL                                 |
| Query            | SQLx                                       |
| Event bus        | NATS JetStream                             |
| Memory vectors   | pgvector, lalu Qdrant                      |
| Artifact storage | Local, lalu S3-compatible                  |
| Secrets          | Encrypted local vault, lalu external Vault |
| Policy source    | ICVS                                       |
| Policy execution | Internal Rust Policy IR                    |
| LLM context      | TOON dan JSON                              |
| Telemetry        | Vector                                     |
| Tracing          | OpenTelemetry                              |
| Metrics          | Prometheus-compatible                      |
| Logs             | Loki or Elasticsearch-compatible           |
| Containers       | Docker or containerd                       |
| Serialization    | Serde, JSON, Protobuf                      |

---

# 50. Acceptance Criteria

## AC-01 Recursive Spawn

Given Parent Agent memiliki spawn permission, when agent mengirim valid Spawn Request, then system membuat child dengan bounded permission, budget, TTL, dan lineage.

## AC-02 Privilege Boundary

Given Parent hanya memiliki repository read, when Parent meminta child dengan production write, then Spawn Broker menolak.

## AC-03 Persistent Agent

Given agent memakai persistent mode, when runtime process dihentikan, then agent dapat resume dari checkpoint pada worker lain.

## AC-04 Hibernation

Given Persistent Agent idle, when idle threshold tercapai, then runtime dilepas tetapi identity, subscriptions, dan state tetap tersedia.

## AC-05 Event Wake

Given agent sedang hibernasi, when subscribed event muncul, then scheduler membuat runtime dan agent melanjutkan dari checkpoint.

## AC-06 Ephemeral Teardown

Given child task diterima verifier, when teardown berjalan, then credentials dicabut, workspace dihapus, runtime dihentikan, dan legacy trace tersimpan.

## AC-07 Descendant Kill

Given parent memiliki descendants, when operator menghentikan parent, then descendants dibekukan dan diproses sesuai termination policy.

## AC-08 Agent Lineage

Given agent telah dihentikan, when operator membuka lineage, then parent, children, tasks, costs, tools, artifacts, dan termination reason dapat dilihat.

## AC-09 Policy

Given tool call tidak diizinkan, when agent mengajukan invocation, then action ditolak sebelum worker menjalankannya.

## AC-10 Budget

Given mission mencapai hard limit, when agent meminta model call atau spawn baru, then request ditolak atau mission dijeda.

## AC-11 Memory

Given child mengusulkan memory, when child dihentikan, then memory tidak aktif sebelum admission pipeline selesai.

## AC-12 Skill

Given candidate skill belum ditandatangani, when worker diminta menjalankan skill, then worker menolak.

## AC-13 TOON Fallback

Given model gagal memahami TOON, when verifier mendeteksi failure, then system memakai JSON fallback.

## AC-14 Orphan Cleanup

Given agent kehilangan parent dan mission, when orphan timeout tercapai, then system membuat forced legacy record dan menghentikan runtime.

## AC-15 Long-Term Operation

Given Persistent Agent telah aktif selama beberapa bulan, when system melakukan upgrade, credential rotation, dan worker migration, then identity, task ownership, lineage, dan memory tetap konsisten.

---

# 51. Launch Gates

Claw10 belum production-ready sampai:

1. policy bypass test lulus;
2. tenant isolation test lulus;
3. recursive spawn limit test lulus;
4. checkpoint recovery test lulus;
5. persistent migration test lulus;
6. descendant kill test lulus;
7. orphan cleanup test lulus;
8. secret redaction test lulus;
9. legacy trace integrity test lulus;
10. skill signature enforcement lulus;
11. budget hard-stop test lulus;
12. backup restore test lulus;
13. audit integrity test lulus;
14. TOON fallback test lulus;
15. Vector outage tidak mengubah task state.

---

# 52. Definition of Done

Sebuah product feature selesai jika:

* requirement diimplementasikan;
* unit tests lulus;
* integration tests lulus;
* security review selesai;
* telemetry tersedia;
* audit event tersedia;
* documentation tersedia;
* migration tersedia;
* rollback tersedia;
* acceptance criteria lulus.

Sebuah agent task selesai jika:

* output contract terpenuhi;
* evidence tersedia;
* verifier menerima;
* policy dipatuhi;
* biaya tercatat;
* artifact memiliki hash;
* audit tersimpan;
* memory candidate telah diputuskan;
* child agents telah ditutup atau dialihkan.

---

# 53. Keputusan Arsitektur Final

```text
OpenClaw-inspired capability
= omnichannel gateway, tools, sessions, multi-agent routing

ZeroClaw-inspired capability
= lightweight Rust-native execution runtime

Hermes-inspired capability
= memory, skills, delegation, scheduled work, learning

Paperclip-inspired capability
= organization, goals, budgets, governance, heartbeats

Claw10 innovation
= recursive self-forming teams
+ bounded agent spawning
+ persistent and ephemeral lifecycle
+ logical agent identity
+ secure teardown
+ complete lineage
+ evidence-based completion

TOON
= LLM context representation

ICVS
= instruction and policy authoring source

Ratatui
= human control center

Vector
= observability pipeline
```

---

# 54. Kesimpulan

Claw10 OS harus memungkinkan agen membentuk organisasi kerja yang sesuai dengan kebutuhan aktual.

Agen dapat membelah tugas menjadi tim. Agen dapat menciptakan child agents. Child agents dapat membentuk swarm lanjutan selama policy mengizinkan.

Tidak semua agent harus dihentikan setelah satu tugas.

Ephemeral Agent berakhir setelah objective selesai.

Persistent Agent dapat berjalan selama berhari-hari, berbulan-bulan, atau tanpa tanggal akhir. Agent tersebut dapat tidur, bangun, berpindah runtime, mengganti model, memperbarui credential, dan melanjutkan pekerjaan dari checkpoint.

Setiap agent tetap memiliki:

* identity;
* parent;
* lineage;
* budget;
* policy;
* memory scope;
* lifecycle;
* termination rule.

Saat agent berakhir, runtime menghilang. Jejak pekerjaan tetap hidup melalui:

* audit;
* lineage;
* artifact;
* evidence;
* verified memory;
* approved skills;
* cost records;
* legacy trace.

Dengan rancangan ini, Claw10 menjadi sistem operasi untuk tenaga kerja digital yang dapat membentuk, mempertahankan, mengubah, dan membubarkan organisasinya sendiri tanpa kehilangan kendali manusia.

---

# Appendix A: Architecture Decisions (2026-06-27)

## A.1 Storage Engine: sled

Memutuskan menggunakan sled sebagai database embedded utama.

Alasan:
1. Zero-schema — drop-in replacement untuk HashMap, tanpa migration
2. Serialisasi via bincode — serde binary, minimal overhead
3. Single file persistent — tidak perlu server terpisah
4. Performa tinggi — concurrent B-tree, lock-free
5. Paling hemat token dibanding SQLite/PostgreSQL

Implementasi:
- `claw10-store` crate: Store trait + SledStore + InMemoryStore (testing)
- Setiap service yang butuh persistence menerima `Arc<dyn Store>`
- InMemoryStore untuk fallback/testing tanpa sled dependency

## A.2 Instruction Format: ICVS (InstructCanvas)

Memutuskan menggunakan ICVS sebagai satu-satunya format authoring instructions.

Alasan:
- DAG-based → presisi, bisa conditional, severity, blocklist/allowlist
- Modular via `[include:]`
- Cycle detection built-in
- Export ke berbagai format (claude, openai, json)
- LSP support

Coverage:
- Prompt agent templates
- Policy rules (compiled ke PolicyRule domain struct)
- Agent instructions (DAG resolusi per task/role)

Tidak menggunakan Markdown untuk instructions. ICVS menggantikan Markdown sepenuhnya di ranah instruction authoring.

Implementasi:
- `claw10-icvs` crate: adapter yang wrap icvs crate
- ICVS source → compile → domain types (PolicyRule, AgentPrompt, etc.)
- Tidak ada hardcoded prompt strings

## A.3 Context Encoding: TOON

Memutuskan menggunakan TOON sebagai format encoding context untuk LLM.

Alasan:
- Format terstruktur yang dioptimalkan untuk LLM consumption
- Menggantikan JSON injection yang verbose
- Pipeline: context selection → classification → redaction → priority → token budget → TOON/JSON

Coverage:
- Task context
- Memory digest
- Agent roster & lineage
- Policy summary
- Evidence summary
- Cost summary

Implementasi:
- `claw10-toon` crate: encoder dari domain struct ke TOON format
- Fallback ke JSON jika model gagal parse
- Bagian dari context pipeline di `claw10-context`

## A.4 Prompt System

Memutuskan TIDAK menggunakan file Markdown untuk prompt.

ICVS menangani semua instruction authoring — baik prompt template maupun policy rules dalam satu format terstruktur.

Prompt template adalah ICVS node dengan `type = prompt`, yang bisa:
- Conditional (if = $ROLE == "specialist" then ...)
- Severity (must/should/may untuk enforcement level)
- DAG dependency antar prompt nodes
- Include modular prompt library

