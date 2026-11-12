use crate::error::{CoreError, Result};
use crate::{ids, migrate, time};
use rusqlite::{Connection, OptionalExtension};
use std::path::{Path, PathBuf};

pub struct Db {
    conn: Connection,
    pub path: PathBuf,
    /// The DB has migrations this binary does not know: never write.
    pub read_only: bool,
    pub applied_now: Vec<String>,
}

impl Db {
    /// Opens `<data_dir>/track.db` (WAL), applies pending migrations and makes sure the
    /// single `Settings` row exists.
    pub fn open(data_dir: &Path) -> Result<Db> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("track.db");
        let conn = Connection::open(&path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
        let state = migrate::run(&conn)?;
        let db = Db { conn, path, read_only: !state.unknown.is_empty(), applied_now: state.applied_now };
        if !db.read_only {
            db.ensure_settings_row()?;
        }
        Ok(db)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    pub fn ensure_writable(&self) -> Result<()> {
        if self.read_only {
            Err(CoreError::code_msg("schema_too_new", "DB がこのバージョンより新しい形式です。Track を更新してください。"))
        } else {
            Ok(())
        }
    }

    /// Changes whenever another connection commits (CLI, Tauri server).
    pub fn data_version(&self) -> Result<i64> {
        Ok(self.conn.query_row("PRAGMA data_version", [], |r| r.get(0))?)
    }

    fn ensure_settings_row(&self) -> Result<()> {
        let exists: Option<String> = self.conn.query_row(r#"SELECT id FROM "Settings" LIMIT 1"#, [], |r| r.get(0)).optional()?;
        if exists.is_none() {
            self.conn.execute(r#"INSERT INTO "Settings" (id, createdAt) VALUES (?1, ?2)"#, [ids::cuid(), time::now_db()])?;
        }
        Ok(())
    }
}
