-- Add mime_type column if it does not already exist.
-- This is handled conditionally by the migration runner since
-- ALTER TABLE ADD COLUMN is not idempotent in SQLite.
ALTER TABLE jobs ADD COLUMN mime_type TEXT;
CREATE INDEX IF NOT EXISTS idx_jobs_mime_type ON jobs(mime_type);
