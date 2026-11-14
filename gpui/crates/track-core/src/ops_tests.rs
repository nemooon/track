use crate::ai_registration::AiRegistrationInput;
use crate::model::*;
use crate::repo::repo_tests::{db, local, seed};

fn count(db: &crate::Db, sql: &str) -> i64 {
    db.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn migration_adds_ai_operation_table() {
    let (_d, db) = db();
    assert_eq!(count(&db, r#"SELECT COUNT(*) FROM "AiOperation""#), 0);
}

#[test]
fn writes_nest_inside_an_outer_transaction() {
    let (_d, mut db) = db();
    let (p, red, blue) = seed(&mut db);
    let e = db.create_entry(NewEntry { project_id: Some(p.clone()), start: local(12, 9, 0), end: local(12, 10, 0), title: Some("x".into()), ..Default::default() }).unwrap();
    let before = (count(&db, r#"SELECT COUNT(*) FROM "Client""#), count(&db, r#"SELECT COUNT(*) FROM "TimeEntry""#));

    db.conn().execute_batch("BEGIN IMMEDIATE").unwrap();
    let c = db.create_client("Beta").unwrap();
    db.create_project_with_tags(&c, "B1", "#ef4444", std::slice::from_ref(&blue)).unwrap();
    db.update_entry(&e.id, EntryPatch { title: Some(Some("changed".into())), tag_ids: Some(vec![blue.clone()]), ..Default::default() }).unwrap();
    db.update_client(&c, ClientPatch { archived: Some(true), ..Default::default() }).unwrap();
    db.delete_tag(&red).unwrap();
    db.create_ai_entry(
        NewEntry { start: local(12, 11, 0), end: local(12, 12, 0), title: Some("ai".into()), ..Default::default() },
        AiRegistrationInput { request_id: "6f1c1f8e-2a6b-4c1d-9d3e-1a2b3c4d5e6f".into(), source: "codex".into(), session_id: "s".into(), allow_overlap: false },
    )
    .unwrap();
    db.conn().execute_batch("ROLLBACK").unwrap();

    assert_eq!((count(&db, r#"SELECT COUNT(*) FROM "Client""#), count(&db, r#"SELECT COUNT(*) FROM "TimeEntry""#)), before);
    assert_eq!(db.entry(&e.id).unwrap().unwrap().title.as_deref(), Some("x"));
    assert_eq!(count(&db, r#"SELECT COUNT(*) FROM "Tag""#), 2);
    assert_eq!(count(&db, r#"SELECT COUNT(*) FROM "AiRegistration""#), 0);
}

use crate::ops::Table;

fn one_entry(db: &mut crate::Db) -> TimeEntry {
    db.create_entry(NewEntry { start: local(12, 9, 0), end: local(12, 10, 0), title: Some("旧".into()), ..Default::default() }).unwrap()
}

fn rename(db: &mut crate::Db, id: &str, title: &str) -> crate::error::Result<((), crate::ops::Operation)> {
    let id = id.to_string();
    let title = title.to_string();
    db.with_operation("codex", move |db, rec| {
        rec.watch(db, Table::TimeEntry, "id", &id)?;
        rec.watch(db, Table::TagOnEntry, "entryId", &id)?;
        db.update_entry(&id, EntryPatch { title: Some(Some(title)), ..Default::default() })?;
        Ok(((), "タイトル変更".into(), 1))
    })
}

#[test]
fn operation_records_before_and_after_rows() {
    let (_d, mut db) = db();
    let e = one_entry(&mut db);
    let (_, op) = rename(&mut db, &e.id, "新").unwrap();
    assert_eq!((op.source.as_str(), op.summary.as_str(), op.count), ("codex", "タイトル変更", 1));
    assert!(op.undone_at.is_none());
    let ops = db.operations(10).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].id, op.id);
    let changes: serde_json::Value = serde_json::from_str(&db.conn().query_row(r#"SELECT changes FROM "AiOperation""#, [], |r| r.get::<_, String>(0)).unwrap()).unwrap();
    let te = changes.as_array().unwrap().iter().find(|c| c["table"] == "TimeEntry").unwrap();
    assert_eq!(te["before"][0]["title"], "旧");
    assert_eq!(te["after"][0]["title"], "新");
}

#[test]
fn failed_operation_changes_nothing_and_records_nothing() {
    let (_d, mut db) = db();
    let e = one_entry(&mut db);
    let id = e.id.clone();
    let r: crate::error::Result<((), crate::ops::Operation)> = db.with_operation("codex", move |db, rec| {
        rec.watch(db, Table::TimeEntry, "id", &id)?;
        db.update_entry(&id, EntryPatch { title: Some(Some("新".into())), ..Default::default() })?;
        Err(crate::CoreError::code_msg("not_found", "x"))
    });
    assert!(r.is_err());
    assert_eq!(db.entry(&e.id).unwrap().unwrap().title.as_deref(), Some("旧"));
    assert!(db.operations(10).unwrap().is_empty());
    assert!(db.conn().is_autocommit(), "transaction closed");
}

#[test]
fn operations_are_pruned_to_100_and_30_days() {
    let (_d, mut db) = db();
    let e = one_entry(&mut db);
    for i in 0..101 {
        rename(&mut db, &e.id, &format!("t{i}")).unwrap();
    }
    assert_eq!(db.operations(1000).unwrap().len(), 100);
    let old = db.operations(1000).unwrap().last().unwrap().id;
    db.conn().execute(r#"UPDATE "AiOperation" SET createdAt = ?1 WHERE id = ?2"#, rusqlite::params![crate::time::to_db(chrono::Utc::now() - chrono::Duration::days(31)), old]).unwrap();
    rename(&mut db, &e.id, "last").unwrap();
    let ops = db.operations(1000).unwrap();
    assert_eq!(ops.len(), 100);
    assert!(ops.iter().all(|o| o.id != old));
}

#[test]
fn operations_after_returns_newer_only() {
    let (_d, mut db) = db();
    let e = one_entry(&mut db);
    assert_eq!(db.latest_operation_id().unwrap(), 0);
    let (_, a) = rename(&mut db, &e.id, "a").unwrap();
    let (_, b) = rename(&mut db, &e.id, "b").unwrap();
    assert_eq!(db.latest_operation_id().unwrap(), b.id);
    let after: Vec<i64> = db.operations_after(a.id).unwrap().iter().map(|o| o.id).collect();
    assert_eq!(after, vec![b.id]);
}

fn code<T: std::fmt::Debug>(r: crate::error::Result<T>) -> String {
    r.unwrap_err().code().to_string()
}

#[test]
fn undo_restores_updated_entry_and_tags() {
    let (_d, mut db) = db();
    let (_p, red, blue) = seed(&mut db);
    let e = db.create_entry(NewEntry { start: local(12, 9, 0), end: local(12, 10, 0), title: Some("旧".into()), tag_ids: vec![red.clone()], ..Default::default() }).unwrap();
    let id = e.id.clone();
    let (_, op) = db
        .with_operation("codex", |db, rec| {
            rec.watch(db, Table::TimeEntry, "id", &id)?;
            rec.watch(db, Table::TagOnEntry, "entryId", &id)?;
            db.update_entry(&id, EntryPatch { title: Some(Some("新".into())), tag_ids: Some(vec![blue.clone()]), ..Default::default() })?;
            Ok(((), "x".into(), 1))
        })
        .unwrap();
    let r = db.undo_operation(None).unwrap();
    assert_eq!(r.operation.id, op.id);
    let back = db.entry(&e.id).unwrap().unwrap();
    assert_eq!(back.title.as_deref(), Some("旧"));
    assert_eq!(back.tags.iter().map(|t| t.tag_id.clone()).collect::<Vec<_>>(), vec![red]);
    assert!(db.operations(10).unwrap()[0].undone_at.is_some());
}

fn ai_create(db: &mut crate::Db) -> (TimeEntry, crate::ops::Operation) {
    let ((e, _), op) = db
        .with_operation("codex", |db, rec| {
            let (e, replay) = db.create_ai_entry(
                NewEntry { start: local(12, 11, 0), end: local(12, 12, 0), title: Some("ai".into()), ..Default::default() },
                AiRegistrationInput { request_id: "6f1c1f8e-2a6b-4c1d-9d3e-1a2b3c4d5e6f".into(), source: "codex".into(), session_id: "s".into(), allow_overlap: false },
            )?;
            rec.created(Table::TimeEntry, "id", &e.id);
            rec.created(Table::TagOnEntry, "entryId", &e.id);
            rec.created(Table::AiRegistration, "requestId", "6f1c1f8e-2a6b-4c1d-9d3e-1a2b3c4d5e6f");
            Ok(((e, replay), "記録".into(), 1))
        })
        .unwrap();
    (e, op)
}

#[test]
fn undo_of_ai_create_removes_entry_and_registration() {
    let (_d, mut db) = db();
    let (e, _) = ai_create(&mut db);
    assert_eq!(db.ai_coverage("codex", "s").unwrap().len(), 1);
    db.undo_operation(None).unwrap();
    assert!(db.entry(&e.id).unwrap().is_none());
    assert!(db.ai_coverage("codex", "s").unwrap().is_empty());
}

#[test]
fn undo_of_tag_delete_restores_links() {
    let (_d, mut db) = db();
    let (p, red, _) = seed(&mut db);
    let e = db.create_entry(NewEntry { start: local(12, 9, 0), end: local(12, 10, 0), tag_ids: vec![red.clone()], ..Default::default() }).unwrap();
    let tag = red.clone();
    db.with_operation("codex", |db, rec| {
        rec.watch(db, Table::Tag, "id", &tag)?;
        rec.watch(db, Table::TagOnEntry, "tagId", &tag)?;
        rec.watch(db, Table::TagOnProject, "tagId", &tag)?;
        db.delete_tag(&tag)?;
        Ok(((), "タグ削除".into(), 1))
    })
    .unwrap();
    assert!(db.entry(&e.id).unwrap().unwrap().tags.is_empty());
    db.undo_operation(None).unwrap();
    assert_eq!(db.entry(&e.id).unwrap().unwrap().tags[0].tag_id, red);
    assert_eq!(db.project(&p).unwrap().tags.iter().map(|t| t.tag_id.clone()).collect::<Vec<_>>(), vec![red]);
}

#[test]
fn undo_conflicts_when_row_changed_later() {
    let (_d, mut db) = db();
    let e = one_entry(&mut db);
    rename(&mut db, &e.id, "AI").unwrap();
    db.update_entry(&e.id, EntryPatch { title: Some(Some("手修正".into())), ..Default::default() }).unwrap();
    let err = db.undo_operation(None).unwrap_err();
    assert_eq!(err.code(), "undo_conflict");
    assert_eq!(err.details()["changed"][0]["table"], "TimeEntry");
    assert_eq!(err.details()["changed"][0]["key"], e.id.as_str());
    assert_eq!(db.entry(&e.id).unwrap().unwrap().title.as_deref(), Some("手修正"));
    assert!(db.operations(10).unwrap()[0].undone_at.is_none());
}

#[test]
fn undo_twice_is_rejected() {
    let (_d, mut db) = db();
    let e = one_entry(&mut db);
    let (_, op) = rename(&mut db, &e.id, "AI").unwrap();
    db.undo_operation(Some(op.id)).unwrap();
    assert_eq!(code(db.undo_operation(Some(op.id))), "already_undone");
    assert_eq!(code(db.undo_operation(Some(op.id + 100))), "not_found");
}

#[test]
fn undo_without_id_takes_latest_not_undone() {
    let (_d, mut db) = db();
    let e = one_entry(&mut db);
    let (_, a) = rename(&mut db, &e.id, "a").unwrap();
    let (_, b) = rename(&mut db, &e.id, "b").unwrap();
    assert_eq!(db.undo_operation(None).unwrap().operation.id, b.id);
    assert_eq!(db.undo_operation(None).unwrap().operation.id, a.id);
    assert_eq!(db.entry(&e.id).unwrap().unwrap().title.as_deref(), Some("旧"));
    assert_eq!(code(db.undo_operation(None)), "nothing_to_undo");
}

#[test]
fn an_operation_recorded_before_the_kind_column_still_undoes() {
    let (_d, mut db) = db();
    let e = one_entry(&mut db);
    rename(&mut db, &e.id, "新").unwrap();
    // Recorded before migration 0017: the rows had no "kind".
    let raw: String = db.conn().query_row(r#"SELECT changes FROM "AiOperation""#, [], |r| r.get(0)).unwrap();
    let mut changes: serde_json::Value = serde_json::from_str(&raw).unwrap();
    for c in changes.as_array_mut().unwrap() {
        for side in ["before", "after"] {
            for row in c[side].as_array_mut().unwrap() {
                row.as_object_mut().unwrap().remove("kind");
            }
        }
    }
    db.conn().execute(r#"UPDATE "AiOperation" SET changes = ?1"#, [changes.to_string()]).unwrap();
    db.undo_operation(None).unwrap();
    assert_eq!(db.entry(&e.id).unwrap().unwrap().title.as_deref(), Some("旧"));
    assert_eq!(db.entry(&e.id).unwrap().unwrap().kind, crate::model::EntryKind::Normal);
}
