//! AI operations: every write the CLI makes for an AI runs as one transaction that records the
//! rows it touched (before and after) in `AiOperation`, so the operation can be undone later.
use crate::error::{CoreError, Result};
use crate::{Db, time};
use chrono::{DateTime, Utc};
use rusqlite::params;
use rusqlite::types::ValueRef;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const KEEP: i64 = 100;
const KEEP_DAYS: i64 = 30;

/// Tables an operation may touch, in parent → child order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Table {
    Client,
    Project,
    Tag,
    TagOnProject,
    TimeEntry,
    TagOnEntry,
    AiRegistration,
}

impl Table {
    pub const PARENT_FIRST: [Table; 7] = [Table::Client, Table::Tag, Table::Project, Table::TagOnProject, Table::TimeEntry, Table::TagOnEntry, Table::AiRegistration];

    pub fn name(self) -> &'static str {
        match self {
            Table::Client => "Client",
            Table::Project => "Project",
            Table::Tag => "Tag",
            Table::TagOnProject => "TagOnProject",
            Table::TimeEntry => "TimeEntry",
            Table::TagOnEntry => "TagOnEntry",
            Table::AiRegistration => "AiRegistration",
        }
    }

    pub fn pk(self) -> &'static str {
        if self == Table::AiRegistration { "requestId" } else { "id" }
    }
}

pub type Row = Map<String, Value>;

/// Rows matching `column = value` in `table`, before and after the operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub table: Table,
    pub column: String,
    pub value: String,
    pub before: Vec<Row>,
    #[serde(default)]
    pub after: Vec<Row>,
}

#[derive(Debug, Default)]
pub struct Recorder {
    changes: Vec<Change>,
}

impl Recorder {
    fn has(&self, table: Table, column: &str, value: &str) -> bool {
        self.changes.iter().any(|c| c.table == table && c.column == column && c.value == value)
    }

    /// Saves the current rows before they are changed or deleted.
    pub fn watch(&mut self, db: &Db, table: Table, column: &'static str, value: &str) -> Result<()> {
        if !self.has(table, column, value) {
            let before = db.scope_rows(table, column, value)?;
            self.changes.push(Change { table, column: column.into(), value: value.into(), before, after: vec![] });
        }
        Ok(())
    }

    /// Rows the operation created (nothing before).
    pub fn created(&mut self, table: Table, column: &'static str, value: &str) {
        if !self.has(table, column, value) {
            self.changes.push(Change { table, column: column.into(), value: value.into(), before: vec![], after: vec![] });
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub id: i64,
    pub created_at: DateTime<Utc>,
    pub source: String,
    pub summary: String,
    pub count: i64,
    pub undone_at: Option<DateTime<Utc>>,
}

fn json_of(v: ValueRef) -> Value {
    match v {
        ValueRef::Null | ValueRef::Blob(_) => Value::Null,
        ValueRef::Integer(i) => i.into(),
        ValueRef::Real(f) => serde_json::Number::from_f64(f).map(Value::Number).unwrap_or(Value::Null),
        ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
    }
}

fn op_from_row(r: &rusqlite::Row) -> rusqlite::Result<Operation> {
    let created: String = r.get(1)?;
    let undone: Option<String> = r.get(5)?;
    Ok(Operation {
        id: r.get(0)?,
        created_at: time::from_db(&created).unwrap_or_default(),
        source: r.get(2)?,
        summary: r.get(3)?,
        count: r.get(4)?,
        undone_at: undone.as_deref().and_then(time::from_db),
    })
}

const OP_SELECT: &str = r#"SELECT id, createdAt, source, summary, count, undoneAt FROM "AiOperation""#;

impl Db {
    /// Rows of `table` where `column = value`, as JSON objects sorted by primary key.
    pub(crate) fn scope_rows(&self, table: Table, column: &str, value: &str) -> Result<Vec<Row>> {
        let mut st = self.conn().prepare(&format!(r#"SELECT * FROM "{}" WHERE "{column}" = ?1 ORDER BY "{}""#, table.name(), table.pk()))?;
        let names: Vec<String> = st.column_names().iter().map(|s| s.to_string()).collect();
        let rows = st.query_map([value], |r| {
            let mut m = Map::new();
            for (i, n) in names.iter().enumerate() {
                m.insert(n.clone(), json_of(r.get_ref(i)?));
            }
            Ok(m)
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Runs `f` as one AI operation: one write transaction, the touched rows recorded before and
    /// after. `f` returns its value, a one-line summary and the number of items it changed.
    /// On error nothing is changed and nothing is recorded.
    pub fn with_operation<T>(&mut self, source: &str, f: impl FnOnce(&mut Db, &mut Recorder) -> Result<(T, String, i64)>) -> Result<(T, Operation)> {
        self.ensure_writable()?;
        if !self.conn().is_autocommit() {
            return Err(CoreError::code_msg("database_error", "操作の途中で別の操作は始められません。"));
        }
        self.conn().execute_batch("BEGIN IMMEDIATE")?;
        let result = self.run_operation(source, f);
        let end = if result.is_ok() { "COMMIT" } else { "ROLLBACK" };
        if let Err(e) = self.conn().execute_batch(end) {
            let _ = self.conn().execute_batch("ROLLBACK");
            return Err(e.into());
        }
        result
    }

    fn run_operation<T>(&mut self, source: &str, f: impl FnOnce(&mut Db, &mut Recorder) -> Result<(T, String, i64)>) -> Result<(T, Operation)> {
        let mut rec = Recorder::default();
        let (value, summary, count) = f(self, &mut rec)?;
        for c in rec.changes.iter_mut() {
            c.after = self.scope_rows(c.table, &c.column, &c.value)?;
        }
        let changes = serde_json::to_string(&rec.changes).expect("serializable");
        self.conn().execute(
            r#"INSERT INTO "AiOperation"(createdAt, source, summary, count, changes) VALUES (?1, ?2, ?3, ?4, ?5)"#,
            params![time::now_db(), source, summary, count, changes],
        )?;
        let id = self.conn().last_insert_rowid();
        self.conn().execute(
            r#"DELETE FROM "AiOperation" WHERE id NOT IN (SELECT id FROM "AiOperation" ORDER BY id DESC LIMIT ?1) OR createdAt < ?2"#,
            params![KEEP, time::to_db(Utc::now() - chrono::Duration::days(KEEP_DAYS))],
        )?;
        let op = self.operation(id)?.expect("just recorded");
        Ok((value, op))
    }

    pub fn operation(&self, id: i64) -> Result<Option<Operation>> {
        use rusqlite::OptionalExtension;
        Ok(self.conn().query_row(&format!("{OP_SELECT} WHERE id = ?1"), [id], op_from_row).optional()?)
    }

    /// Newest first.
    pub fn operations(&self, limit: i64) -> Result<Vec<Operation>> {
        let mut st = self.conn().prepare(&format!("{OP_SELECT} ORDER BY id DESC LIMIT ?1"))?;
        Ok(st.query_map([limit], op_from_row)?.collect::<std::result::Result<_, _>>()?)
    }

    /// 0 when nothing has been recorded.
    pub fn latest_operation_id(&self) -> Result<i64> {
        Ok(self.conn().query_row(r#"SELECT COALESCE(MAX(id), 0) FROM "AiOperation""#, [], |r| r.get(0))?)
    }

    /// Operations newer than `id`, newest first.
    pub fn operations_after(&self, id: i64) -> Result<Vec<Operation>> {
        let mut st = self.conn().prepare(&format!("{OP_SELECT} WHERE id > ?1 ORDER BY id DESC"))?;
        Ok(st.query_map([id], op_from_row)?.collect::<std::result::Result<_, _>>()?)
    }
}
