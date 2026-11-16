//! Idempotent AI registration (POST /api/entries with `aiRegistration` in the Bun server).
use crate::error::{CoreError, Result};
use crate::model::{NewEntry, TimeEntry};
use crate::repo::entries::{check_new_entry, insert_entry};
use crate::validate::{invalid, is_quarter, normalize_tag_ids};
use crate::{Db, time};
use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, params};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq)]
pub struct AiRegistrationInput {
    pub request_id: String,
    pub source: String,
    pub session_id: String,
    pub allow_overlap: bool,
}

/// A registered interval: (`recordedFrom`, `recordedUntil`).
pub type Coverage = (Option<DateTime<Utc>>, DateTime<Utc>);

fn js_iso(t: DateTime<Utc>) -> String {
    t.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

fn js(v: impl serde::Serialize) -> String {
    serde_json::to_string(&v).unwrap()
}

/// sha256 of the same `JSON.stringify` the Bun server hashes, so requests registered by
/// either implementation replay correctly. Key order:
/// projectId, start, end, title, note, externalEventId, externalEventSource, breakMinutes, [tags], source, sessionId.
pub fn payload_hash(e: &NewEntry, source: &str, session_id: &str) -> String {
    let tags = normalize_tag_ids(&e.tag_ids);
    let mut s = String::from("{");
    s += &format!("\"projectId\":{}", js(&e.project_id));
    s += &format!(",\"start\":{}", js(js_iso(e.start)));
    s += &format!(",\"end\":{}", js(js_iso(e.end)));
    s += &format!(",\"title\":{}", js(&e.title));
    s += &format!(",\"note\":{}", js(&e.note));
    s += &format!(",\"externalEventId\":{}", js(&e.external_event_id));
    s += &format!(",\"externalEventSource\":{}", js(&e.external_event_source));
    s += &format!(",\"breakMinutes\":{}", e.break_minutes);
    if !tags.is_empty() {
        let items: Vec<String> = tags.iter().map(|t| format!("{{\"tagId\":{}}}", js(t))).collect();
        s += &format!(",\"tags\":{{\"create\":[{}]}}", items.join(","));
    }
    s += &format!(",\"source\":{}", js(source));
    s += &format!(",\"sessionId\":{}", js(session_id));
    s += "}";
    format!("{:x}", Sha256::digest(s.as_bytes()))
}

fn is_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && [8, 4, 4, 4, 12].iter().zip(&parts).all(|(n, p)| p.len() == *n && p.chars().all(|c| c.is_ascii_hexdigit()))
}

impl Db {
    /// Returns the entry and whether it was a replay of an earlier identical request.
    pub fn create_ai_entry(&mut self, input: NewEntry, reg: AiRegistrationInput) -> Result<(TimeEntry, bool)> {
        self.ensure_writable()?;
        // Take the write lock up front when standing alone; inside an outer transaction
        // (an AI operation) the caller already holds it.
        let own = self.conn().is_autocommit();
        if own {
            self.conn().execute_batch("BEGIN IMMEDIATE")?;
        }
        let result = self.create_ai_entry_locked(input, reg);
        if own {
            let end = if result.is_ok() { "COMMIT" } else { "ROLLBACK" };
            if let Err(e) = self.conn().execute_batch(end) {
                let _ = self.conn().execute_batch("ROLLBACK");
                return Err(e.into());
            }
        }
        result
    }

    fn create_ai_entry_locked(&mut self, input: NewEntry, reg: AiRegistrationInput) -> Result<(TimeEntry, bool)> {
        if !is_uuid(&reg.request_id)
            || !matches!(reg.source.as_str(), "codex" | "claude")
            || reg.session_id.is_empty()
            || reg.session_id.chars().count() > 200
        {
            return Err(invalid("aiRegistration が不正です。"));
        }
        if input.end <= input.start {
            return Err(invalid("終了は開始より後にしてください。"));
        }
        if !is_quarter(input.start) || !is_quarter(input.end) {
            return Err(CoreError::code_msg("invalid_time_step", "開始と終了は15分単位にしてください。"));
        }
        let (start, end) = check_new_entry(&input)?;
        let hash = payload_hash(&NewEntry { start, end, ..input.clone() }, &reg.source, &reg.session_id);

        let tx = self.conn_mut().savepoint()?;
        let prior: Option<(String, Option<String>)> = tx
            .query_row(r#"SELECT payloadHash, entryId FROM "AiRegistration" WHERE requestId = ?1"#, [&reg.request_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        if let Some((prior_hash, entry_id)) = prior {
            drop(tx);
            if prior_hash != hash {
                return Err(CoreError::code_msg("request_id_conflict", "同じ要求IDで別の内容が登録されています。"));
            }
            let Some(entry_id) = entry_id else {
                return Err(CoreError::code_msg("registered_entry_deleted", "この要求で登録した工数は削除されています。"));
            };
            let entry = self.entry(&entry_id)?.ok_or_else(|| CoreError::code_msg("registered_entry_deleted", "この要求で登録した工数は削除されています。"))?;
            return Ok((entry, true));
        }
        let overlap_ids: Vec<(String, String, String, Option<String>)> = tx
            .prepare(r#"SELECT id, start, "end", title FROM "TimeEntry" WHERE start < ?1 AND "end" > ?2 ORDER BY start"#)?
            .query_map([time::to_db(end), time::to_db(start)], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<std::result::Result<_, _>>()?;
        let exact = overlap_ids.iter().any(|(_, s, e, t)| {
            time::from_db(s) == Some(start) && time::from_db(e) == Some(end) && *t == input.title
        });
        if exact || (!overlap_ids.is_empty() && !reg.allow_overlap) {
            drop(tx);
            let overlaps: Vec<TimeEntry> = overlap_ids.iter().filter_map(|(id, ..)| self.entry(id).ok().flatten()).collect();
            let code = if exact { "exact_duplicate" } else { "overlap_requires_confirmation" };
            let message = if exact { "同じ時間帯・タイトルの工数が既にあります。" } else { "既存の工数と時間が重なっています。" };
            return Err(CoreError::with_details(code, message, serde_json::json!({ "error": code, "overlaps": overlaps })));
        }
        let id = insert_entry(&tx, &input, start, end)?;
        tx.execute(
            r#"INSERT INTO "AiRegistration"(requestId, payloadHash, source, sessionId, recordedFrom, recordedUntil, entryId)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
            params![reg.request_id, hash, reg.source, reg.session_id, time::to_db(start), time::to_db(end), id],
        )?;
        tx.commit()?;
        Ok((self.entry(&id)?.expect("just created"), false))
    }

    /// Latest `recordedUntil` for a session (GET /entries/ai-progress).
    pub fn ai_progress(&self, source: &str, session_id: &str) -> Result<Option<DateTime<Utc>>> {
        let v: Option<String> = self
            .conn()
            .query_row(
                r#"SELECT recordedUntil FROM "AiRegistration" WHERE source = ?1 AND sessionId = ?2 ORDER BY recordedUntil DESC LIMIT 1"#,
                [source, session_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(v.and_then(|s| time::from_db(&s)))
    }

    /// All registered intervals for a session, oldest `recordedUntil` first (`coverage=1`).
    pub fn ai_coverage(&self, source: &str, session_id: &str) -> Result<Vec<Coverage>> {
        let mut st = self.conn().prepare(
            r#"SELECT recordedFrom, recordedUntil FROM "AiRegistration" WHERE source = ?1 AND sessionId = ?2 ORDER BY recordedUntil ASC"#,
        )?;
        let rows: Vec<(Option<String>, String)> = st.query_map([source, session_id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
        Ok(rows.into_iter().filter_map(|(f, u)| Some((f.and_then(|f| time::from_db(&f)), time::from_db(&u)?))).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NewEntry;
    use crate::repo::repo_tests::{db, local, seed};

    const RID: &str = "6f1c1f8e-2a6b-4c1d-9d3e-1a2b3c4d5e6f";

    fn input(start: &str, end: &str) -> NewEntry {
        NewEntry { start: start.parse().unwrap(), end: end.parse().unwrap(), title: Some("設計".into()), ..Default::default() }
    }

    fn reg(rid: &str, allow: bool) -> AiRegistrationInput {
        AiRegistrationInput { request_id: rid.into(), source: "codex".into(), session_id: "s1".into(), allow_overlap: allow }
    }

    #[test]
    fn payload_hash_matches_bun_server() {
        let a = input("2026-11-12T01:00:00Z", "2026-11-12T02:00:00Z");
        assert_eq!(payload_hash(&a, "codex", "s1"), "5abdabd7df1e611afb6945ba22431d5d2fd38bc320b2b1559bcc968117669b25");
        let mut b = a.clone();
        b.project_id = Some("p1".into());
        b.note = Some("メモ".into());
        b.tag_ids = vec!["b".into(), "a".into()];
        assert_eq!(payload_hash(&b, "claude", "s2"), "7d15d87f4dbd712db701751612634eebb528bf4c39383743feac980d630d37d2");
    }

    #[test]
    fn replay_same_payload_returns_existing() {
        let (_d, mut db) = db();
        let (e1, replayed1) = db.create_ai_entry(input("2026-11-12T01:00:00Z", "2026-11-12T02:00:00Z"), reg(RID, false)).unwrap();
        let (e2, replayed2) = db.create_ai_entry(input("2026-11-12T01:00:00Z", "2026-11-12T02:00:00Z"), reg(RID, false)).unwrap();
        assert!(!replayed1 && replayed2);
        assert_eq!(e1.id, e2.id);
        assert_eq!(db.entries(local(12, 0, 0), local(13, 0, 0)).unwrap().len(), 1);
    }

    #[test]
    fn replay_different_payload_conflicts() {
        let (_d, mut db) = db();
        db.create_ai_entry(input("2026-11-12T01:00:00Z", "2026-11-12T02:00:00Z"), reg(RID, false)).unwrap();
        let err = db.create_ai_entry(input("2026-11-12T01:00:00Z", "2026-11-12T02:15:00Z"), reg(RID, false)).unwrap_err();
        assert_eq!(err.code(), "request_id_conflict");
    }

    #[test]
    fn replay_after_delete_errors() {
        let (_d, mut db) = db();
        let (e, _) = db.create_ai_entry(input("2026-11-12T01:00:00Z", "2026-11-12T02:00:00Z"), reg(RID, false)).unwrap();
        db.delete_entry(&e.id).unwrap();
        let err = db.create_ai_entry(input("2026-11-12T01:00:00Z", "2026-11-12T02:00:00Z"), reg(RID, false)).unwrap_err();
        assert_eq!(err.code(), "registered_entry_deleted");
    }

    #[test]
    fn exact_duplicate_even_with_allow_overlap() {
        let (_d, mut db) = db();
        db.create_entry(input("2026-11-12T01:00:00Z", "2026-11-12T02:00:00Z")).unwrap();
        let err = db.create_ai_entry(input("2026-11-12T01:00:00Z", "2026-11-12T02:00:00Z"), reg(RID, true)).unwrap_err();
        assert_eq!(err.code(), "exact_duplicate");
        assert_eq!(err.details()["overlaps"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn overlap_requires_confirmation_unless_allowed() {
        let (_d, mut db) = db();
        db.create_entry(input("2026-11-12T01:00:00Z", "2026-11-12T02:00:00Z")).unwrap();
        let err = db.create_ai_entry(input("2026-11-12T01:30:00Z", "2026-11-12T02:30:00Z"), reg(RID, false)).unwrap_err();
        assert_eq!(err.code(), "overlap_requires_confirmation");
        assert!(db.create_ai_entry(input("2026-11-12T01:30:00Z", "2026-11-12T02:30:00Z"), reg(RID, true)).is_ok());
    }

    #[test]
    fn ai_path_rejects_unrounded_times() {
        let (_d, mut db) = db();
        let err = db.create_ai_entry(input("2026-11-12T01:05:00Z", "2026-11-12T02:00:00Z"), reg(RID, false)).unwrap_err();
        assert_eq!(err.code(), "invalid_time_step");
    }

    #[test]
    fn validates_registration_fields() {
        let (_d, mut db) = db();
        let e = input("2026-11-12T01:00:00Z", "2026-11-12T02:00:00Z");
        assert_eq!(db.create_ai_entry(e.clone(), reg("not-a-uuid", false)).unwrap_err().code(), "invalid_input");
        let mut r = reg(RID, false);
        r.source = "chatgpt".into();
        assert_eq!(db.create_ai_entry(e, r).unwrap_err().code(), "invalid_input");
    }

    #[test]
    fn coverage_and_progress() {
        let (_d, mut db) = db();
        let (_p, _r, _b) = seed(&mut db);
        db.create_ai_entry(input("2026-11-12T01:00:00Z", "2026-11-12T02:00:00Z"), reg(RID, false)).unwrap();
        db.create_ai_entry(input("2026-11-12T03:00:00Z", "2026-11-12T04:00:00Z"), reg("7f1c1f8e-2a6b-4c1d-9d3e-1a2b3c4d5e6f", false)).unwrap();
        assert_eq!(db.ai_progress("codex", "s1").unwrap(), Some("2026-11-12T04:00:00Z".parse().unwrap()));
        assert_eq!(db.ai_progress("codex", "other").unwrap(), None);
        let cov = db.ai_coverage("codex", "s1").unwrap();
        assert_eq!(cov.len(), 2);
        assert_eq!(cov[0], (Some("2026-11-12T01:00:00Z".parse().unwrap()), "2026-11-12T02:00:00Z".parse().unwrap()));
        let n: i64 = db.conn().query_row(r#"SELECT COUNT(*) FROM "AiRegistration""#, [], |r| r.get(0)).unwrap();
        assert_eq!(n, 2);
    }
}

/// Real registrations made by the Bun server (copy DB, run with --ignored): our hash must
/// reproduce every stored `payloadHash` whose entry still exists unchanged.
#[cfg(test)]
mod live_tests {
    use super::*;

    #[test]
    #[ignore]
    fn reproduces_stored_payload_hashes() {
        let db = Db::open(&dirs::home_dir().unwrap().join(".track-gpui")).unwrap();
        let rows: Vec<(String, String, String, String)> = db
            .conn()
            .prepare(r#"SELECT payloadHash, source, sessionId, entryId FROM "AiRegistration" WHERE entryId IS NOT NULL"#)
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        let (mut matched, mut total) = (0, 0);
        for (hash, source, session, entry_id) in rows {
            let Some(e) = db.entry(&entry_id).unwrap() else { continue };
            // Entries edited after registration (updatedAt moved) can't reproduce the original hash.
            let (created, updated): (String, String) = db
                .conn()
                .query_row(r#"SELECT createdAt, updatedAt FROM "TimeEntry" WHERE id = ?1"#, [&entry_id], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap();
            let edited = time::from_db(&updated).zip(time::from_db(&created)).is_some_and(|(u, c)| (u - c).num_seconds().abs() > 2);
            if edited {
                continue;
            }
            total += 1;
            let input = NewEntry {
                project_id: e.project_id.clone(),
                start: e.start,
                end: e.end,
                title: e.title.clone(),
                note: e.note.clone(),
                tag_ids: e.tags.iter().map(|t| t.tag_id.clone()).collect(),
                external_event_id: e.external_event_id.clone(),
                external_event_source: e.external_event_source.clone(),
                break_minutes: e.break_minutes,
            };
            if payload_hash(&input, &source, &session) == hash {
                matched += 1;
            } else {
                println!("mismatch {entry_id} created={:?}", e.start);
            }
        }
        println!("matched {matched}/{total}");
        assert!(total > 0 && matched == total, "matched {matched}/{total} unedited registrations");
    }
}
