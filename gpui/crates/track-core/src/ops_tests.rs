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
