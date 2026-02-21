use rusqlite::{params, Connection};
use std::sync::MutexGuard;

use crate::error::Result;
use crate::models::Folder;

use super::Repository;

pub trait FolderRepository: Repository<Entity = Folder, Id = String> {
    fn list_children(&self, parent_id: Option<&str>) -> Result<Vec<Folder>>;
    fn rename(&self, id: &str, new_name: &str) -> Result<()>;
    fn find_by_disk_path(&self, disk_path: &str) -> Result<Option<String>>;
    fn list_all_paths(&self) -> Result<Vec<String>>;
}

pub struct SqliteFolderRepository<'a> {
    conn: MutexGuard<'a, Connection>,
}

impl<'a> SqliteFolderRepository<'a> {
    pub fn new(conn: MutexGuard<'a, Connection>) -> Self {
        Self { conn }
    }
}

impl<'a> Repository for SqliteFolderRepository<'a> {
    type Entity = Folder;
    type Id = String;

    fn find_by_id(&self, id: &Self::Id) -> Result<Option<Self::Entity>> {
        let result = self.conn.query_row(
            "SELECT id, name, parent_id, library_id, disk_path FROM folders WHERE id = ?1",
            params![id],
            |row| {
                Ok(Folder {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    parent_id: row.get(2)?,
                    library_id: row.get(3)?,
                    disk_path: row.get(4)?,
                })
            },
        );

        match result {
            Ok(folder) => Ok(Some(folder)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn save(&self, folder: &Self::Entity) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO folders (id, name, parent_id, library_id, disk_path)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                folder.id,
                folder.name,
                folder.parent_id,
                folder.library_id,
                folder.disk_path
            ],
        )?;
        Ok(())
    }

    fn delete(&self, id: &Self::Id) -> Result<bool> {
        let deleted = self.conn.execute("DELETE FROM folders WHERE id = ?1", params![id])?;
        Ok(deleted > 0)
    }
}

impl<'a> FolderRepository for SqliteFolderRepository<'a> {
    fn list_children(&self, parent_id: Option<&str>) -> Result<Vec<Folder>> {
        let (sql, params_vec): (&str, Vec<Box<dyn rusqlite::ToSql>>) = match parent_id {
            Some(pid) => (
                "SELECT id, name, parent_id, library_id, disk_path FROM folders WHERE parent_id = ?1 ORDER BY name",
                vec![Box::new(pid.to_owned())],
            ),
            None => (
                "SELECT id, name, parent_id, library_id, disk_path FROM folders WHERE parent_id IS NULL ORDER BY name",
                vec![],
            ),
        };

        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())), |row| {
            Ok(Folder {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
                library_id: row.get(3)?,
                disk_path: row.get(4)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn rename(&self, id: &str, new_name: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE folders SET name = ?1 WHERE id = ?2",
            params![new_name, id],
        )?;
        Ok(())
    }

    fn find_by_disk_path(&self, disk_path: &str) -> Result<Option<String>> {
        let result = self.conn.query_row(
            "SELECT id FROM folders WHERE disk_path = ?1",
            params![disk_path],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn list_all_paths(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT disk_path FROM folders WHERE disk_path IS NOT NULL",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }
}

#[cfg(feature = "async")]
pub mod async_impl {
    use async_trait::async_trait;
    use rusqlite::Connection;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    use crate::error::Result;
    use crate::models::Folder;

    use super::FolderRepository;

    pub struct AsyncFolderRepository {
        conn: Arc<Mutex<Connection>>,
    }

    impl AsyncFolderRepository {
        pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
            Self { conn }
        }
    }

    #[async_trait]
    impl super::super::AsyncRepository for AsyncFolderRepository {
        type Entity = Folder;
        type Id = String;

        async fn find_by_id(&self, id: &Self::Id) -> Result<Option<Self::Entity>> {
            let conn = self.conn.lock().await;
            let repo = super::SqliteFolderRepository::new(conn);
            repo.find_by_id(id)
        }

        async fn save(&self, entity: &Self::Entity) -> Result<()> {
            let conn = self.conn.lock().await;
            let repo = super::SqliteFolderRepository::new(conn);
            repo.save(entity)
        }

        async fn delete(&self, id: &Self::Id) -> Result<bool> {
            let conn = self.conn.lock().await;
            let repo = super::SqliteFolderRepository::new(conn);
            repo.delete(id)
        }
    }
}
