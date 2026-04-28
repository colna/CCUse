-- T1.0.1.15 — initial schema.
--
-- Two tables for Phase 1.0.1:
--   providers   — encrypted upstream API credentials & metadata
--   app_config  — generic key/value app settings (theme, ports, …)
--
-- T1.0.2.17–18 add `switch_history` and `request_logs` later.

CREATE TABLE IF NOT EXISTS providers (
    id                  TEXT PRIMARY KEY NOT NULL,
    name                TEXT NOT NULL,
    kind                TEXT NOT NULL,
    base_url            TEXT NOT NULL,
    encrypted_api_key   BLOB NOT NULL,
    priority            INTEGER NOT NULL DEFAULT 100,
    enabled             INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS providers_priority_idx
    ON providers (enabled DESC, priority ASC);

CREATE TABLE IF NOT EXISTS app_config (
    key         TEXT PRIMARY KEY NOT NULL,
    value       TEXT NOT NULL,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
