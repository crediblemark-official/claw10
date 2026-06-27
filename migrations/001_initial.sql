-- ClawHive OS - Initial Schema
-- PostgreSQL migration

CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Tenants
CREATE TABLE tenants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    settings JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Organizations
CREATE TABLE organizations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    mission_statement TEXT,
    budget JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Departments
CREATE TABLE departments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    name TEXT NOT NULL,
    parent_department_id UUID REFERENCES departments(id),
    budget JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Identities
CREATE TABLE identities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    organization_id UUID NOT NULL REFERENCES organizations(id),
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Roles
CREATE TABLE roles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    organization_id UUID NOT NULL REFERENCES organizations(id),
    parent_role_id UUID REFERENCES roles(id)
);

-- Role permissions
CREATE TABLE role_permissions (
    role_id UUID NOT NULL REFERENCES roles(id),
    permission TEXT NOT NULL,
    PRIMARY KEY (role_id, permission)
);

-- Identity roles
CREATE TABLE identity_roles (
    identity_id UUID NOT NULL REFERENCES identities(id),
    role_id UUID NOT NULL REFERENCES roles(id),
    PRIMARY KEY (identity_id, role_id)
);

-- Credentials
CREATE TABLE credentials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    identity_id UUID NOT NULL REFERENCES identities(id),
    kind TEXT NOT NULL,
    scope TEXT NOT NULL,
    issued_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ
);

-- Missions
CREATE TABLE missions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    owner_id UUID NOT NULL REFERENCES identities(id),
    objective TEXT NOT NULL,
    scope TEXT,
    lifecycle_mode TEXT NOT NULL DEFAULT 'ephemeral',
    campaign_end TIMESTAMPTZ,
    review_interval_days INT,
    budget JSONB NOT NULL DEFAULT '{}',
    risk TEXT NOT NULL DEFAULT 'medium',
    require_evidence BOOLEAN NOT NULL DEFAULT true,
    minimum_verifiers INT NOT NULL DEFAULT 1,
    state TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Tasks
CREATE TABLE tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mission_id UUID NOT NULL REFERENCES missions(id),
    parent_task_id UUID REFERENCES tasks(id),
    owner_id UUID,
    objective TEXT NOT NULL,
    risk TEXT NOT NULL DEFAULT 'medium',
    budget JSONB NOT NULL DEFAULT '{}',
    deadline TIMESTAMPTZ,
    input JSONB NOT NULL DEFAULT '{}',
    output_contract JSONB NOT NULL DEFAULT '{}',
    state TEXT NOT NULL DEFAULT 'created',
    retry_policy JSONB NOT NULL DEFAULT '{"max_retries": 3, "backoff_seconds": 30}',
    idempotency_key TEXT,
    lifecycle_mode TEXT NOT NULL DEFAULT 'ephemeral',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Task dependencies
CREATE TABLE task_dependencies (
    task_id UUID NOT NULL REFERENCES tasks(id),
    depends_on UUID NOT NULL REFERENCES tasks(id),
    PRIMARY KEY (task_id, depends_on)
);

-- Agents
CREATE TABLE agents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    identity_id UUID NOT NULL REFERENCES identities(id),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    mission_id UUID NOT NULL REFERENCES missions(id),
    parent_agent_id UUID REFERENCES agents(id),
    lineage_id UUID,
    name TEXT NOT NULL,
    role TEXT NOT NULL,
    genome JSONB NOT NULL DEFAULT '{}',
    state TEXT NOT NULL DEFAULT 'draft',
    lifecycle_mode TEXT NOT NULL DEFAULT 'ephemeral',
    budget JSONB NOT NULL DEFAULT '{}',
    delegable_permissions TEXT[] NOT NULL DEFAULT '{}',
    non_delegable_permissions TEXT[] NOT NULL DEFAULT '{}',
    turn_count BIGINT NOT NULL DEFAULT 0,
    total_cost_usd DOUBLE PRECISION NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    terminated_at TIMESTAMPTZ
);

-- Agent lineages
CREATE TABLE lineages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mission_id UUID NOT NULL REFERENCES missions(id),
    root_agent_id UUID NOT NULL REFERENCES agents(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Lineage entries
CREATE TABLE lineage_entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    lineage_id UUID NOT NULL REFERENCES lineages(id),
    agent_id UUID NOT NULL REFERENCES agents(id),
    parent_agent_id UUID REFERENCES agents(id),
    role TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    terminated_at TIMESTAMPTZ
);

-- Agent legacy traces
CREATE TABLE agent_legacies (
    agent_id UUID PRIMARY KEY REFERENCES agents(id),
    parent_agent_id UUID REFERENCES agents(id),
    lineage_id UUID NOT NULL REFERENCES lineages(id),
    mission_id UUID NOT NULL REFERENCES missions(id),
    lifecycle_mode TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    terminated_at TIMESTAMPTZ NOT NULL,
    termination_reason TEXT NOT NULL,
    model_calls BIGINT NOT NULL DEFAULT 0,
    tool_calls BIGINT NOT NULL DEFAULT 0,
    children_created INT NOT NULL DEFAULT 0,
    cost_usd DOUBLE PRECISION NOT NULL DEFAULT 0,
    status TEXT NOT NULL,
    trace_hash TEXT NOT NULL,
    signed_by TEXT NOT NULL
);

-- Spawn requests
CREATE TABLE spawn_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mission_id UUID NOT NULL REFERENCES missions(id),
    task_id TEXT,
    requested_by UUID NOT NULL REFERENCES agents(id),
    reason TEXT NOT NULL,
    team_spec JSONB NOT NULL DEFAULT '{}',
    children_spec JSONB NOT NULL DEFAULT '[]',
    state TEXT NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Approvals
CREATE TABLE approvals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    requested_by UUID NOT NULL REFERENCES agents(id),
    approved_by UUID REFERENCES identities(id),
    level INT NOT NULL DEFAULT 0,
    reason TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    decided_at TIMESTAMPTZ,
    expiry TIMESTAMPTZ
);

-- Policies
CREATE TABLE policies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    rules JSONB NOT NULL DEFAULT '[]',
    is_active BOOLEAN NOT NULL DEFAULT false,
    signed_by TEXT,
    signature TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    activated_at TIMESTAMPTZ
);

-- Workers
CREATE TABLE workers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    worker_type TEXT NOT NULL,
    capabilities JSONB NOT NULL DEFAULT '[]',
    state TEXT NOT NULL DEFAULT 'online',
    version TEXT NOT NULL DEFAULT '0.1.0',
    is_draining BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Memory
CREATE TABLE memory_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id TEXT NOT NULL,
    scope TEXT NOT NULL,
    memory_type TEXT NOT NULL,
    content TEXT NOT NULL,
    source_agent_id UUID,
    source_task_id UUID,
    confidence DOUBLE PRECISION NOT NULL DEFAULT 0,
    classification TEXT NOT NULL DEFAULT 'public',
    status TEXT NOT NULL DEFAULT 'candidate',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Skills
CREATE TABLE skills (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    purpose TEXT NOT NULL,
    version TEXT NOT NULL DEFAULT '1.0.0',
    input_schema JSONB NOT NULL DEFAULT '{}',
    output_schema JSONB NOT NULL DEFAULT '{}',
    steps JSONB NOT NULL DEFAULT '[]',
    state TEXT NOT NULL DEFAULT 'candidate',
    signature TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Tool invocations
CREATE TABLE tool_invocations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tool_id TEXT NOT NULL,
    agent_id UUID NOT NULL REFERENCES agents(id),
    worker_id UUID,
    mission_id UUID NOT NULL REFERENCES missions(id),
    task_id UUID REFERENCES tasks(id),
    input JSONB NOT NULL DEFAULT '{}',
    output JSONB,
    idempotency_key TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'pending',
    cost_usd DOUBLE PRECISION NOT NULL DEFAULT 0,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ
);

-- Evidence
CREATE TABLE evidence (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id UUID NOT NULL REFERENCES tasks(id),
    agent_id UUID NOT NULL REFERENCES agents(id),
    evidence_type TEXT NOT NULL,
    content TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    accepted BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Artifacts
CREATE TABLE artifacts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id UUID NOT NULL REFERENCES tasks(id),
    agent_id UUID NOT NULL REFERENCES agents(id),
    name TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size_bytes BIGINT NOT NULL DEFAULT 0,
    storage_path TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Cost records
CREATE TABLE cost_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mission_id UUID NOT NULL REFERENCES missions(id),
    task_id UUID REFERENCES tasks(id),
    agent_id TEXT,
    amount_usd DOUBLE PRECISION NOT NULL,
    category TEXT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Audit events
CREATE TABLE audit_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id TEXT,
    mission_id TEXT,
    task_id TEXT,
    agent_id TEXT,
    parent_agent_id TEXT,
    lineage_id TEXT,
    worker_id TEXT,
    trace_id TEXT,
    event_type TEXT NOT NULL,
    lifecycle_mode TEXT,
    risk_level TEXT,
    status TEXT NOT NULL,
    cost_usd DOUBLE PRECISION NOT NULL DEFAULT 0,
    payload JSONB NOT NULL DEFAULT '{}',
    timestamp TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Indexes
CREATE INDEX idx_organizations_tenant ON organizations(tenant_id);
CREATE INDEX idx_identities_org ON identities(organization_id);
CREATE INDEX idx_missions_org ON missions(organization_id);
CREATE INDEX idx_tasks_mission ON tasks(mission_id);
CREATE INDEX idx_agents_org ON agents(organization_id);
CREATE INDEX idx_agents_mission ON agents(mission_id);
CREATE INDEX idx_agents_parent ON agents(parent_agent_id);
CREATE INDEX idx_lineage_entries_lineage ON lineage_entries(lineage_id);
CREATE INDEX idx_spawn_requests_mission ON spawn_requests(mission_id);
CREATE INDEX idx_approvals_target ON approvals(target_type, target_id);
CREATE INDEX idx_audit_events_type ON audit_events(event_type);
CREATE INDEX idx_audit_events_ts ON audit_events(timestamp DESC);
CREATE INDEX idx_cost_records_mission ON cost_records(mission_id);
CREATE INDEX idx_memory_scope ON memory_records(scope);
