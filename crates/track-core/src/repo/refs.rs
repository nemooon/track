//! Clients, projects and tags.
use crate::error::{CoreError, Result};
use crate::model::{Client, ClientPatch, Project, ProjectPatch, Tag, TagLink, TagPatch};
use crate::{Db, ids, time};
use rusqlite::params;

impl Db {
    pub fn clients(&self) -> Result<Vec<Client>> {
        let mut st = self.conn().prepare(r#"SELECT id, name, archived FROM "Client" ORDER BY createdAt ASC, id ASC"#)?;
        let rows = st.query_map([], |r| Ok(Client { id: r.get(0)?, name: r.get(1)?, archived: r.get::<_, i64>(2)? != 0 }))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn tags(&self) -> Result<Vec<Tag>> {
        let mut st = self.conn().prepare(r#"SELECT id, name, color FROM "Tag" ORDER BY name ASC"#)?;
        let rows = st.query_map([], |r| Ok(Tag { id: r.get(0)?, name: r.get(1)?, color: r.get(2)? }))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Archived projects are excluded unless requested (GET /api/projects).
    pub fn projects(&self, include_archived: bool) -> Result<Vec<Project>> {
        let sql = format!(
            r#"SELECT p.id, p.clientId, p.name, p.color, p.archived, c.id, c.name, c.archived
               FROM "Project" p JOIN "Client" c ON c.id = p.clientId
               {} ORDER BY p.createdAt ASC, p.id ASC"#,
            if include_archived { "" } else { "WHERE p.archived = 0" }
        );
        let mut st = self.conn().prepare(&sql)?;
        let mut projects: Vec<Project> = st
            .query_map([], |r| {
                Ok(Project {
                    id: r.get(0)?,
                    client_id: r.get(1)?,
                    name: r.get(2)?,
                    color: r.get(3)?,
                    archived: r.get::<_, i64>(4)? != 0,
                    client: Client { id: r.get(5)?, name: r.get(6)?, archived: r.get::<_, i64>(7)? != 0 },
                    tags: vec![],
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        for p in &mut projects {
            p.tags = self.project_tags(&p.id)?;
        }
        Ok(projects)
    }

    pub(crate) fn project_tags(&self, project_id: &str) -> Result<Vec<TagLink>> {
        let mut st = self.conn().prepare(
            r#"SELECT t.id, t.name, t.color FROM "TagOnProject" tp JOIN "Tag" t ON t.id = tp.tagId WHERE tp.projectId = ?1 ORDER BY t.name"#,
        )?;
        let rows = st.query_map([project_id], |r| {
            let tag = Tag { id: r.get(0)?, name: r.get(1)?, color: r.get(2)? };
            Ok(TagLink { tag_id: tag.id.clone(), tag })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn create_client(&mut self, name: &str) -> Result<String> {
        self.ensure_writable()?;
        check_name(name, 100)?;
        let id = ids::cuid();
        self.conn().execute(r#"INSERT INTO "Client"(id, name, createdAt) VALUES (?1, ?2, ?3)"#, params![id, name, time::now_db()])?;
        Ok(id)
    }

    pub fn create_project(&mut self, client_id: &str, name: &str, color: &str) -> Result<String> {
        self.create_project_with_tags(client_id, name, color, &[])
    }

    pub fn create_project_with_tags(&mut self, client_id: &str, name: &str, color: &str, tag_ids: &[String]) -> Result<String> {
        self.ensure_writable()?;
        check_name(name, 100)?;
        check_color(color)?;
        let id = ids::cuid();
        let tx = self.conn_mut().savepoint()?;
        tx.execute(
            r#"INSERT INTO "Project"(id, clientId, name, color, createdAt) VALUES (?1, ?2, ?3, ?4, ?5)"#,
            params![id, client_id, name, color, time::now_db()],
        )?;
        for t in tag_ids {
            tx.execute(r#"INSERT INTO "TagOnProject"(id, tagId, projectId) VALUES (?1, ?2, ?3)"#, params![ids::cuid(), t, id])?;
        }
        tx.commit()?;
        Ok(id)
    }

    /// Every tag with the number of projects it is on and entries that carry it.
    pub fn tag_usage(&self) -> Result<Vec<crate::model::TagUsage>> {
        let mut st = self.conn().prepare(
            r#"SELECT t.id,
                      (SELECT COUNT(*) FROM "TagOnProject" tp WHERE tp.tagId = t.id),
                      (SELECT COUNT(*) FROM "TagOnEntry" te WHERE te.tagId = t.id)
               FROM "Tag" t ORDER BY t.name"#,
        )?;
        let rows = st.query_map([], |r| Ok(crate::model::TagUsage { tag_id: r.get(0)?, projects: r.get(1)?, entries: r.get(2)? }))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn create_tag(&mut self, name: &str, color: &str) -> Result<String> {
        self.ensure_writable()?;
        check_name(name, 50)?;
        check_color(color)?;
        if self.tag_name_taken(name, None)? {
            return Err(CoreError::code_msg("tag_already_exists", "同じ名前のタグがあります。"));
        }
        let id = ids::cuid();
        self.conn().execute(r#"INSERT INTO "Tag"(id, name, color, createdAt) VALUES (?1, ?2, ?3, ?4)"#, params![id, name, color, time::now_db()])?;
        Ok(id)
    }

    fn exists(&self, table: &str, id: &str) -> Result<()> {
        let found: Option<i64> = self.conn().query_row(&format!(r#"SELECT 1 FROM "{table}" WHERE id = ?1"#), [id], |r| r.get(0)).ok();
        if found.is_some() { Ok(()) } else { Err(not_found()) }
    }

    fn tag_name_taken(&self, name: &str, except: Option<&str>) -> Result<bool> {
        let n: i64 = self.conn().query_row(r#"SELECT COUNT(*) FROM "Tag" WHERE name = ?1 AND id != ?2"#, params![name, except.unwrap_or("")], |r| r.get(0))?;
        Ok(n > 0)
    }

    pub fn client(&self, id: &str) -> Result<Client> {
        self.conn()
            .query_row(r#"SELECT id, name, archived FROM "Client" WHERE id = ?1"#, [id], |r| Ok(Client { id: r.get(0)?, name: r.get(1)?, archived: r.get::<_, i64>(2)? != 0 }))
            .map_err(|_| not_found())
    }

    /// Archiving a client archives its projects too (one transaction), like the server.
    pub fn update_client(&mut self, id: &str, patch: ClientPatch) -> Result<Client> {
        self.ensure_writable()?;
        if let Some(n) = &patch.name {
            check_name(n, 100)?;
        }
        self.exists("Client", id)?;
        let tx = self.conn_mut().savepoint()?;
        if let Some(n) = &patch.name {
            tx.execute(r#"UPDATE "Client" SET name = ?1 WHERE id = ?2"#, params![n, id])?;
        }
        if let Some(a) = patch.archived {
            tx.execute(r#"UPDATE "Client" SET archived = ?1 WHERE id = ?2"#, params![a as i64, id])?;
            if a {
                tx.execute(r#"UPDATE "Project" SET archived = 1 WHERE clientId = ?1"#, [id])?;
            }
        }
        tx.commit()?;
        self.client(id)
    }

    /// Projects cascade; their entries restrict the delete (TimeEntry.projectId ON DELETE RESTRICT).
    pub fn delete_client(&mut self, id: &str) -> Result<()> {
        self.ensure_writable()?;
        self.exists("Client", id)?;
        let entries: i64 = self.conn().query_row(
            r#"SELECT COUNT(*) FROM "TimeEntry" e JOIN "Project" p ON p.id = e.projectId WHERE p.clientId = ?1"#,
            [id],
            |r| r.get(0),
        )?;
        if entries > 0 {
            return Err(CoreError::code_msg("client_has_entries", "工数が登録されたプロジェクトがあるため削除できません。"));
        }
        self.conn().execute(r#"DELETE FROM "Client" WHERE id = ?1"#, [id])?;
        Ok(())
    }

    pub fn project(&self, id: &str) -> Result<Project> {
        self.projects(true)?.into_iter().find(|p| p.id == id).ok_or_else(not_found)
    }

    pub fn update_project(&mut self, id: &str, patch: ProjectPatch) -> Result<Project> {
        self.ensure_writable()?;
        if let Some(n) = &patch.name {
            check_name(n, 100)?;
        }
        if let Some(c) = &patch.color {
            check_color(c)?;
        }
        if patch.client_id.as_deref() == Some("") {
            return Err(invalid());
        }
        self.exists("Project", id)?;
        let tx = self.conn_mut().savepoint()?;
        if let Some(v) = &patch.client_id {
            tx.execute(r#"UPDATE "Project" SET clientId = ?1 WHERE id = ?2"#, params![v, id])?;
        }
        if let Some(v) = &patch.name {
            tx.execute(r#"UPDATE "Project" SET name = ?1 WHERE id = ?2"#, params![v, id])?;
        }
        if let Some(v) = &patch.color {
            tx.execute(r#"UPDATE "Project" SET color = ?1 WHERE id = ?2"#, params![v, id])?;
        }
        if let Some(v) = patch.archived {
            tx.execute(r#"UPDATE "Project" SET archived = ?1 WHERE id = ?2"#, params![v as i64, id])?;
        }
        if let Some(tags) = &patch.tag_ids {
            tx.execute(r#"DELETE FROM "TagOnProject" WHERE projectId = ?1"#, [id])?;
            for t in tags {
                tx.execute(r#"INSERT INTO "TagOnProject"(id, tagId, projectId) VALUES (?1, ?2, ?3)"#, params![ids::cuid(), t, id])?;
            }
        }
        tx.commit()?;
        self.project(id)
    }

    /// Ids of projects that have at least one entry (these cannot be deleted).
    pub fn projects_with_entries(&self) -> Result<Vec<String>> {
        let mut st = self.conn().prepare(r#"SELECT DISTINCT projectId FROM "TimeEntry" WHERE projectId IS NOT NULL ORDER BY projectId"#)?;
        let rows = st.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn delete_project(&mut self, id: &str) -> Result<()> {
        self.ensure_writable()?;
        self.exists("Project", id)?;
        let n: i64 = self.conn().query_row(r#"SELECT COUNT(*) FROM "TimeEntry" WHERE projectId = ?1"#, [id], |r| r.get(0))?;
        if n > 0 {
            return Err(CoreError::code_msg("project_has_entries", "工数が登録されているため削除できません。アーカイブしてください。"));
        }
        self.conn().execute(r#"DELETE FROM "Project" WHERE id = ?1"#, [id])?;
        Ok(())
    }

    pub fn update_tag(&mut self, id: &str, patch: TagPatch) -> Result<Tag> {
        self.ensure_writable()?;
        if let Some(n) = &patch.name {
            check_name(n, 50)?;
        }
        if let Some(c) = &patch.color {
            check_color(c)?;
        }
        self.exists("Tag", id)?;
        if let Some(n) = &patch.name
            && self.tag_name_taken(n, Some(id))?
        {
            return Err(CoreError::code_msg("tag_already_exists", "同じ名前のタグがあります。"));
        }
        if let Some(n) = &patch.name {
            self.conn().execute(r#"UPDATE "Tag" SET name = ?1 WHERE id = ?2"#, params![n, id])?;
        }
        if let Some(c) = &patch.color {
            self.conn().execute(r#"UPDATE "Tag" SET color = ?1 WHERE id = ?2"#, params![c, id])?;
        }
        self.tags()?.into_iter().find(|t| t.id == id).ok_or_else(not_found)
    }

    /// Links to entries and projects cascade.
    pub fn delete_tag(&mut self, id: &str) -> Result<()> {
        self.ensure_writable()?;
        self.exists("Tag", id)?;
        self.conn().execute(r#"DELETE FROM "Tag" WHERE id = ?1"#, [id])?;
        Ok(())
    }
}

fn invalid() -> CoreError {
    CoreError::code_msg("invalid_input", "入力内容を確認してください。")
}

fn not_found() -> CoreError {
    CoreError::code_msg("not_found", "見つかりません。")
}

/// zod `.min(1).max(n)`: lengths count UTF-16 code units, no trimming.
fn check_name(name: &str, max: usize) -> Result<()> {
    let n = name.encode_utf16().count();
    if n == 0 || n > max { Err(invalid()) } else { Ok(()) }
}

/// `^#[0-9a-fA-F]{6}$`
fn check_color(color: &str) -> Result<()> {
    let ok = color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit());
    if ok { Ok(()) } else { Err(invalid()) }
}
