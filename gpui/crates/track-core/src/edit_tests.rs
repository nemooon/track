use crate::ai_registration::AiRegistrationInput;
use crate::edit::EntrySearch;
use crate::model::*;
use crate::repo::repo_tests::{db, local, seed};
use crate::Db;

fn entry(db: &mut Db, project: Option<&str>, h: u32, title: &str) -> TimeEntry {
    db.create_entry(NewEntry { project_id: project.map(String::from), start: local(12, h, 0), end: local(12, h + 1, 0), title: Some(title.into()), ..Default::default() }).unwrap()
}

fn code<T: std::fmt::Debug>(r: crate::error::Result<T>) -> String {
    r.unwrap_err().code().to_string()
}

#[test]
fn edit_entries_moves_all_to_another_project_and_undo_restores() {
    let (_d, mut db) = db();
    let (p, _, _) = seed(&mut db);
    let c = db.projects(true).unwrap()[0].client_id.clone();
    let q = db.create_project(&c, "Beta", "#ef4444").unwrap();
    let ids: Vec<String> = (8..11).map(|h| entry(&mut db, Some(&p), h, "作業").id).collect();
    let (out, op) = db.edit_entries("codex", &ids, EntryPatch { project_id: Some(Some(q.clone())), ..Default::default() }).unwrap();
    assert_eq!(out.len(), 3);
    assert_eq!(op.count, 3);
    assert!(ids.iter().all(|id| db.entry(id).unwrap().unwrap().project_id.as_deref() == Some(q.as_str())));
    db.undo_operation(Some(op.id)).unwrap();
    assert!(ids.iter().all(|id| db.entry(id).unwrap().unwrap().project_id.as_deref() == Some(p.as_str())));
}

#[test]
fn edit_entries_with_unknown_id_changes_nothing() {
    let (_d, mut db) = db();
    let a = entry(&mut db, None, 8, "a").id;
    let b = entry(&mut db, None, 9, "b").id;
    let r = db.edit_entries("codex", &[a.clone(), b.clone(), "missing".into()], EntryPatch { title: Some(Some("x".into())), ..Default::default() });
    assert_eq!(code(r), "not_found");
    assert_eq!(db.entry(&a).unwrap().unwrap().title.as_deref(), Some("a"));
    assert_eq!(db.entry(&b).unwrap().unwrap().title.as_deref(), Some("b"));
    assert!(db.operations(10).unwrap().is_empty());
}

#[test]
fn edit_entries_rejects_time_change_for_many_and_empty_ids() {
    let (_d, mut db) = db();
    let a = entry(&mut db, None, 8, "a").id;
    let b = entry(&mut db, None, 9, "b").id;
    let patch = EntryPatch { start: Some(local(12, 13, 0)), end: Some(local(12, 14, 0)), ..Default::default() };
    assert_eq!(code(db.edit_entries("codex", &[a.clone(), b], patch.clone())), "invalid_input");
    assert_eq!(code(db.edit_entries("codex", &[], EntryPatch::default())), "invalid_input");
    let (out, _) = db.edit_entries("codex", &[a.clone(), a], patch).unwrap();
    assert_eq!(out.len(), 1, "duplicate ids collapse");
}

#[test]
fn remove_entries_and_undo_restores_registration_link() {
    let (_d, mut db) = db();
    let rid = "6f1c1f8e-2a6b-4c1d-9d3e-1a2b3c4d5e6f";
    let (e, replay, op) = db
        .record_ai_entry(
            "codex",
            NewEntry { start: local(12, 11, 0), end: local(12, 12, 0), title: Some("ai".into()), ..Default::default() },
            AiRegistrationInput { request_id: rid.into(), source: "codex".into(), session_id: "s".into(), allow_overlap: false },
        )
        .unwrap();
    assert!(!replay && op.is_some());
    let (_, del) = db.remove_entries("codex", std::slice::from_ref(&e.id)).unwrap();
    assert!(db.entry(&e.id).unwrap().is_none());
    db.undo_operation(Some(del.id)).unwrap();
    assert!(db.entry(&e.id).unwrap().is_some());
    let linked: Option<String> = db.conn().query_row(r#"SELECT entryId FROM "AiRegistration" WHERE requestId = ?1"#, [rid], |r| r.get(0)).unwrap();
    assert_eq!(linked.as_deref(), Some(e.id.as_str()));
}

#[test]
fn record_ai_entry_replay_records_no_operation() {
    let (_d, mut db) = db();
    let input = NewEntry { start: local(12, 11, 0), end: local(12, 12, 0), title: Some("ai".into()), ..Default::default() };
    let reg = AiRegistrationInput { request_id: "6f1c1f8e-2a6b-4c1d-9d3e-1a2b3c4d5e6f".into(), source: "codex".into(), session_id: "s".into(), allow_overlap: false };
    db.record_ai_entry("codex", input.clone(), reg.clone()).unwrap();
    let (_, replay, op) = db.record_ai_entry("codex", input, reg).unwrap();
    assert!(replay && op.is_none());
    assert_eq!(db.operations(10).unwrap().len(), 1);
}

#[test]
fn archiving_client_archives_projects_and_undo_unarchives_them() {
    let (_d, mut db) = db();
    let (p, _, _) = seed(&mut db);
    let c = db.projects(true).unwrap()[0].client_id.clone();
    let (client, op) = db.op_update_client("codex", &c, ClientPatch { archived: Some(true), ..Default::default() }).unwrap();
    assert!(client.archived);
    assert!(db.project(&p).unwrap().archived);
    db.undo_operation(Some(op.id)).unwrap();
    assert!(!db.client(&c).unwrap().archived);
    assert!(!db.project(&p).unwrap().archived);
}

#[test]
fn search_by_project_and_literal_query() {
    let (_d, mut db) = db();
    let (p, _, _) = seed(&mut db);
    entry(&mut db, Some(&p), 8, "100%完了");
    entry(&mut db, Some(&p), 9, "1000完了");
    entry(&mut db, None, 10, "a_b");
    entry(&mut db, None, 11, "axb");
    let titles = |db: &Db, q: EntrySearch| db.search_entries(&q, 100).unwrap().into_iter().map(|e| e.title.unwrap()).collect::<Vec<_>>();
    assert_eq!(titles(&db, EntrySearch { query: Some("100%".into()), ..Default::default() }), vec!["100%完了"]);
    assert_eq!(titles(&db, EntrySearch { query: Some("a_b".into()), ..Default::default() }), vec!["a_b"]);
    assert_eq!(titles(&db, EntrySearch { project_id: Some("none".into()), ..Default::default() }), vec!["axb", "a_b"]);
    assert_eq!(titles(&db, EntrySearch { project_id: Some(p), ..Default::default() }), vec!["1000完了", "100%完了"]);
    assert_eq!(titles(&db, EntrySearch { from: Some(local(12, 10, 30)), ..Default::default() }), vec!["axb", "a_b"]);
}

#[test]
fn create_project_records_operation_and_undo_removes_it() {
    let (_d, mut db) = db();
    let (_, red, _) = seed(&mut db);
    let (c, _) = db.op_create_client("codex", "Newco").unwrap();
    let (p, op) = db.op_create_project("codex", &c.id, "N1", "#22c55e", std::slice::from_ref(&red)).unwrap();
    assert_eq!(p.tags.len(), 1);
    db.undo_operation(Some(op.id)).unwrap();
    assert_eq!(code(db.project(&p.id)), "not_found");
    db.undo_operation(None).unwrap();
    assert_eq!(code(db.client(&c.id)), "not_found");
}

#[test]
fn tag_operations_round_trip() {
    let (_d, mut db) = db();
    let (t, _) = db.op_create_tag("codex", "green", "#22c55e").unwrap();
    let (t2, _) = db.op_update_tag("codex", &t.id, TagPatch { name: Some("緑".into()), ..Default::default() }).unwrap();
    assert_eq!(t2.name, "緑");
    db.op_delete_tag("codex", &t.id).unwrap();
    assert!(db.tags().unwrap().iter().all(|x| x.id != t.id));
    db.undo_operation(None).unwrap();
    assert_eq!(db.tags().unwrap().iter().find(|x| x.id == t.id).unwrap().name, "緑");
    let (c, _) = db.op_create_client("codex", "C").unwrap();
    let (p, _) = db.op_create_project("codex", &c.id, "P", "#ef4444", &[]).unwrap();
    let (p, _) = db.op_update_project("codex", &p.id, ProjectPatch { color: Some("#3b82f6".into()), ..Default::default() }).unwrap();
    assert_eq!(p.color, "#3b82f6");
}

#[test]
fn undo_of_tag_create_conflicts_when_tag_was_used_later() {
    let (_d, mut db) = db();
    let (t, op) = db.op_create_tag("codex", "green", "#22c55e").unwrap();
    let e = entry(&mut db, None, 8, "a");
    db.update_entry(&e.id, EntryPatch { tag_ids: Some(vec![t.id.clone()]), ..Default::default() }).unwrap();
    let err = db.undo_operation(Some(op.id)).unwrap_err();
    assert_eq!(err.code(), "undo_conflict");
    assert_eq!(err.details()["changed"][0]["table"], "TagOnEntry");
    assert_eq!(db.entry(&e.id).unwrap().unwrap().tags.len(), 1, "the later tagging survives");
}

#[test]
fn undo_of_client_create_conflicts_when_projects_were_added() {
    let (_d, mut db) = db();
    let (c, op) = db.op_create_client("codex", "Newco").unwrap();
    let p = db.create_project(&c.id, "手で追加", "#ef4444").unwrap();
    let err = db.undo_operation(Some(op.id)).unwrap_err();
    assert_eq!(err.code(), "undo_conflict");
    assert_eq!(err.details()["changed"][0]["table"], "Project");
    assert!(db.project(&p).is_ok(), "the project is not cascade-deleted");
}

#[test]
fn undo_of_project_create_conflicts_when_entries_moved_onto_it() {
    let (_d, mut db) = db();
    let (c, _) = db.op_create_client("codex", "Newco").unwrap();
    let (p, op) = db.op_create_project("codex", &c.id, "N1", "#22c55e", &[]).unwrap();
    entry(&mut db, Some(&p.id), 8, "a");
    let err = db.undo_operation(Some(op.id)).unwrap_err();
    assert_eq!(err.code(), "undo_conflict", "not a raw foreign-key error");
    assert_eq!(err.details()["changed"][0]["table"], "TimeEntry");
}

#[test]
fn undo_of_tag_delete_conflicts_when_a_tagged_entry_is_gone() {
    let (_d, mut db) = db();
    let (t, _) = db.op_create_tag("codex", "green", "#22c55e").unwrap();
    let e = db.create_entry(NewEntry { start: local(12, 9, 0), end: local(12, 10, 0), tag_ids: vec![t.id.clone()], ..Default::default() }).unwrap();
    let (_, op) = db.op_delete_tag("codex", &t.id).unwrap();
    db.delete_entry(&e.id).unwrap();
    let err = db.undo_operation(Some(op.id)).unwrap_err();
    assert_eq!(err.code(), "undo_conflict", "not a raw foreign-key error");
    assert_eq!(err.details()["changed"][0]["table"], "TimeEntry");
    assert_eq!(err.details()["changed"][0]["key"], e.id.as_str());
    assert!(db.tags().unwrap().is_empty(), "nothing restored");
}

#[test]
fn project_colours_are_twenty_distinct_in_hue_order() {
    use crate::edit::PROJECT_COLORS;
    assert_eq!(PROJECT_COLORS.len(), 20);
    let mut seen = std::collections::HashSet::new();
    for c in PROJECT_COLORS {
        assert!(c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit()), "{c}");
        assert!(seen.insert(c.to_lowercase()), "duplicate {c}");
    }
    // Hue order (red → rose), the two greys last.
    let hue = |c: &str| {
        let v: Vec<f32> = (0..3).map(|i| u8::from_str_radix(&c[1 + 2 * i..3 + 2 * i], 16).unwrap() as f32 / 255.).collect();
        let (max, min) = (v.iter().cloned().fold(0., f32::max), v.iter().cloned().fold(1., f32::min));
        let d = max - min;
        let h = if d == 0. { 0. } else if max == v[0] { ((v[1] - v[2]) / d).rem_euclid(6.) } else if max == v[1] { (v[2] - v[0]) / d + 2. } else { (v[0] - v[1]) / d + 4. };
        (h * 60., if max == 0. { 0. } else { d / max })
    };
    let chroma: Vec<f32> = PROJECT_COLORS[..18].iter().map(|c| hue(c).0).collect();
    assert!(chroma.windows(2).all(|w| w[0] <= w[1]), "hue order: {chroma:?}");
    assert!(PROJECT_COLORS[18..].iter().all(|c| hue(c).1 < 0.3), "greys last");
}
