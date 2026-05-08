-- T1.0.6.03 — allow deleting providers referenced by switch_history.
--
-- Migration 0002 declared `to_provider TEXT NOT NULL` while also using
-- `ON DELETE SET NULL`. SQLite correctly tried to clear switch history
-- references when a provider was deleted, then failed on the NOT NULL
-- constraint. Rebuild the table with a nullable target provider.

CREATE TABLE switch_history_new (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    from_provider   TEXT,
    to_provider     TEXT,
    strategy        TEXT NOT NULL,
    reason          TEXT NOT NULL,
    attempts        INTEGER NOT NULL DEFAULT 1,
    FOREIGN KEY (to_provider) REFERENCES providers(id) ON DELETE SET NULL
);

INSERT INTO switch_history_new (
    id,
    timestamp,
    from_provider,
    to_provider,
    strategy,
    reason,
    attempts
)
SELECT
    id,
    timestamp,
    from_provider,
    CASE
        WHEN EXISTS (SELECT 1 FROM providers WHERE providers.id = switch_history.to_provider)
        THEN to_provider
        ELSE NULL
    END,
    strategy,
    reason,
    attempts
FROM switch_history;

DROP TABLE switch_history;
ALTER TABLE switch_history_new RENAME TO switch_history;

CREATE INDEX IF NOT EXISTS switch_history_ts_idx
    ON switch_history (timestamp DESC);
