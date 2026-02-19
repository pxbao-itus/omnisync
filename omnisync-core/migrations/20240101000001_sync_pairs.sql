CREATE TABLE IF NOT EXISTS sync_pairs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    local_path TEXT NOT NULL UNIQUE,
    remote_path TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active', -- active, paused, error
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX idx_sync_pairs_local_path ON sync_pairs(local_path);
