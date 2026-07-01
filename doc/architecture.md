# 35. Ratatui Control Center

Ratatui menjadi operator interface utama.

## 35.1 Screens

* Overview;
* Organizations;
* Missions;
* Tasks;
* Agents;
* Swarms;
* Agent Lineage;
* Persistent Agents;
* Spawn Requests;
* Approvals;
* Workers;
* Policies;
* Memory;
* Skills;
* Artifacts;
* Costs;
* Logs;
* Incidents;
* Terminated Agent Archive.

## 35.2 Persistent Agent Screen

Menampilkan:

* lifecycle mode;
* uptime;
* current runtime;
* hibernation state;
* subscriptions;
* schedules;
* checkpoint;
* monthly cost;
* child teams;
* next review;
* policy version.

## 35.3 Example

```text
┌ Claw10 Control Center ───────────────────────────────────────────────┐
│ Organization: Teacher Portal                  Environment: Production │
├───────────────────┬───────────────────────────────────────────────────┤
│ PERSISTENT AGENTS │ ACTIVE SWARM                                      │
│                   │                                                   │
│ ● Director        │ Mission: Security Audit                           │
│ ● Ops Manager     │ Root: security-manager-01                         │
│ ○ Report Agent    │ Children: 6 active, 2 completed                   │
│ ● Monitor Agent   │ Depth: 2 / 3                                      │
│                   │ Budget: $7.20 / $15.00                            │
├───────────────────┼───────────────────────────────────────────────────┤
│ SPAWN REQUESTS    │ EXECUTION                                         │
│ SR-014 APPROVAL   │ Agent: database-specialist-03                     │
│ SR-015 VALIDATING │ Worker: sandbox-worker-07                         │
│ SR-016 DENIED     │ Tool: static_analysis.run                         │
├───────────────────┴───────────────────────────────────────────────────┤
│ [a] approve  [d] deny  [p] pause  [k] kill  [g] lineage  [l] logs     │
└───────────────────────────────────────────────────────────────────────┘
```

## 35.4 TUI Architecture

Gunakan pola:

```text
Event
→ Message
→ State Update
→ Command
→ Render
```

TUI tidak berkomunikasi langsung dengan worker. Semua command melewati Control API.

---

# 36. Vector Observability

Vector digunakan untuk:

* logs;
* structured agent events;
* metrics;
* security events;
* audit copies;
* worker telemetry.

OpenTelemetry digunakan untuk distributed tracing.

## 36.1 Topology

```mermaid
flowchart LR
    CONTROL[Control Plane]
    AGENT[Agent Runtime]
    WORKER[Workers]
    GATEWAY[Gateway]
    TUI[Ratatui]

    CONTROL --> VA[Vector Agents]
    AGENT --> VA
    WORKER --> VA
    GATEWAY --> VA
    TUI --> VA

    VA --> AGG[Vector Aggregator]
    AGG --> REDACT[Redaction]
    REDACT --> ENRICH[Enrichment]
    ENRICH --> ROUTE[Routing]

    ROUTE --> LOGS[Logs]
    ROUTE --> METRICS[Metrics]
    ROUTE --> SECURITY[Security]
    ROUTE --> ARCHIVE[Audit Archive]
```

## 36.2 Required Fields

```json
{
  "timestamp": "2026-06-27T15:02:18Z",
  "tenant_id": "tenant-a",
  "mission_id": "mission-204",
  "task_id": "task-14",
  "agent_id": "agent-7F21",
  "parent_agent_id": "engineering-lead-01",
  "lineage_id": "lineage-204",
  "worker_id": "worker-07",
  "trace_id": "trace-001",
  "event_type": "agent.terminated",
  "lifecycle_mode": "ephemeral",
  "risk_level": "medium",
  "status": "success",
  "cost_usd": 0.42
}
```

Vector harus menghapus:

* passwords;
* tokens;
* cookies;
* API keys;
* authorization headers;
* private keys;
* sensitive tool arguments.

---

# 37. Data Architecture

## 37.1 PostgreSQL

Menyimpan:

* tenant;
* organization;
* identity;
* agent;
* lineage;
* mission;
* task;
* lease;
* spawn request;
* approval;
* policy;
* budget;
* memory metadata;
* skill metadata;
* artifact metadata;
* legacy metadata.

## 37.2 NATS JetStream

Digunakan untuk:

* task dispatch;
* commands;
* state events;
* heartbeat;
* wake events;
* child completion;
* approval result.

## 37.3 Vector Database

Digunakan untuk semantic memory retrieval.

## 37.4 Object Storage

Digunakan untuk:

* artifact;
* screenshot;
* reports;
* dataset;
* test output;
* legacy bundles.

## 37.5 Secret Vault

Menyimpan credential dan secret leases.

## 37.6 Append-Only Audit

Menyimpan critical audit events.

---

# 38. Core Domain Entities

```text
Tenant
Organization
Department
HumanIdentity
ServiceIdentity
AgentIdentity
AgentGenome
AgentRuntime
AgentCheckpoint
AgentSubscription
AgentSchedule
AgentLineage
AgentLegacy
SpawnRequest
Mission
Task
TaskDependency
TaskLease
Approval
PolicyBundle
PolicyRule
Tool
ToolInvocation
Worker
WorkerCapability
ModelProvider
ModelProfile
Memory
Skill
SkillVersion
Artifact
Evidence
Budget
CostRecord
Reputation
Incident
AuditEvent
Channel
Session
```

---

