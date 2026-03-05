-- Files tracking table
CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    hash TEXT,
    size INTEGER,
    modified_at INTEGER,
    status TEXT DEFAULT 'pending' -- pending, synced, conflict
);

CREATE INDEX idx_files_path ON files(path);

-- Credentials table: supports multiple accounts per provider via account_id
-- account_id format: "provider:email" e.g. "gdrive:user@gmail.com"
CREATE TABLE IF NOT EXISTS credentials (
    account_id TEXT PRIMARY KEY,
    provider_id TEXT NOT NULL,
    access_token TEXT NOT NULL,
    refresh_token TEXT,
    expires_at INTEGER,
    user_name TEXT,
    user_email TEXT,
    user_avatar TEXT
);

CREATE INDEX idx_credentials_provider ON credentials(provider_id);

-- Sync pairs table: each pair links to a specific account
CREATE TABLE IF NOT EXISTS sync_pairs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    local_path TEXT NOT NULL,
    remote_path TEXT NOT NULL,
    remote_name TEXT NOT NULL DEFAULT 'Unknown',
    provider_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active', -- active, paused, error
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    last_sync_at INTEGER
);

CREATE INDEX idx_sync_pairs_local_path ON sync_pairs(local_path);
CREATE INDEX idx_sync_pairs_account ON sync_pairs(account_id);
