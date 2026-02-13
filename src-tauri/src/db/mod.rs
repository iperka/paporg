use tauri_plugin_sql::{Migration, MigrationKind};

pub fn migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 1,
            description: "create_jobs_table",
            sql: include_str!("migrations/001_create_jobs.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 2,
            description: "create_processed_emails_table",
            sql: include_str!("migrations/002_create_processed_emails.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 3,
            description: "create_oauth_tokens_table",
            sql: include_str!("migrations/003_create_oauth_tokens.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 4,
            description: "add_mime_type_to_jobs",
            sql: include_str!("migrations/004_add_mime_type.sql"),
            kind: MigrationKind::Up,
        },
    ]
}
