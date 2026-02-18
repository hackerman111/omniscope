use std::path::Path;

use rusqlite::{Connection, params};

use crate::error::{OmniscopeError, Result};
use crate::models::{BookCard, BookSummaryView, ReadStatus};

/// SQLite database wrapper for fast queries.
/// JSON cards on disk are the source of truth; SQLite is a denormalized cache.
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open or create the database at the given path.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// Open an in-memory database (for testing).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// Create all tables if they don't exist.
    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS books (
                id          TEXT PRIMARY KEY,
                title       TEXT NOT NULL,
                authors     TEXT DEFAULT '[]',
                year        INTEGER,
                isbn        TEXT,
                file_path   TEXT,
                file_format TEXT,
                tags        TEXT DEFAULT '[]',
                libraries   TEXT DEFAULT '[]',
                folders     TEXT DEFAULT '[]',
                read_status TEXT DEFAULT 'unread',
                rating      INTEGER,
                summary     TEXT,
                key_topics  TEXT DEFAULT '[]',
                updated_at  TEXT NOT NULL,
                frecency_score REAL DEFAULT 0.0
            );

            CREATE TABLE IF NOT EXISTS tags (
                id          INTEGER PRIMARY KEY,
                name        TEXT UNIQUE NOT NULL,
                color       TEXT,
                description TEXT,
                book_count  INTEGER DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS libraries (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                description TEXT,
                icon        TEXT,
                color       TEXT
            );
            ",
        )?;

        // FTS5 virtual table — created separately since CREATE VIRTUAL TABLE
        // doesn't support IF NOT EXISTS in all SQLite versions.
        let has_fts: bool = self
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='books_fts'")?
            .exists([])?;

        if !has_fts {
            self.conn.execute_batch(
                "
                CREATE VIRTUAL TABLE books_fts USING fts5(
                    title, authors, tags, summary, key_topics,
                    content='books', content_rowid='rowid'
                );
                ",
            )?;
        }

        Ok(())
    }

    // ─── Book CRUD ──────────────────────────────────────────

    /// Insert or replace a book card into the database.
    pub fn upsert_book(&self, card: &BookCard) -> Result<()> {
        let authors_json = serde_json::to_string(&card.metadata.authors)?;
        let isbn = card.metadata.isbn.first().map(|s| s.as_str());
        let tags_json = serde_json::to_string(&card.organization.tags)?;
        let libraries_json = serde_json::to_string(&card.organization.libraries)?;
        let folders_json = serde_json::to_string(&card.organization.folders)?;
        let key_topics_json = serde_json::to_string(&card.ai.key_topics)?;

        self.conn.execute(
            "INSERT OR REPLACE INTO books
                (id, title, authors, year, isbn, file_path, file_format,
                 tags, libraries, folders, read_status, rating, summary,
                 key_topics, updated_at, frecency_score)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                card.id.to_string(),
                card.metadata.title,
                authors_json,
                card.metadata.year,
                isbn,
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

        // Update FTS index
        self.conn.execute(
            "INSERT OR REPLACE INTO books_fts(rowid, title, authors, tags, summary, key_topics)
             SELECT rowid, title, authors, tags, summary, key_topics FROM books WHERE id = ?1",
            params![card.id.to_string()],
        )?;

        Ok(())
    }

    /// Get a book summary by ID.
    pub fn get_book_summary(&self, id: &str) -> Result<BookSummaryView> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, authors, year, file_format, rating, read_status, tags,
                    file_path IS NOT NULL, frecency_score
             FROM books WHERE id = ?1",
        )?;

        stmt.query_row(params![id], |row| {
            let authors_str: String = row.get(2)?;
            let format_str: Option<String> = row.get(4)?;
            let status_str: String = row.get(6)?;
            let tags_str: String = row.get(7)?;

            Ok(BookSummaryView {
                id: uuid::Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                title: row.get(1)?,
                authors: serde_json::from_str(&authors_str).unwrap_or_default(),
                year: row.get(3)?,
                format: format_str.and_then(|s| {
                    serde_json::from_str(&format!("\"{s}\"")).ok()
                }),
                rating: row.get(5)?,
                read_status: match status_str.as_str() {
                    "reading" => ReadStatus::Reading,
                    "read" => ReadStatus::Read,
                    "dnf" => ReadStatus::Dnf,
                    _ => ReadStatus::Unread,
                },
                tags: serde_json::from_str(&tags_str).unwrap_or_default(),
                has_file: row.get(8)?,
                frecency_score: row.get(9)?,
            })
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                OmniscopeError::BookNotFound(id.to_string())
            }
            other => OmniscopeError::Database(other),
        })
    }

    /// List all book summaries, optionally filtered.
    pub fn list_books(&self, limit: usize, offset: usize) -> Result<Vec<BookSummaryView>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, authors, year, file_format, rating, read_status, tags,
                    file_path IS NOT NULL, frecency_score
             FROM books ORDER BY updated_at DESC LIMIT ?1 OFFSET ?2",
        )?;

        let rows = stmt
            .query_map(params![limit as i64, offset as i64], |row| {
                let authors_str: String = row.get(2)?;
                let format_str: Option<String> = row.get(4)?;
                let status_str: String = row.get(6)?;
                let tags_str: String = row.get(7)?;

                Ok(BookSummaryView {
                    id: uuid::Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                    title: row.get(1)?,
                    authors: serde_json::from_str(&authors_str).unwrap_or_default(),
                    year: row.get(3)?,
                    format: format_str.and_then(|s| {
                        serde_json::from_str(&format!("\"{s}\"")).ok()
                    }),
                    rating: row.get(5)?,
                    read_status: match status_str.as_str() {
                        "reading" => ReadStatus::Reading,
                        "read" => ReadStatus::Read,
                        "dnf" => ReadStatus::Dnf,
                        _ => ReadStatus::Unread,
                    },
                    tags: serde_json::from_str(&tags_str).unwrap_or_default(),
                    has_file: row.get(8)?,
                    frecency_score: row.get(9)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    /// Delete a book from the database.
    pub fn delete_book(&self, id: &str) -> Result<()> {
        let deleted = self
            .conn
            .execute("DELETE FROM books WHERE id = ?1", params![id])?;
        if deleted == 0 {
            return Err(OmniscopeError::BookNotFound(id.to_string()));
        }
        Ok(())
    }

    /// Count total books.
    pub fn count_books(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM books", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Full-text search using FTS5.
    pub fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<BookSummaryView>> {
        let mut stmt = self.conn.prepare(
            "SELECT b.id, b.title, b.authors, b.year, b.file_format, b.rating,
                    b.read_status, b.tags, b.file_path IS NOT NULL, b.frecency_score
             FROM books_fts f
             JOIN books b ON b.rowid = f.rowid
             WHERE books_fts MATCH ?1
             LIMIT ?2",
        )?;

        let rows = stmt
            .query_map(params![query, limit as i64], |row| {
                let authors_str: String = row.get(2)?;
                let format_str: Option<String> = row.get(4)?;
                let status_str: String = row.get(6)?;
                let tags_str: String = row.get(7)?;

                Ok(BookSummaryView {
                    id: uuid::Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                    title: row.get(1)?,
                    authors: serde_json::from_str(&authors_str).unwrap_or_default(),
                    year: row.get(3)?,
                    format: format_str.and_then(|s| {
                        serde_json::from_str(&format!("\"{s}\"")).ok()
                    }),
                    rating: row.get(5)?,
                    read_status: match status_str.as_str() {
                        "reading" => ReadStatus::Reading,
                        "read" => ReadStatus::Read,
                        "dnf" => ReadStatus::Dnf,
                        _ => ReadStatus::Unread,
                    },
                    tags: serde_json::from_str(&tags_str).unwrap_or_default(),
                    has_file: row.get(8)?,
                    frecency_score: row.get(9)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    // ─── Tags & Libraries ───────────────────────────────────

    /// Get all unique tags with book counts, extracted from the books table.
    pub fn list_tags(&self) -> Result<Vec<(String, u32)>> {
        // Tags are stored as JSON arrays in the books table, so we extract them
        let mut stmt = self.conn.prepare("SELECT tags FROM books")?;
        let mut tag_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();

        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for row in rows {
            let tags_str = row?;
            if let Ok(tags) = serde_json::from_str::<Vec<String>>(&tags_str) {
                for tag in tags {
                    *tag_counts.entry(tag).or_insert(0) += 1;
                }
            }
        }

        let mut result: Vec<(String, u32)> = tag_counts.into_iter().collect();
        result.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        Ok(result)
    }

    /// Get all unique libraries with book counts.
    pub fn list_libraries(&self) -> Result<Vec<(String, u32)>> {
        let mut stmt = self.conn.prepare("SELECT libraries FROM books")?;
        let mut lib_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();

        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for row in rows {
            let libs_str = row?;
            if let Ok(libs) = serde_json::from_str::<Vec<String>>(&libs_str) {
                for lib in libs {
                    *lib_counts.entry(lib).or_insert(0) += 1;
                }
            }
        }

        let mut result: Vec<(String, u32)> = lib_counts.into_iter().collect();
        result.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        Ok(result)
    }

    /// List books filtered by tag.
    pub fn list_books_by_tag(&self, tag: &str, limit: usize) -> Result<Vec<BookSummaryView>> {
        let pattern = format!("%\"{tag}\"%");
        let mut stmt = self.conn.prepare(
            "SELECT id, title, authors, year, file_format, rating, read_status, tags,
                    file_path IS NOT NULL, frecency_score
             FROM books WHERE tags LIKE ?1
             ORDER BY updated_at DESC LIMIT ?2",
        )?;

        let rows = stmt
            .query_map(params![pattern, limit as i64], |row| {
                Self::row_to_summary(row)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// List books filtered by library.
    pub fn list_books_by_library(&self, library: &str, limit: usize) -> Result<Vec<BookSummaryView>> {
        let pattern = format!("%\"{library}\"%");
        let mut stmt = self.conn.prepare(
            "SELECT id, title, authors, year, file_format, rating, read_status, tags,
                    file_path IS NOT NULL, frecency_score
             FROM books WHERE libraries LIKE ?1
             ORDER BY updated_at DESC LIMIT ?2",
        )?;

        let rows = stmt
            .query_map(params![pattern, limit as i64], |row| {
                Self::row_to_summary(row)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Sync all JSON cards from disk into the database.
    pub fn sync_from_cards(&self, cards_dir: &std::path::Path) -> Result<usize> {
        let cards = crate::storage::json_cards::list_cards(cards_dir)?;
        let count = cards.len();
        for card in &cards {
            self.upsert_book(card)?;
        }
        Ok(count)
    }

    // ─── Helper ─────────────────────────────────────────────

    fn row_to_summary(row: &rusqlite::Row) -> rusqlite::Result<BookSummaryView> {
        let authors_str: String = row.get(2)?;
        let format_str: Option<String> = row.get(4)?;
        let status_str: String = row.get(6)?;
        let tags_str: String = row.get(7)?;

        Ok(BookSummaryView {
            id: uuid::Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
            title: row.get(1)?,
            authors: serde_json::from_str(&authors_str).unwrap_or_default(),
            year: row.get(3)?,
            format: format_str.and_then(|s| {
                serde_json::from_str(&format!("\"{s}\"")).ok()
            }),
            rating: row.get(5)?,
            read_status: match status_str.as_str() {
                "reading" => ReadStatus::Reading,
                "read" => ReadStatus::Read,
                "dnf" => ReadStatus::Dnf,
                _ => ReadStatus::Unread,
            },
            tags: serde_json::from_str(&tags_str).unwrap_or_default(),
            has_file: row.get(8)?,
            frecency_score: row.get(9)?,
        })
    }

    // ─── Frecency ───────────────────────────────────────────

    /// Record a book access event and update its frecency score.
    pub fn record_access(&self, id: &str) -> Result<()> {
        // Get current score
        let current: f64 = self.conn.query_row(
            "SELECT frecency_score FROM books WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        ).unwrap_or(0.0);

        let new_score = current + crate::frecency::calculate_frecency(1, chrono::Utc::now());

        self.conn.execute(
            "UPDATE books SET frecency_score = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![new_score, chrono::Utc::now().to_rfc3339(), id],
        )?;
        Ok(())
    }

    // ─── Autocomplete ───────────────────────────────────────

    /// Get all unique authors for autocomplete.
    pub fn get_all_authors(&self) -> Result<Vec<String>> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_card(title: &str) -> BookCard {
        let mut card = BookCard::new(title);
        card.metadata.authors = vec!["Test Author".to_string()];
        card.metadata.year = Some(2023);
        card.organization.tags = vec!["rust".to_string()];
        card
    }

    #[test]
    fn test_open_in_memory() {
        let db = Database::open_in_memory().unwrap();
        assert_eq!(db.count_books().unwrap(), 0);
    }

    #[test]
    fn test_upsert_and_get() {
        let db = Database::open_in_memory().unwrap();
        let card = make_test_card("Test Book");
        let id = card.id.to_string();

        db.upsert_book(&card).unwrap();

        let summary = db.get_book_summary(&id).unwrap();
        assert_eq!(summary.title, "Test Book");
        assert_eq!(summary.authors, vec!["Test Author"]);
        assert_eq!(summary.year, Some(2023));
    }

    #[test]
    fn test_list_books() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_book(&make_test_card("Book A")).unwrap();
        db.upsert_book(&make_test_card("Book B")).unwrap();
        db.upsert_book(&make_test_card("Book C")).unwrap();

        let all = db.list_books(100, 0).unwrap();
        assert_eq!(all.len(), 3);

        let page = db.list_books(2, 0).unwrap();
        assert_eq!(page.len(), 2);
    }

    #[test]
    fn test_delete_book() {
        let db = Database::open_in_memory().unwrap();
        let card = make_test_card("Deletable");
        let id = card.id.to_string();

        db.upsert_book(&card).unwrap();
        assert_eq!(db.count_books().unwrap(), 1);

        db.delete_book(&id).unwrap();
        assert_eq!(db.count_books().unwrap(), 0);
    }

    #[test]
    fn test_delete_nonexistent() {
        let db = Database::open_in_memory().unwrap();
        let result = db.delete_book("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_search_fts() {
        let db = Database::open_in_memory().unwrap();

        let mut card1 = make_test_card("Rust Programming");
        card1.organization.tags = vec!["rust".to_string(), "programming".to_string()];
        db.upsert_book(&card1).unwrap();

        let mut card2 = make_test_card("Python Cookbook");
        card2.organization.tags = vec!["python".to_string()];
        db.upsert_book(&card2).unwrap();

        let results = db.search_fts("rust", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Programming");
    }

    #[test]
    fn test_upsert_updates_existing() {
        let db = Database::open_in_memory().unwrap();
        let mut card = make_test_card("Original Title");
        let id = card.id.to_string();

        db.upsert_book(&card).unwrap();

        card.metadata.title = "Updated Title".to_string();
        db.upsert_book(&card).unwrap();

        let summary = db.get_book_summary(&id).unwrap();
        assert_eq!(summary.title, "Updated Title");
        assert_eq!(db.count_books().unwrap(), 1);
    }
}
