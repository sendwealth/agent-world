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
