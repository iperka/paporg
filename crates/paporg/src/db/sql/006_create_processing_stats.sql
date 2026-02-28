CREATE TABLE IF NOT EXISTS processing_stats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    date TEXT NOT NULL,
    category TEXT,
    source_name TEXT,
    mime_type TEXT,
    total_processed INTEGER NOT NULL DEFAULT 0,
    total_succeeded INTEGER NOT NULL DEFAULT 0,
    total_failed INTEGER NOT NULL DEFAULT 0,
    avg_duration_ms INTEGER NOT NULL DEFAULT 0,
    UNIQUE(date, category, source_name, mime_type)
);
CREATE INDEX IF NOT EXISTS idx_processing_stats_date ON processing_stats(date);
CREATE INDEX IF NOT EXISTS idx_processing_stats_category ON processing_stats(category);
CREATE INDEX IF NOT EXISTS idx_processing_stats_source ON processing_stats(source_name);
