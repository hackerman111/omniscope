use rusqlite::{params, Connection};
use std::sync::MutexGuard;

use crate::error::Result;
use crate::models::Folder;

use super::Repository;

pub trait FolderRepository: Repository<Entity = Folder, Id = String> {
    fn list_children(&self, parent_id: Option<&str>) -> Result<Vec<Folder>>;
    fn list_all(&self) -> Result<Vec<Folder>>;
    fn list_virtual_folders(&self) -> Result<Vec<Folder>>;
    fn rename(&self, id: &str, new_name: &str) -> Result<()>;
    fn find_by_disk_path(&self, disk_path: &str) -> Result<Option<String>>;
    fn list_all_paths(&self) -> Result<Vec<String>>;
    fn update_path_recursive(&self, old_path: &str, new_path: &str) -> Result<()>;
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
            "SELECT id, name, folder_type, parent_id, library_id, disk_path, icon, color, sort_order, created_at, updated_at FROM folders WHERE id = ?1",
            params![id],
            |row| {
                Ok(Folder {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    folder_type: match row.get::<_, String>(2)?.as_str() {
                        "virtual" => crate::models::FolderType::Virtual,
                        "library_root" => crate::models::FolderType::LibraryRoot,
                        _ => crate::models::FolderType::Physical,
                    },
                    parent_id: row.get(3)?,
                    library_id: row.get(4)?,
                    disk_path: row.get(5)?,
                    icon: row.get(6)?,
                    color: row.get(7)?,
                    sort_order: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
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
        let folder_type_str = match folder.folder_type {
            crate::models::FolderType::Physical => "physical",
            crate::models::FolderType::Virtual => "virtual",
            crate::models::FolderType::LibraryRoot => "library_root",
        };

        self.conn.execute(
            "INSERT OR REPLACE INTO folders (id, name, folder_type, parent_id, library_id, disk_path, icon, color, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                folder.id,
                folder.name,
                folder_type_str,
                folder.parent_id,
                folder.library_id,
                folder.disk_path,
                folder.icon,
                folder.color,
                folder.sort_order,
                folder.created_at,
                folder.updated_at
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
                "SELECT id, name, folder_type, parent_id, library_id, disk_path, icon, color, sort_order, created_at, updated_at FROM folders WHERE parent_id = ?1 ORDER BY sort_order, name",
                vec![Box::new(pid.to_owned())],
            ),
            None => (
                "SELECT id, name, folder_type, parent_id, library_id, disk_path, icon, color, sort_order, created_at, updated_at FROM folders WHERE parent_id IS NULL ORDER BY sort_order, name",
                vec![],
            ),
        };

        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())), |row| {
            Ok(Folder {
                id: row.get(0)?,
                name: row.get(1)?,
                folder_type: match row.get::<_, String>(2)?.as_str() {
                    "virtual" => crate::models::FolderType::Virtual,
                    "library_root" => crate::models::FolderType::LibraryRoot,
                    _ => crate::models::FolderType::Physical,
                },
                parent_id: row.get(3)?,
                library_id: row.get(4)?,
                disk_path: row.get(5)?,
                icon: row.get(6)?,
                color: row.get(7)?,
                sort_order: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn list_all(&self) -> Result<Vec<Folder>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, folder_type, parent_id, library_id, disk_path, icon, color, sort_order, created_at, updated_at FROM folders ORDER BY sort_order, name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Folder {
                id: row.get(0)?,
                name: row.get(1)?,
                folder_type: match row.get::<_, String>(2)?.as_str() {
                    "virtual" => crate::models::FolderType::Virtual,
                    "library_root" => crate::models::FolderType::LibraryRoot,
                    _ => crate::models::FolderType::Physical,
                },
                parent_id: row.get(3)?,
                library_id: row.get(4)?,
                disk_path: row.get(5)?,
                icon: row.get(6)?,
                color: row.get(7)?,
                sort_order: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn list_virtual_folders(&self) -> Result<Vec<Folder>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, folder_type, parent_id, library_id, disk_path, icon, color, sort_order, created_at, updated_at FROM folders WHERE folder_type = 'virtual' ORDER BY sort_order, name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Folder {
                id: row.get(0)?,
                name: row.get(1)?,
                folder_type: crate::models::FolderType::Virtual,
                parent_id: row.get(3)?,
                library_id: row.get(4)?,
                disk_path: row.get(5)?,
                icon: row.get(6)?,
                color: row.get(7)?,
                sort_order: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
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

    fn update_path_recursive(&self, old_path: &str, new_path: &str) -> Result<()> {
        let like_pattern = format!("{}/%", old_path);
        let old_len = old_path.len() as i32;
        
        self.conn.execute(
            "UPDATE folders 
             SET disk_path = ?2 || SUBSTR(disk_path, ?3 + 1) 
             WHERE disk_path = ?1 OR disk_path LIKE ?4",
            params![old_path, new_path, old_len, like_pattern],
        )?;
        Ok(())
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
