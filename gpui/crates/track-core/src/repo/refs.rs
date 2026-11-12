//! Clients, projects and tags.
use crate::error::Result;
use crate::model::{Client, Project, Tag, TagLink};
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
        let id = ids::cuid();
        self.conn().execute(r#"INSERT INTO "Client"(id, name, createdAt) VALUES (?1, ?2, ?3)"#, params![id, name, time::now_db()])?;
        Ok(id)
    }

    pub fn create_project(&mut self, client_id: &str, name: &str, color: &str) -> Result<String> {
        self.ensure_writable()?;
        let id = ids::cuid();
        self.conn().execute(
            r#"INSERT INTO "Project"(id, clientId, name, color, createdAt) VALUES (?1, ?2, ?3, ?4, ?5)"#,
            params![id, client_id, name, color, time::now_db()],
        )?;
        Ok(id)
    }

    pub fn create_tag(&mut self, name: &str, color: &str) -> Result<String> {
        self.ensure_writable()?;
        let id = ids::cuid();
        self.conn().execute(r#"INSERT INTO "Tag"(id, name, color, createdAt) VALUES (?1, ?2, ?3, ?4)"#, params![id, name, color, time::now_db()])?;
        Ok(id)
    }
}
