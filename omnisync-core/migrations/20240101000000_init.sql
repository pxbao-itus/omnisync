CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    hash TEXT,
    size INTEGER,
    modified_at INTEGER,
    status TEXT DEFAULT 'pending' -- pending, synced, conflict
);

CREATE INDEX idx_files_path ON files(path);
