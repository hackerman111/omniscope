use rusqlite::{params, Connection};
use std::str::FromStr;
use std::sync::MutexGuard;
use uuid::Uuid;

use crate::error::{OmniscopeError, Result};
use crate::models::{BookCard, BookSummaryView, ReadStatus};

use super::Repository;

pub trait BookRepository: Repository<Entity = BookCard, Id = Uuid> {
    fn find_summary(&self, id: &Uuid) -> Result<BookSummaryView>;
    fn list(&self, limit: usize, offset: usize) -> Result<Vec<BookSummaryView>>;
    fn count(&self) -> Result<usize>;
    fn list_by_tag(&self, tag: &str, limit: usize) -> Result<Vec<BookSummaryView>>;
    fn list_by_library(&self, library: &str, limit: usize) -> Result<Vec<BookSummaryView>>;
    fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<BookSummaryView>>;
    fn get_all_authors(&self) -> Result<Vec<String>>;
    fn update_frecency(&self, id: &Uuid, score: f64) -> Result<()>;
    fn list_all_file_paths(&self) -> Result<Vec<String>>;
}

pub struct SqliteBookRepository<'a> {
    conn: MutexGuard<'a, Connection>,
}

impl<'a> SqliteBookRepository<'a> {
    pub fn new(conn: MutexGuard<'a, Connection>) -> Self {
        Self { conn }
    }

    fn row_to_summary(row: &rusqlite::Row) -> rusqlite::Result<BookSummaryView> {
        let authors_str: String = row.get(2)?;
        let format_str: Option<String> = row.get(4)?;
        let status_str: String = row.get(6)?;
        let tags_str: String = row.get(7)?;

        Ok(BookSummaryView {
            id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
            title: row.get(1)?,
            authors: serde_json::from_str(&authors_str).unwrap_or_default(),
            year: row.get(3)?,
            format: format_str.and_then(|s| serde_json::from_str(&format!("\"{s}\"")).ok()),
            rating: row.get(5)?,
            read_status: ReadStatus::from_str(&status_str).unwrap_or_default(),
            tags: serde_json::from_str(&tags_str).unwrap_or_default(),
            has_file: row.get(8)?,
            frecency_score: row.get(9)?,
        })
    }
}

impl<'a> Repository for SqliteBookRepository<'a> {
    type Entity = BookCard;
    type Id = Uuid;

    fn find_by_id(&self, id: &Self::Id) -> Result<Option<Self::Entity>> {
        match self.find_summary(id) {
            Ok(_summary) => {
                let mut stmt = self.conn.prepare(
                    "SELECT id, title, authors, year, isbn, doi, arxiv_id, file_path,
                            file_format, tags, libraries, folders, read_status, rating,
                            summary, key_topics, updated_at, frecency_score
                     FROM books WHERE id = ?1",
                )?;

                let card = stmt
                    .query_row(params![id.to_string()], |row| {
                        let authors_str: String = row.get(2)?;
                        let tags_str: String = row.get(9)?;
                        let libraries_str: String = row.get(10)?;
                        let folders_str: String = row.get(11)?;
                        let key_topics_str: String = row.get(15)?;

                        let mut card = BookCard::new(row.get::<_, String>(1)?);
                        card.id = *id;
                        card.metadata.authors = serde_json::from_str(&authors_str).unwrap_or_default();
                        card.metadata.year = row.get(3)?;
                        card.metadata.isbn = row.get::<_, Option<String>>(4)?
                            .map(|s| vec![s])
                            .unwrap_or_default();
                        card.identifiers = Some(crate::models::ScientificIdentifiers {
                            doi: row.get(5)?,
                            arxiv_id: row.get(6)?,
                            ..Default::default()
                        });
                        card.organization.tags = serde_json::from_str(&tags_str).unwrap_or_default();
                        card.organization.libraries = serde_json::from_str(&libraries_str).unwrap_or_default();
                        card.organization.folders = serde_json::from_str(&folders_str).unwrap_or_default();
                        card.organization.read_status = ReadStatus::from_str(&row.get::<_, String>(12)?).unwrap_or_default();
                        card.organization.rating = row.get(13)?;
                        card.ai.summary = row.get(14)?;
                        card.ai.key_topics = serde_json::from_str(&key_topics_str).unwrap_or_default();
                        Ok(card)
                    })
                    .ok();
                Ok(card)
            }
            Err(OmniscopeError::BookNotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn save(&self, card: &Self::Entity) -> Result<()> {
        let authors_json = serde_json::to_string(&card.metadata.authors)?;
        let isbn = card.metadata.isbn.first().map(|s| s.as_str());
        let tags_json = serde_json::to_string(&card.organization.tags)?;
        let libraries_json = serde_json::to_string(&card.organization.libraries)?;
        let folders_json = serde_json::to_string(&card.organization.folders)?;
        let key_topics_json = serde_json::to_string(&card.ai.key_topics)?;

        let doi = card.identifiers.as_ref().and_then(|i| i.doi.as_deref());
        let arxiv_id = card.identifiers.as_ref().and_then(|i| i.arxiv_id.as_deref());

        self.conn.execute(
            "INSERT OR REPLACE INTO books
                (id, title, authors, year, isbn, doi, arxiv_id, file_path, file_format,
                 tags, libraries, folders, read_status, rating, summary,
                 key_topics, updated_at, frecency_score)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                card.id.to_string(),
                card.metadata.title,
                authors_json,
                card.metadata.year,
                isbn,
                doi,
                arxiv_id,
                card.file.as_ref().map(|f| &f.path),
                card.file.as_ref().map(|f| f.format.to_string()),
                tags_json,
                libraries_json,
                folders_json,
                card.organization.read_status.to_string(),
                card.organization.rating,
                card.ai.summary.as_deref(),
                key_topics_json,
                card.updated_at.to_rfc3339(),
                0.0f64,
            ],
        )?;

        self.conn.execute(
            "INSERT OR REPLACE INTO books_fts(rowid, title, authors, tags, summary, key_topics)
             SELECT rowid, title, authors, tags, summary, key_topics FROM books WHERE id = ?1",
            params![card.id.to_string()],
        )?;

        Ok(())
    }

    fn delete(&self, id: &Self::Id) -> Result<bool> {
        let deleted = self.conn.execute("DELETE FROM books WHERE id = ?1", params![id.to_string()])?;
        Ok(deleted > 0)
    }
}

impl<'a> BookRepository for SqliteBookRepository<'a> {
    fn find_summary(&self, id: &Uuid) -> Result<BookSummaryView> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, authors, year, file_format, rating, read_status, tags,
                    file_path IS NOT NULL, frecency_score
             FROM books WHERE id = ?1",
        )?;

        stmt.query_row(params![id.to_string()], Self::row_to_summary)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => OmniscopeError::BookNotFound(id.to_string()),
                other => OmniscopeError::Database(other),
            })
    }

    fn list(&self, limit: usize, offset: usize) -> Result<Vec<BookSummaryView>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, authors, year, file_format, rating, read_status, tags,
                    file_path IS NOT NULL, frecency_score
             FROM books ORDER BY updated_at DESC LIMIT ?1 OFFSET ?2",
        )?;

        let rows = stmt
            .query_map(params![limit as i64, offset as i64], Self::row_to_summary)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn count(&self) -> Result<usize> {
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM books", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    fn list_by_tag(&self, tag: &str, limit: usize) -> Result<Vec<BookSummaryView>> {
        let pattern = format!("%\"{tag}\"%");
        let mut stmt = self.conn.prepare(
            "SELECT id, title, authors, year, file_format, rating, read_status, tags,
                    file_path IS NOT NULL, frecency_score
             FROM books WHERE tags LIKE ?1
             ORDER BY updated_at DESC LIMIT ?2",
        )?;

        let rows = stmt
            .query_map(params![pattern, limit as i64], Self::row_to_summary)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn list_by_library(&self, library: &str, limit: usize) -> Result<Vec<BookSummaryView>> {
        let pattern = format!("%\"{library}\"%");
        let mut stmt = self.conn.prepare(
            "SELECT id, title, authors, year, file_format, rating, read_status, tags,
                    file_path IS NOT NULL, frecency_score
             FROM books WHERE libraries LIKE ?1
             ORDER BY updated_at DESC LIMIT ?2",
        )?;

        let rows = stmt
            .query_map(params![pattern, limit as i64], Self::row_to_summary)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<BookSummaryView>> {
        let mut stmt = self.conn.prepare(
            "SELECT b.id, b.title, b.authors, b.year, b.file_format, b.rating,
                    b.read_status, b.tags, b.file_path IS NOT NULL, b.frecency_score
             FROM books_fts f
             JOIN books b ON b.rowid = f.rowid
             WHERE books_fts MATCH ?1
             LIMIT ?2",
        )?;

        let rows = stmt
            .query_map(params![query, limit as i64], Self::row_to_summary)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn get_all_authors(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT authors FROM books")?;
        let mut authors_set: std::collections::HashSet<String> = std::collections::HashSet::new();

        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for row in rows {
            let authors_str = row?;
            if let Ok(authors) = serde_json::from_str::<Vec<String>>(&authors_str) {
                for author in authors {
                    authors_set.insert(author);
                }
            }
        }

        let mut result: Vec<String> = authors_set.into_iter().collect();
        result.sort();
        Ok(result)
    }

    fn update_frecency(&self, id: &Uuid, score: f64) -> Result<()> {
        self.conn.execute(
            "UPDATE books SET frecency_score = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![score, chrono::Utc::now().to_rfc3339(), id.to_string()],
        )?;
        Ok(())
    }

    fn list_all_file_paths(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT file_path FROM books WHERE file_path IS NOT NULL",
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
    use crate::models::{BookCard, BookSummaryView, ReadStatus};
    use uuid::Uuid;

    use super::BookRepository;

    pub struct AsyncBookRepository {
        conn: Arc<Mutex<Connection>>,
    }

    impl AsyncBookRepository {
        pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
            Self { conn }
        }
    }

    #[async_trait]
    impl super::super::AsyncRepository for AsyncBookRepository {
        type Entity = BookCard;
        type Id = Uuid;

        async fn find_by_id(&self, id: &Self::Id) -> Result<Option<Self::Entity>> {
            let conn = self.conn.lock().await;
            let repo = super::SqliteBookRepository::new(conn);
            repo.find_by_id(id)
        }

        async fn save(&self, entity: &Self::Entity) -> Result<()> {
            let conn = self.conn.lock().await;
            let repo = super::SqliteBookRepository::new(conn);
            repo.save(entity)
        }

        async fn delete(&self, id: &Self::Id) -> Result<bool> {
            let conn = self.conn.lock().await;
            let repo = super::SqliteBookRepository::new(conn);
            repo.delete(id)
        }
    }

    #[async_trait]
    impl BookRepository for AsyncBookRepository {
        fn find_summary(&self, _id: &Uuid) -> Result<BookSummaryView> {
            unimplemented!("Use async methods with async repository")
        }

        fn list(&self, _limit: usize, _offset: usize) -> Result<Vec<BookSummaryView>> {
            unimplemented!("Use async methods with async repository")
        }

        fn count(&self) -> Result<usize> {
            unimplemented!("Use async methods with async repository")
        }

        fn list_by_tag(&self, _tag: &str, _limit: usize) -> Result<Vec<BookSummaryView>> {
            unimplemented!("Use async methods with async repository")
        }

        fn list_by_library(&self, _library: &str, _limit: usize) -> Result<Vec<BookSummaryView>> {
            unimplemented!("Use async methods with async repository")
        }

        fn search_fts(&self, _query: &str, _limit: usize) -> Result<Vec<BookSummaryView>> {
            unimplemented!("Use async methods with async repository")
        }

        fn get_all_authors(&self) -> Result<Vec<String>> {
            unimplemented!("Use async methods with async repository")
        }

        fn update_frecency(&self, _id: &Uuid, _score: f64) -> Result<()> {
            unimplemented!("Use async methods with async repository")
        }

        fn list_all_file_paths(&self) -> Result<Vec<String>> {
            unimplemented!("Use async methods with async repository")
        }
    }
}
