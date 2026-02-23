-- Add remote_name column to sync_pairs table
ALTER TABLE sync_pairs ADD COLUMN remote_name TEXT NOT NULL DEFAULT 'Unknown';
