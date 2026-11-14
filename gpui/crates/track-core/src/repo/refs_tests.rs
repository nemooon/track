use crate::model::*;
use crate::repo::repo_tests::{db, local, seed};
use crate::Db;

fn entry(db: &mut Db, project: &str) -> TimeEntry {
    db.create_entry(NewEntry { project_id: Some(project.into()), start: local(12, 9, 0), end: local(12, 10, 0), title: Some("x".into()), ..Default::default() }).unwrap()
}

fn code<T: std::fmt::Debug>(r: crate::error::Result<T>) -> String {
    r.unwrap_err().code().to_string()
}

#[test]
fn archiving_client_archives_its_projects() {
    let (_d, mut db) = db();
    let (p, _, _) = seed(&mut db);
    let client = db.projects(true).unwrap()[0].client_id.clone();
    let c = db.update_client(&client, ClientPatch { archived: Some(true), ..Default::default() }).unwrap();
    assert!(c.archived);
    assert!(db.projects(true).unwrap().iter().find(|x| x.id == p).unwrap().archived);
    assert!(db.projects(false).unwrap().is_empty());
    // Un-archiving the client leaves the projects archived (like the server).
    db.update_client(&client, ClientPatch { archived: Some(false), ..Default::default() }).unwrap();
    assert!(db.projects(false).unwrap().is_empty());
    assert_eq!(db.update_client(&client, ClientPatch { name: Some("新名".into()), ..Default::default() }).unwrap().name, "新名");
    assert_eq!(db.clients().unwrap().len(), 1);
}

#[test]
fn project_with_entries_cannot_be_deleted() {
    let (_d, mut db) = db();
    let (p, _, _) = seed(&mut db);
    let e = entry(&mut db, &p);
    assert_eq!(code(db.delete_project(&p)), "project_has_entries");
    db.delete_entry(&e.id).unwrap();
    db.delete_project(&p).unwrap();
    assert!(db.projects(true).unwrap().is_empty());
}

#[test]
fn duplicate_tag_name_is_rejected() {
    let (_d, mut db) = db();
    let (_, red, blue) = seed(&mut db); // tags "red" and "blue"
    assert_eq!(code(db.create_tag("red", "#000000")), "tag_already_exists");
    assert_eq!(code(db.update_tag(&blue, TagPatch { name: Some("red".into()), ..Default::default() })), "tag_already_exists");
    assert_eq!(db.update_tag(&red, TagPatch { color: Some("#ABCDEF".into()), ..Default::default() }).unwrap().color, "#ABCDEF");
}

#[test]
fn invalid_color_and_empty_name_are_rejected() {
    let (_d, mut db) = db();
    let (p, _, _) = seed(&mut db);
    let client = db.projects(true).unwrap()[0].client_id.clone();
    assert_eq!(code(db.create_client("")), "invalid_input");
    assert_eq!(code(db.create_client(&"あ".repeat(101))), "invalid_input");
    assert!(db.create_client(&"あ".repeat(100)).is_ok());
    assert_eq!(code(db.create_project(&client, "P", "#12345")), "invalid_input");
    assert_eq!(code(db.update_project(&p, ProjectPatch { color: Some("red".into()), ..Default::default() })), "invalid_input");
    assert_eq!(code(db.create_tag(&"t".repeat(51), "#000000")), "invalid_input");
    assert_eq!(code(db.update_client(&client, ClientPatch { name: Some(String::new()), ..Default::default() })), "invalid_input");
}

#[test]
fn update_unknown_id_is_not_found() {
    let (_d, mut db) = db();
    seed(&mut db);
    assert_eq!(code(db.update_client("nope", ClientPatch::default())), "not_found");
    assert_eq!(code(db.update_project("nope", ProjectPatch::default())), "not_found");
    assert_eq!(code(db.update_tag("nope", TagPatch::default())), "not_found");
    assert_eq!(code(db.delete_client("nope")), "not_found");
    assert_eq!(code(db.delete_project("nope")), "not_found");
    assert_eq!(code(db.delete_tag("nope")), "not_found");
}

#[test]
fn project_tag_ids_replace_existing() {
    let (_d, mut db) = db();
    let (p, _red, blue) = seed(&mut db); // Alpha carries red
    let updated = db.update_project(&p, ProjectPatch { tag_ids: Some(vec![blue.clone()]), ..Default::default() }).unwrap();
    assert_eq!(updated.tags.iter().map(|t| t.tag_id.clone()).collect::<Vec<_>>(), vec![blue.clone()]);
    let client = updated.client_id.clone();
    let id = db.create_project_with_tags(&client, "Beta", "#111111", std::slice::from_ref(&blue)).unwrap();
    assert_eq!(db.projects(true).unwrap().iter().find(|x| x.id == id).unwrap().tags.len(), 1);
    let cleared = db.update_project(&p, ProjectPatch { tag_ids: Some(vec![]), name: Some("Alpha2".into()), ..Default::default() }).unwrap();
    assert!(cleared.tags.is_empty());
    assert_eq!(cleared.name, "Alpha2");
}

#[test]
fn deleting_client_cascades_like_prisma() {
    let (_d, mut db) = db();
    let (p, _, _) = seed(&mut db);
    let client = db.projects(true).unwrap()[0].client_id.clone();
    let e = entry(&mut db, &p);
    // Entries restrict the cascade (TimeEntry.projectId ON DELETE RESTRICT).
    assert_eq!(code(db.delete_client(&client)), "client_has_entries");
    db.delete_entry(&e.id).unwrap();
    db.delete_client(&client).unwrap();
    assert!(db.projects(true).unwrap().is_empty());
    assert!(db.clients().unwrap().is_empty());
    // Tag deletion cascades to TagOnProject.
    let c2 = db.create_client("C2").unwrap();
    let green = db.create_tag("green", "#00ff00").unwrap();
    let p2 = db.create_project_with_tags(&c2, "P2", "#000000", std::slice::from_ref(&green)).unwrap();
    db.delete_tag(&green).unwrap();
    assert!(db.projects(true).unwrap().iter().find(|x| x.id == p2).unwrap().tags.is_empty());
}

#[test]
fn writes_refused_when_schema_too_new() {
    let (d, mut db) = db();
    let (p, _, _) = seed(&mut db);
    db.conn().execute(r#"INSERT INTO "_local_migrations"(name) VALUES ('9999_future.sql')"#, []).unwrap();
    drop(db);
    let mut db = Db::open(d.path()).unwrap();
    assert_eq!(code(db.update_project(&p, ProjectPatch { name: Some("x".into()), ..Default::default() })), "schema_too_new");
    assert_eq!(code(db.delete_tag("whatever")), "schema_too_new");
}

#[test]
fn projects_with_entries_lists_only_projects_that_have_some() {
    let (_d, mut db) = db();
    let (p1, _, _) = seed(&mut db);
    let c = db.projects(true).unwrap()[0].client_id.clone();
    let _p2 = db.create_project(&c, "Empty", "#ef4444").unwrap();
    entry(&mut db, &p1);
    entry(&mut db, &p1);
    assert_eq!(db.projects_with_entries().unwrap(), vec![p1]);
}
