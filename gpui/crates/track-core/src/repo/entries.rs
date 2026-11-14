//! Time entries: overlap query, create/update/delete with the Bun server's rules.
use crate::error::{CoreError, Result};
use crate::model::{Client, EntryPatch, NewEntry, Project, Tag, TagLink, TimeEntry};
use crate::validate::{check_break, check_same_day, check_text, invalid, normalize_tag_ids, snap_to_quarter};
use crate::{Db, ids, time};
use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, Row, params};

const ENTRY_SELECT: &str = r#"SELECT e.id, e.projectId, e.start, e."end", e.title, e.note, e.externalEventId, e.externalEventSource, e.breakMinutes,
       p.id, p.clientId, p.name, p.color, p.archived, c.id, c.name, c.archived, e.kind
FROM "TimeEntry" e LEFT JOIN "Project" p ON p.id = e.projectId LEFT JOIN "Client" c ON c.id = p.clientId"#;

fn parse_time(s: String) -> rusqlite::Result<DateTime<Utc>> {
    time::from_db(&s).ok_or_else(|| rusqlite::Error::InvalidColumnType(0, s, rusqlite::types::Type::Text))
}

fn entry_from_row(r: &Row) -> rusqlite::Result<TimeEntry> {
    let project = match r.get::<_, Option<String>>(9)? {
        Some(pid) => Some(Project {
            id: pid,
            client_id: r.get(10)?,
            name: r.get(11)?,
            color: r.get(12)?,
            archived: r.get::<_, i64>(13)? != 0,
            client: Client { id: r.get(14)?, name: r.get(15)?, archived: r.get::<_, i64>(16)? != 0 },
            tags: vec![],
        }),
        None => None,
    };
    Ok(TimeEntry {
        id: r.get(0)?,
        project_id: r.get(1)?,
        start: parse_time(r.get(2)?)?,
        end: parse_time(r.get(3)?)?,
        title: r.get(4)?,
        note: r.get(5)?,
        project,
        tags: vec![],
        external_event_id: r.get(6)?,
        external_event_source: r.get(7)?,
        break_minutes: r.get(8)?,
        kind: crate::model::EntryKind::from_db(&r.get::<_, String>(17)?),
    })
}

impl Db {
    fn hydrate(&self, mut e: TimeEntry) -> Result<TimeEntry> {
        if let Some(p) = e.project.as_mut() {
            p.tags = self.project_tags(&p.id)?;
        }
        let mut st = self.conn().prepare(
            r#"SELECT t.id, t.name, t.color FROM "TagOnEntry" te JOIN "Tag" t ON t.id = te.tagId WHERE te.entryId = ?1 ORDER BY t.name"#,
        )?;
        e.tags = st
            .query_map([&e.id], |r| {
                let tag = Tag { id: r.get(0)?, name: r.get(1)?, color: r.get(2)? };
                Ok(TagLink { tag_id: tag.id.clone(), tag })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(e)
    }

    pub fn entry(&self, id: &str) -> Result<Option<TimeEntry>> {
        let e = self.conn().query_row(&format!("{ENTRY_SELECT} WHERE e.id = ?1"), [id], entry_from_row).optional()?;
        e.map(|e| self.hydrate(e)).transpose()
    }

    /// Entries overlapping `[from, to)` (`start < to AND end > from`), oldest first.
    pub fn entries(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<TimeEntry>> {
        let mut st = self.conn().prepare(&format!(r#"{ENTRY_SELECT} WHERE e.start < ?1 AND e."end" > ?2 ORDER BY e.start ASC"#))?;
        let rows: Vec<TimeEntry> = st.query_map([time::to_db(to), time::to_db(from)], entry_from_row)?.collect::<std::result::Result<_, _>>()?;
        rows.into_iter().map(|e| self.hydrate(e)).collect()
    }

    /// Plain (non-AI) create: rounds to 15 minutes, same-day rule.
    pub fn create_entry(&mut self, input: NewEntry) -> Result<TimeEntry> {
        self.ensure_writable()?;
        let (start, end) = check_new_entry(&input)?;
        let tx = self.conn_mut().savepoint()?;
        let id = insert_entry(&tx, &input, start, end)?;
        tx.commit()?;
        Ok(self.entry(&id)?.expect("just created"))
    }

    pub fn update_entry(&mut self, id: &str, patch: EntryPatch) -> Result<TimeEntry> {
        self.ensure_writable()?;
        let existing = self.entry(id)?.ok_or_else(|| CoreError::code_msg("not_found", "工数が見つかりません。"))?;
        if let (Some(s), Some(e)) = (patch.start, patch.end)
            && e <= s
        {
            return Err(invalid("終了は開始より後にしてください。"));
        }
        if let Some(t) = &patch.title {
            check_text(t, &None)?;
        }
        if let Some(n) = &patch.note {
            check_text(&None, n)?;
        }
        if let Some(b) = patch.break_minutes {
            check_break(b)?;
        }
        let start = patch.start.map(snap_to_quarter).unwrap_or(existing.start);
        let end = patch.end.map(snap_to_quarter).unwrap_or(existing.end);
        check_same_day(start, end)?;
        let project_id = patch.project_id.clone().unwrap_or(existing.project_id.clone());
        let title = patch.title.clone().unwrap_or(existing.title.clone());
        let note = patch.note.clone().unwrap_or(existing.note.clone());
        let break_minutes = patch.break_minutes.unwrap_or(existing.break_minutes);
        let tx = self.conn_mut().savepoint()?;
        tx.execute(
            r#"UPDATE "TimeEntry" SET projectId = ?2, start = ?3, "end" = ?4, title = ?5, note = ?6, breakMinutes = ?7, updatedAt = ?8 WHERE id = ?1"#,
            params![id, project_id, time::to_db(start), time::to_db(end), title, note, break_minutes, time::now_db()],
        )?;
        if let Some(tags) = &patch.tag_ids {
            tx.execute(r#"DELETE FROM "TagOnEntry" WHERE entryId = ?1"#, [id])?;
            for t in tags {
                tx.execute(r#"INSERT INTO "TagOnEntry"(id, tagId, entryId) VALUES (?1, ?2, ?3)"#, params![ids::cuid(), t, id])?;
            }
        }
        tx.commit()?;
        Ok(self.entry(id)?.expect("exists"))
    }

    pub fn delete_entry(&mut self, id: &str) -> Result<()> {
        self.ensure_writable()?;
        let n = self.conn().execute(r#"DELETE FROM "TimeEntry" WHERE id = ?1"#, [id])?;
        if n == 0 {
            return Err(CoreError::code_msg("not_found", "工数が見つかりません。"));
        }
        Ok(())
    }

    /// Entries matching every given condition, newest first (at most 1000).
    pub fn search_entries(&self, q: &crate::edit::EntrySearch, limit: i64) -> Result<Vec<TimeEntry>> {
        use rusqlite::types::Value;
        let mut cond: Vec<String> = Vec::new();
        let mut args: Vec<Value> = Vec::new();
        match q.project_id.as_deref() {
            Some("none") => cond.push("e.projectId IS NULL".into()),
            Some(p) => {
                args.push(Value::Text(p.into()));
                cond.push(format!("e.projectId = ?{}", args.len()));
            }
            None => {}
        }
        if let Some(text) = q.query.as_deref().filter(|t| !t.is_empty()) {
            let escaped = text.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
            args.push(Value::Text(format!("%{escaped}%")));
            let n = args.len();
            cond.push(format!(r"(e.title LIKE ?{n} ESCAPE '\' OR e.note LIKE ?{n} ESCAPE '\')"));
        }
        if let Some(from) = q.from {
            args.push(Value::Text(time::to_db(from)));
            cond.push(format!(r#"e."end" > ?{}"#, args.len()));
        }
        if let Some(to) = q.to {
            args.push(Value::Text(time::to_db(to)));
            cond.push(format!("e.start < ?{}", args.len()));
        }
        let where_ = if cond.is_empty() { String::new() } else { format!("WHERE {}", cond.join(" AND ")) };
        args.push(Value::Integer(limit.clamp(1, 1000)));
        let sql = format!("{ENTRY_SELECT} {where_} ORDER BY e.start DESC LIMIT ?{}", args.len());
        let mut st = self.conn().prepare(&sql)?;
        let rows: Vec<TimeEntry> = st.query_map(rusqlite::params_from_iter(args), entry_from_row)?.collect::<std::result::Result<_, _>>()?;
        rows.into_iter().map(|e| self.hydrate(e)).collect()
    }

    /// Up to 100 distinct trimmed titles from the newest 500 entries; `Some("none")` = no project.
    pub fn entry_titles(&self, project: Option<&str>) -> Result<Vec<String>> {
        let (cond, arg): (&str, Option<&str>) = match project {
            Some("none") => (r#"AND projectId IS NULL"#, None),
            Some(p) => ("AND projectId = ?1", Some(p)),
            None => ("", None),
        };
        let sql = format!(r#"SELECT title FROM "TimeEntry" WHERE title IS NOT NULL {cond} ORDER BY start DESC LIMIT 500"#);
        let mut st = self.conn().prepare(&sql)?;
        let titles: Vec<String> = match arg {
            Some(a) => st.query_map([a], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?,
            None => st.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?,
        };
        let mut out: Vec<String> = Vec::new();
        for t in titles {
            let t = t.trim().to_string();
            if !t.is_empty() && !out.contains(&t) {
                out.push(t);
                if out.len() >= 100 {
                    break;
                }
            }
        }
        Ok(out)
    }
}

/// Validation for a new entry; returns the snapped (start, end).
pub(crate) fn check_new_entry(input: &NewEntry) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    if input.end <= input.start {
        return Err(invalid("終了は開始より後にしてください。"));
    }
    check_text(&input.title, &input.note)?;
    check_break(input.break_minutes)?;
    let (start, end) = (snap_to_quarter(input.start), snap_to_quarter(input.end));
    check_same_day(start, end)?;
    if end <= start {
        return Err(CoreError::code_msg("invalid_rounded_interval", "15分単位に丸めると長さが0になります。"));
    }
    Ok((start, end))
}

pub(crate) fn insert_entry(tx: &rusqlite::Connection, input: &NewEntry, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<String> {
    let id = ids::cuid();
    let now = time::now_db();
    tx.execute(
        r#"INSERT INTO "TimeEntry"(id, projectId, start, "end", title, note, externalEventId, externalEventSource, breakMinutes, createdAt, updatedAt)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)"#,
        params![id, input.project_id, time::to_db(start), time::to_db(end), input.title, input.note, input.external_event_id, input.external_event_source, input.break_minutes, now],
    )?;
    for t in normalize_tag_ids(&input.tag_ids) {
        tx.execute(r#"INSERT INTO "TagOnEntry"(id, tagId, entryId) VALUES (?1, ?2, ?3)"#, params![ids::cuid(), t, id])?;
    }
    Ok(id)
}
