-- T1.0.2.17 — switch_history table.
--
-- Records every provider switch event so users can audit failover
-- behaviour. One row per switch (not per attempt).

CREATE TABLE IF NOT EXISTS switch_history (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    from_provider   TEXT,           -- NULL on first-ever dispatch
    to_provider     TEXT NOT NULL,
    strategy        TEXT NOT NULL,  -- snake_case enum value
    reason          TEXT NOT NULL,  -- e.g. "upstream_500", "rate_limited", "manual"
    attempts        INTEGER NOT NULL DEFAULT 1,
    FOREIGN KEY (to_provider) REFERENCES providers(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS switch_history_ts_idx
    ON switch_history (timestamp DESC);
