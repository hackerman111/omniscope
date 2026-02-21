use rusqlite::{params, Connection};
use std::sync::MutexGuard;

use crate::error::Result;
use crate::models::Library;

use super::Repository;

pub trait LibraryRepository: Repository<Entity = Library, Id = String> {
    fn list(&self) -> Result<Vec<Library>>;
    fn count_books(&self, library_id: &str) -> Result<u32>;
}

pub struct SqliteLibraryRepository<'a> {
    conn: MutexGuard<'a, Connection>,
}

impl<'a> SqliteLibraryRepository<'a> {
    pub fn new(conn: MutexGuard<'a, Connection>) -> Self {
        Self { conn }
    }
}

impl<'a> Repository for SqliteLibraryRepository<'a> {
    type Entity = Library;
    type Id = String;

    fn find_by_id(&self, id: &Self::Id) -> Result<Option<Self::Entity>> {
        let result = self.conn.query_row(
            "SELECT id, name, description, icon, color FROM libraries WHERE id = ?1",
            params![id],
            |row| {
                Ok(Library {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    icon: row.get(3)?,
                    color: row.get(4)?,
                    book_count: 0,
                })
            },
        );

        match result {
            Ok(mut library) => {
                library.book_count = self.count_books(&library.id)?;
                Ok(Some(library))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn save(&self, library: &Self::Entity) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO libraries (id, name, description, icon, color)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                library.id,
                library.name,
                library.description,
                library.icon,
                library.color
            ],
        )?;
        Ok(())
    }

    fn delete(&self, id: &Self::Id) -> Result<bool> {
        let deleted = self.conn.execute("DELETE FROM libraries WHERE id = ?1", params![id])?;
        Ok(deleted > 0)
    }
}

impl<'a> LibraryRepository for SqliteLibraryRepository<'a> {
    fn list(&self) -> Result<Vec<Library>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, description, icon, color FROM libraries ORDER BY name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Library {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                icon: row.get(3)?,
                color: row.get(4)?,
                book_count: 0,
            })
        })?;
        let mut libraries: Vec<Library> = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        for lib in &mut libraries {
            lib.book_count = self.count_books(&lib.id)?;
        }
        Ok(libraries)
    }

    fn count_books(&self, library_id: &str) -> Result<u32> {
        let pattern = format!("%\"{library_id}\"%");
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM books WHERE libraries LIKE ?1",
            params![pattern],
            |row| row.get(0),
        )?;
        Ok(count as u32)
    }
}

#[cfg(feature = "async")]
pub mod async_impl {
    use async_trait::async_trait;
    use rusqlite::Connection;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    use crate::error::Result;
    use crate::models::Library;

    use super::LibraryRepository;

    pub struct AsyncLibraryRepository {
        conn: Arc<Mutex<Connection>>,
    }

    impl AsyncLibraryRepository {
        pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
            Self { conn }
        }
    }

    #[async_trait]
    impl super::super::AsyncRepository for AsyncLibraryRepository {
        type Entity = Library;
        type Id = String;

        async fn find_by_id(&self, id: &Self::Id) -> Result<Option<Self::Entity>> {
            let conn = self.conn.lock().await;
            let repo = super::SqliteLibraryRepository::new(conn);
            repo.find_by_id(id)
        }

        async fn save(&self, entity: &Self::Entity) -> Result<()> {
            let conn = self.conn.lock().await;
            let repo = super::SqliteLibraryRepository::new(conn);
            repo.save(entity)
        }

        async fn delete(&self, id: &Self::Id) -> Result<bool> {
            let conn = self.conn.lock().await;
            let repo = super::SqliteLibraryRepository::new(conn);
            repo.delete(id)
        }
    }
}
