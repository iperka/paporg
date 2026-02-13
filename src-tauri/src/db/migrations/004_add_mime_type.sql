-- Migration: Add mime_type column to jobs table
-- Note: This migration is tracked by tauri_plugin_sql and will only run once.
-- The ALTER TABLE will fail if run manually a second time (no IF NOT EXISTS in SQLite).
ALTER TABLE jobs ADD COLUMN mime_type TEXT;

-- Create index (idempotent with IF NOT EXISTS)
CREATE INDEX IF NOT EXISTS idx_jobs_mime_type ON jobs(mime_type);
