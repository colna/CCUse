-- T1.0.2.18 — request_logs table.
--
-- Lightweight metadata log for every proxied request. Does NOT store
-- request/response bodies — only timing, token counts, and outcome.

CREATE TABLE IF NOT EXISTS request_logs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    provider_id     TEXT NOT NULL,
    model           TEXT NOT NULL,
    status          TEXT NOT NULL,  -- "ok", "error"
    error_kind      TEXT,           -- NULL on success; ProviderError variant name
    latency_ms      INTEGER NOT NULL DEFAULT 0,
    prompt_tokens   INTEGER,
    completion_tokens INTEGER,
    total_tokens    INTEGER,
    stream          INTEGER NOT NULL DEFAULT 0 CHECK (stream IN (0, 1)),
    FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS request_logs_ts_idx
    ON request_logs (timestamp DESC);

CREATE INDEX IF NOT EXISTS request_logs_provider_idx
    ON request_logs (provider_id, timestamp DESC);
