CREATE TABLE IF NOT EXISTS processed_emails (
    id TEXT PRIMARY KEY,
    source_name TEXT NOT NULL,
    uidvalidity INTEGER NOT NULL,
    uid INTEGER NOT NULL,
    message_id TEXT,
    processed_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_processed_emails_source_validity ON processed_emails(source_name, uidvalidity);
CREATE UNIQUE INDEX IF NOT EXISTS idx_processed_emails_unique ON processed_emails(source_name, uidvalidity, uid);
