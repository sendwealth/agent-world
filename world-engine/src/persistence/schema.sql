-- World Engine SQLite Persistence Schema
-- Stores structured world state snapshots for crash recovery.

PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;

-- Snapshots metadata: tracks each full world state dump
CREATE TABLE IF NOT EXISTS snapshots (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    tick        INTEGER NOT NULL,
    agent_count INTEGER NOT NULL,
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(tick)
);

-- Agent records within a snapshot
-- Composite PK: same agent can appear in different snapshots
CREATE TABLE IF NOT EXISTS agents (
    id          TEXT    NOT NULL,  -- UUID as text
    name        TEXT    NOT NULL,
    phase       TEXT    NOT NULL,
    tokens      INTEGER NOT NULL,
    spawn_tick  INTEGER NOT NULL,
    skills_json TEXT    NOT NULL DEFAULT '{}',  -- JSON-serialized HashMap<String, SkillRecord>
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
    PRIMARY KEY (id, snapshot_id)
);

CREATE INDEX IF NOT EXISTS idx_agents_snapshot_id ON agents(snapshot_id);
CREATE INDEX IF NOT EXISTS idx_agents_phase ON agents(phase);

-- Economy ledger entries (per-snapshot aggregate)
CREATE TABLE IF NOT EXISTS economy_ledger (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    snapshot_id     INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
    total_tokens    INTEGER NOT NULL,
    total_agents    INTEGER NOT NULL,
    living_agents   INTEGER NOT NULL,
    gini            REAL    NOT NULL DEFAULT 0.0,
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_economy_ledger_snapshot_id ON economy_ledger(snapshot_id);

-- Organizations (snapshot-relative)
CREATE TABLE IF NOT EXISTS organizations (
    id          TEXT    PRIMARY KEY,
    name        TEXT    NOT NULL,
    org_type    TEXT    NOT NULL DEFAULT 'guild',
    founder_id  TEXT    NOT NULL,
    created_tick INTEGER NOT NULL DEFAULT 0,
    metadata_json TEXT  NOT NULL DEFAULT '{}',
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_organizations_snapshot_id ON organizations(snapshot_id);

-- Tasks (snapshot-relative)
CREATE TABLE IF NOT EXISTS tasks (
    id          TEXT    PRIMARY KEY,
    publisher   TEXT    NOT NULL,
    assignee    TEXT,
    reward      INTEGER NOT NULL DEFAULT 0,
    status      TEXT    NOT NULL DEFAULT 'open',
    created_tick INTEGER NOT NULL DEFAULT 0,
    snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_tasks_snapshot_id ON tasks(snapshot_id);

-- Foreign worlds: registry of discovered cross-world endpoints
CREATE TABLE IF NOT EXISTS foreign_worlds (
    id                  TEXT    PRIMARY KEY,
    name                TEXT    NOT NULL,
    endpoint            TEXT    NOT NULL,
    diplomatic_status   TEXT    NOT NULL DEFAULT 'neutral',
    relation_score      INTEGER NOT NULL DEFAULT 0,
    online              INTEGER NOT NULL DEFAULT 1,
    discovered_tick     INTEGER NOT NULL DEFAULT 0,
    last_seen_tick      INTEGER NOT NULL DEFAULT 0
);

-- Cross-world treaties: diplomatic agreements with foreign worlds
CREATE TABLE IF NOT EXISTS cross_world_treaties (
    id                  TEXT    PRIMARY KEY,
    foreign_world_id    TEXT    NOT NULL REFERENCES foreign_worlds(id) ON DELETE CASCADE,
    treaty_type         TEXT    NOT NULL,
    status              TEXT    NOT NULL DEFAULT 'proposed',
    proposed_tick       INTEGER NOT NULL DEFAULT 0,
    accepted_tick       INTEGER,
    ended_tick          INTEGER,
    duration_ticks      INTEGER,
    terms               TEXT    NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_cross_world_treaties_world ON cross_world_treaties(foreign_world_id);
CREATE INDEX IF NOT EXISTS idx_cross_world_treaties_status ON cross_world_treaties(status);-- ═══════════════════════════════════════════════════════════
-- Federation / Cross-World Migration Tables (SEN-320)
-- ═══════════════════════════════════════════════════════════

-- Registered foreign worlds in the federation
CREATE TABLE IF NOT EXISTS federation_worlds (
    world_id        TEXT    PRIMARY KEY,
    name            TEXT    NOT NULL,
    description     TEXT    NOT NULL DEFAULT '',
    host            TEXT    NOT NULL,
    grpc_port       INTEGER NOT NULL DEFAULT 50051,
    http_port       INTEGER NOT NULL DEFAULT 8080,
    status          TEXT    NOT NULL DEFAULT 'online',
    capabilities    TEXT    NOT NULL DEFAULT '[]',  -- JSON array of strings
    max_agents      INTEGER NOT NULL DEFAULT 100,
    current_agents  INTEGER NOT NULL DEFAULT 0,
    labels          TEXT    NOT NULL DEFAULT '{}',  -- JSON object
    metrics_json    TEXT    NOT NULL DEFAULT '{}',  -- JSON WorldMetrics
    registered_at   TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_heartbeat  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Migration applications (immigration requests)
CREATE TABLE IF NOT EXISTS migration_applications (
    migration_id    TEXT    PRIMARY KEY,
    agent_id        TEXT    NOT NULL,
    source_world_id TEXT    NOT NULL,
    target_world_id TEXT    NOT NULL,
    status          TEXT    NOT NULL DEFAULT 'pending',
    agent_snapshot  TEXT    NOT NULL DEFAULT '{}',  -- JSON AgentSnapshot
    rejection_reason TEXT,
    submitted_at    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    reviewed_at     TEXT,
    completed_at    TEXT,
    token_cost      INTEGER NOT NULL DEFAULT 0,
    resource_tax_rate REAL  NOT NULL DEFAULT 0.0,
    metadata_json   TEXT    NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_migration_agent_id ON migration_applications(agent_id);
CREATE INDEX IF NOT EXISTS idx_migration_source ON migration_applications(source_world_id);
CREATE INDEX IF NOT EXISTS idx_migration_target ON migration_applications(target_world_id);
CREATE INDEX IF NOT EXISTS idx_migration_status ON migration_applications(status);

-- Completed migration records (audit trail)
CREATE TABLE IF NOT EXISTS migration_records (
    migration_id    TEXT    PRIMARY KEY,
    agent_id        TEXT    NOT NULL,
    source_world_id TEXT    NOT NULL,
    target_world_id TEXT    NOT NULL,
    migration_type  TEXT    NOT NULL DEFAULT 'permanent',
    token_cost      INTEGER NOT NULL DEFAULT 0,
    resource_tax_collected INTEGER NOT NULL DEFAULT 0,
    tokens_remaining INTEGER NOT NULL DEFAULT 0,
    money_remaining INTEGER NOT NULL DEFAULT 0,
    skills_transferred TEXT NOT NULL DEFAULT '[]',  -- JSON array
    skills_blocked  TEXT    NOT NULL DEFAULT '[]',  -- JSON array
    submitted_at    TEXT    NOT NULL,
    completed_at    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_migration_records_agent ON migration_records(agent_id);

-- Migration policy per world
CREATE TABLE IF NOT EXISTS migration_policy (
    world_id                    TEXT    PRIMARY KEY,
    enabled                     INTEGER NOT NULL DEFAULT 1,
    daily_quota                 INTEGER NOT NULL DEFAULT 10,
    weekly_quota                INTEGER NOT NULL DEFAULT 50,
    min_reputation              REAL    NOT NULL DEFAULT 0.0,
    token_cost                  INTEGER NOT NULL DEFAULT 10000,
    resource_tax_rate           REAL    NOT NULL DEFAULT 0.2,
    require_skill_certification INTEGER NOT NULL DEFAULT 0,
    blocked_skills              TEXT    NOT NULL DEFAULT '[]',
    cooldown_ticks              INTEGER NOT NULL DEFAULT 100
);

-- Provider Configuration and Agent Model Assignments (SEN-574)
CREATE TABLE IF NOT EXISTS provider_configs (
    id                TEXT    PRIMARY KEY,
    protocol          TEXT    NOT NULL,
    base_url          TEXT    NOT NULL,
    api_key_encrypted BLOB    NOT NULL,
    api_key_nonce     BLOB    NOT NULL,
    api_version       TEXT    NOT NULL DEFAULT '',
    display_name      TEXT    NOT NULL DEFAULT '',
    is_default        INTEGER NOT NULL DEFAULT 0,
    created_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_provider_configs_protocol ON provider_configs(protocol);
CREATE INDEX IF NOT EXISTS idx_provider_configs_is_default ON provider_configs(is_default);

CREATE TABLE IF NOT EXISTS agent_model_assignments (
    agent_id     TEXT    NOT NULL,
    provider_id  TEXT    NOT NULL REFERENCES provider_configs(id) ON DELETE CASCADE,
    model_id     TEXT    NOT NULL DEFAULT '',
    updated_at   TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (agent_id)
);

CREATE INDEX IF NOT EXISTS idx_agent_model_assignments_provider ON agent_model_assignments(provider_id);
