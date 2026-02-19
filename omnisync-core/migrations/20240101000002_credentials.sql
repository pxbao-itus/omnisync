CREATE TABLE IF NOT EXISTS credentials (
    provider_id TEXT PRIMARY KEY,
    access_token TEXT NOT NULL,
    refresh_token TEXT,
    expires_at INTEGER
);
