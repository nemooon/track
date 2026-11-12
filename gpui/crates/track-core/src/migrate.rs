//! Applies `database/migrations/*.sql` exactly like `src/server/db/migrate.ts`:
//! same `_local_migrations` table, lexicographic order, one transaction per file,
//! FK checks off during migration and a `foreign_key_check` afterwards.
use crate::error::{CoreError, Result};
use rusqlite::Connection;

pub const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_init.sql", include_str!("../../../../database/migrations/0001_init.sql")),
    ("0002_tags.sql", include_str!("../../../../database/migrations/0002_tags.sql")),
    ("0003_tags_on_project.sql", include_str!("../../../../database/migrations/0003_tags_on_project.sql")),
    ("0004_external_event_link.sql", include_str!("../../../../database/migrations/0004_external_event_link.sql")),
    ("0005_break_minutes.sql", include_str!("../../../../database/migrations/0005_break_minutes.sql")),
    ("0006_personal_access_tokens.sql", include_str!("../../../../database/migrations/0006_personal_access_tokens.sql")),
    ("0007_drop_account_tables.sql", include_str!("../../../../database/migrations/0007_drop_account_tables.sql")),
    ("0008_drop_user_id.sql", include_str!("../../../../database/migrations/0008_drop_user_id.sql")),
    ("0009_weekly_report_template.sql", include_str!("../../../../database/migrations/0009_weekly_report_template.sql")),
    ("0010_notes.sql", include_str!("../../../../database/migrations/0010_notes.sql")),
    ("0011_note_archived.sql", include_str!("../../../../database/migrations/0011_note_archived.sql")),
    ("0012_note_pinned.sql", include_str!("../../../../database/migrations/0012_note_pinned.sql")),
    ("0013_report_copy_formats.sql", include_str!("../../../../database/migrations/0013_report_copy_formats.sql")),
    ("0014_ai_registration.sql", include_str!("../../../../database/migrations/0014_ai_registration.sql")),
    ("0015_ai_registration_interval.sql", include_str!("../../../../database/migrations/0015_ai_registration_interval.sql")),
];

pub struct MigrationState {
    pub applied_now: Vec<String>,
    /// Names recorded in the DB that this binary does not know (DB is newer).
    pub unknown: Vec<String>,
}

pub fn run(conn: &Connection) -> Result<MigrationState> {
    conn.execute_batch(
        r#"PRAGMA foreign_keys = OFF;
           PRAGMA legacy_alter_table = ON;
           CREATE TABLE IF NOT EXISTS "_local_migrations" (
             "name" TEXT NOT NULL PRIMARY KEY,
             "appliedAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
           );"#,
    )?;
    let applied: Vec<String> = conn
        .prepare(r#"SELECT name FROM "_local_migrations""#)?
        .query_map([], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?;
    let known: Vec<&str> = MIGRATIONS.iter().map(|(n, _)| *n).collect();
    let unknown: Vec<String> = applied.iter().filter(|a| !known.contains(&a.as_str())).cloned().collect();
    let mut applied_now = Vec::new();
    if unknown.is_empty() {
        for (name, sql) in MIGRATIONS {
            if applied.iter().any(|a| a == name) {
                continue;
            }
            conn.execute_batch("BEGIN")?;
            let step = conn.execute_batch(sql).and_then(|_| {
                conn.execute(r#"INSERT INTO "_local_migrations" (name) VALUES (?1)"#, [name]).map(|_| ())
            });
            match step {
                Ok(()) => conn.execute_batch("COMMIT")?,
                Err(e) => {
                    conn.execute_batch("ROLLBACK")?;
                    return Err(CoreError::code_msg("migration_failed", format!("migration {name} failed: {e}")));
                }
            }
            applied_now.push(name.to_string());
        }
        if !applied_now.is_empty() {
            let violations: i64 = conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0))?;
            if violations > 0 {
                return Err(CoreError::code_msg("migration_failed", format!("マイグレーション後に外部キー違反が {violations} 件")));
            }
        }
    }
    conn.execute_batch("PRAGMA legacy_alter_table = OFF; PRAGMA foreign_keys = ON;")?;
    Ok(MigrationState { applied_now, unknown })
}
