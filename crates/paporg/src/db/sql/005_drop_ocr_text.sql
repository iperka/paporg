-- Remove ocr_text column from jobs table if it exists.
-- This is handled conditionally by the migration runner since
-- ALTER TABLE DROP COLUMN may not exist on older SQLite and the
-- column may not exist on fresh installs.
ALTER TABLE jobs DROP COLUMN ocr_text;
