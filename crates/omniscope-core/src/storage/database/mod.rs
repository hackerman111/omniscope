mod connection;
mod error;
mod migrations;
mod schema;

pub use connection::ConnectionPool;
pub use error::DatabaseError;
pub use migrations::{get_applied_versions, run_migrations, Migration};
pub use schema::{init_schema, SCHEMA_VERSION};

use std::path::Path;

use rusqlite::params;

use crate::error::{OmniscopeError, Result};
use crate::models::{BookCard, BookSummaryView, Folder};
use uuid::Uuid;

use super::repositories::{BookRepository, FolderRepository, LibraryRepository, Repository, TagRepository};

pub struct DatabaseConfig {
    pub path: Option<String>,
    pub enable_wal: bool,
    pub foreign_keys: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: None,
            enable_wal: true,
            foreign_keys: true,
        }
    }
}

pub fn open_database(path: &Path) -> Result<ConnectionPool> {
    let pool = ConnectionPool::open(path)?;
    {
        let conn = pool.get_connection();
        migrations::run_migrations(&conn)?;
    }
    Ok(pool)
}

pub fn open_in_memory() -> Result<ConnectionPool> {
    let pool = ConnectionPool::open_in_memory()?;
    {
        let conn = pool.get_connection();
        migrations::run_migrations(&conn)?;
    }
    Ok(pool)
}

pub struct Database {
    pool: ConnectionPool,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let pool = open_database(path)?;
        Ok(Self { pool })
    }

    pub fn open_in_memory() -> Result<Self> {
        let pool = open_in_memory()?;
        Ok(Self { pool })
    }

    pub fn init_schema(&self) -> Result<()> {
        Ok(())
    }

    pub fn upsert_book(&self, card: &BookCard) -> Result<()> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteBookRepository::new(conn);
        repo.save(card)
    }

    pub fn get_book_summary(&self, id: &str) -> Result<BookSummaryView> {
        let uuid = Uuid::parse_str(id).map_err(|_| OmniscopeError::BookNotFound(id.to_string()))?;
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteBookRepository::new(conn);
        repo.find_summary(&uuid)
    }

    pub fn list_books(&self, limit: usize, offset: usize) -> Result<Vec<BookSummaryView>> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteBookRepository::new(conn);
        repo.list(limit, offset)
    }

    pub fn delete_book(&self, id: &str) -> Result<()> {
        let uuid = Uuid::parse_str(id).map_err(|_| OmniscopeError::BookNotFound(id.to_string()))?;
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteBookRepository::new(conn);
        if !repo.delete(&uuid)? {
            return Err(OmniscopeError::BookNotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn get_book_card(&self, id: &str) -> Result<BookCard> {
        let uuid = Uuid::parse_str(id).map_err(|_| OmniscopeError::BookNotFound(id.to_string()))?;
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteBookRepository::new(conn);
        repo.find_by_id(&uuid)?.ok_or_else(|| OmniscopeError::BookNotFound(id.to_string()))
    }

    pub fn count_books(&self) -> Result<usize> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteBookRepository::new(conn);
        repo.count()
    }

    pub fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<BookSummaryView>> {
        let conn = self.pool.get_connection();
        let search = super::queries::BookSearchQuery::new(conn);
        search.fts(query, limit)
    }

    pub fn list_tags(&self) -> Result<Vec<(String, u32)>> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteTagRepository::new(conn);
        repo.list()
    }

    pub fn list_libraries(&self) -> Result<Vec<(String, u32)>> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteLibraryRepository::new(conn);
        let libraries = repo.list()?;
        Ok(libraries.into_iter().map(|l| (l.name, l.book_count)).collect())
    }

    pub fn list_books_by_tag(&self, tag: &str, limit: usize) -> Result<Vec<BookSummaryView>> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteBookRepository::new(conn);
        repo.list_by_tag(tag, limit)
    }

    pub fn list_books_by_library(&self, library: &str, limit: usize) -> Result<Vec<BookSummaryView>> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteBookRepository::new(conn);
        repo.list_by_library(library, limit)
    }

    pub fn list_books_by_folder_id(&self, folder_id: Option<&str>, limit: usize) -> Result<Vec<BookSummaryView>> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteBookRepository::new(conn);
        repo.list_by_folder_id(folder_id, limit)
    }

    pub fn sync_from_cards(&self, cards_dir: &std::path::Path) -> Result<usize> {
        let cards = crate::storage::json_cards::list_cards(cards_dir)?;
        let count = cards.len();

        let disk_ids: std::collections::HashSet<String> = cards.iter().map(|c| c.id.to_string()).collect();

        for card in &cards {
            self.upsert_book(card)?;
        }

        {
            let conn = self.pool.get_connection();
            let mut stmt = conn.prepare("SELECT id FROM books")?;
            let db_ids: Vec<String> = stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect();

            for db_id in &db_ids {
                if !disk_ids.contains(db_id) {
                    let _ = conn.execute("DELETE FROM books WHERE id = ?1", params![db_id]);
                }
            }
        }

        Ok(count)
    }

    pub fn record_access(&self, id: &str) -> Result<()> {
        let conn = self.pool.get_connection();
        let frecency = super::queries::FrecencyService::new(conn);
        frecency.record_access(id)
    }

    pub fn get_all_authors(&self) -> Result<Vec<String>> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteBookRepository::new(conn);
        repo.get_all_authors()
    }

    pub fn create_folder(&self, name: &str, parent_id: Option<&str>, library_id: Option<&str>) -> Result<String> {
        let folder = Folder::new(name);
        let folder = match parent_id {
            Some(pid) => folder.with_parent(pid),
            None => folder,
        };
        let folder = match library_id {
            Some(lid) => folder.with_library(lid),
            None => folder,
        };
        let id = folder.id.clone();

        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteFolderRepository::new(conn);
        repo.save(&folder)?;
        Ok(id)
    }

    pub fn create_virtual_folder(&self, name: &str) -> Result<String> {
        let mut folder = Folder::new(name);
        folder.folder_type = crate::models::FolderType::Virtual;
        let id = folder.id.clone();
        
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteFolderRepository::new(conn);
        repo.save(&folder)?;
        Ok(id)
    }

    pub fn list_folders(&self, parent_id: Option<&str>) -> Result<Vec<Folder>> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteFolderRepository::new(conn);
        repo.list_children(parent_id)
    }

    pub fn list_all_folders(&self) -> Result<Vec<Folder>> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteFolderRepository::new(conn);
        repo.list_all()
    }

    pub fn delete_folder(&self, id: &str) -> Result<()> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteFolderRepository::new(conn);
        repo.delete(&id.to_string())?;
        Ok(())
    }

    pub fn find_folder_by_id(&self, id: &str) -> Result<Option<Folder>> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteFolderRepository::new(conn);
        repo.find_by_id(&id.to_string())
    }

    pub fn update_folder(&self, folder: &Folder) -> Result<()> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteFolderRepository::new(conn);
        repo.save(folder)
    }

    pub fn rename_folder(&self, id: &str, new_name: &str) -> Result<()> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteFolderRepository::new(conn);
        repo.rename(id, new_name)
    }

    pub fn create_folder_with_path(
        &self,
        name: &str,
        parent_id: Option<&str>,
        library_id: Option<&str>,
        disk_path: &str,
    ) -> Result<String> {
        let folder = Folder::new(name)
            .with_parent_opt(parent_id)
            .with_library_opt(library_id)
            .with_disk_path(disk_path);
        let id = folder.id.clone();

        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteFolderRepository::new(conn);
        repo.save(&folder)?;
        Ok(id)
    }

    pub fn find_folder_by_disk_path(&self, disk_path: &str) -> Result<Option<String>> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteFolderRepository::new(conn);
        repo.find_by_disk_path(disk_path)
    }

    pub fn list_all_folder_paths(&self) -> Result<Vec<String>> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteFolderRepository::new(conn);
        repo.list_all_paths()
    }

    pub fn list_all_file_paths(&self) -> Result<Vec<String>> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteBookRepository::new(conn);
        repo.list_all_file_paths()
    }

    pub fn update_folder_path_recursive(&self, id: &str, old_path: &str, new_path: &str) -> Result<()> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteFolderRepository::new(conn);
        repo.update_path_recursive(old_path, new_path)?;
        Ok(())
    }

    pub fn list_virtual_folders(&self) -> Result<Vec<Folder>> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteFolderRepository::new(conn);
        repo.list_virtual_folders()
    }

    pub fn add_book_to_virtual_folder(&self, book_id: &str, folder_id: &str) -> Result<()> {
        let uuid = Uuid::parse_str(book_id).map_err(|_| OmniscopeError::BookNotFound(book_id.to_string()))?;
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteBookRepository::new(conn);
        repo.add_to_virtual_folder(&uuid, folder_id)
    }

    pub fn remove_book_from_virtual_folder(&self, book_id: &str, folder_id: &str) -> Result<()> {
        let uuid = Uuid::parse_str(book_id).map_err(|_| OmniscopeError::BookNotFound(book_id.to_string()))?;
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteBookRepository::new(conn);
        repo.remove_from_virtual_folder(&uuid, folder_id)
    }

    pub fn list_books_by_virtual_folder(&self, folder_id: &str, limit: usize) -> Result<Vec<BookSummaryView>> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteBookRepository::new(conn);
        repo.list_by_virtual_folder(folder_id, limit)
    }

    pub fn count_books_in_virtual_folder(&self, folder_id: &str) -> Result<usize> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteBookRepository::new(conn);
        repo.count_by_virtual_folder(folder_id)
    }

    pub fn find_books_by_path_prefix(&self, prefix: &str) -> Result<Vec<BookCard>> {
        let conn = self.pool.get_connection();
        let repo = super::repositories::SqliteBookRepository::new(conn);
        repo.find_books_by_path_prefix(prefix)
    }
}

#[cfg(feature = "async")]
pub mod async_db {
    use std::path::Path;

    use super::connection::async_pool::AsyncConnectionPool;
    use super::migrations;
    use crate::error::Result;

    pub async fn open_database(path: &Path) -> Result<AsyncConnectionPool> {
        let pool = AsyncConnectionPool::open(path).await?;
        {
            let conn = pool.get_connection().await;
            migrations::run_migrations(&conn)?;
        }
        Ok(pool)
    }

    pub async fn open_in_memory() -> Result<AsyncConnectionPool> {
        let pool = AsyncConnectionPool::open_in_memory().await?;
        {
            let conn = pool.get_connection().await;
            migrations::run_migrations(&conn)?;
        }
        Ok(pool)
    }
}
