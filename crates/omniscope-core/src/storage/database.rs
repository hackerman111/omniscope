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
    pub(crate) fn init_schema(&self) -> Result<()> {
        // Base tables + WAL mode
        self.conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS schema_migrations (
                version    INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS books (
                id          TEXT PRIMARY KEY,
                title       TEXT NOT NULL,
                authors     TEXT DEFAULT '[]',
                year        INTEGER,
                isbn        TEXT,
                doi         TEXT,
                arxiv_id    TEXT,
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

            CREATE TABLE IF NOT EXISTS folders (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                parent_id   TEXT,
                library_id  TEXT,
                FOREIGN KEY (parent_id) REFERENCES folders(id) ON DELETE SET NULL
            );

            CREATE TABLE IF NOT EXISTS action_log (
                id              TEXT PRIMARY KEY,
                action_type     TEXT NOT NULL,
                payload         TEXT NOT NULL,
                snapshot_before TEXT,
                created_at      TEXT NOT NULL,
                reversed        INTEGER NOT NULL DEFAULT 0
            );
            ",
        )?;

        // Indexes on columns that always exist
        self.conn.execute_batch(
            "
            CREATE INDEX IF NOT EXISTS idx_books_read_status ON books(read_status);
            CREATE INDEX IF NOT EXISTS idx_books_year        ON books(year);
            CREATE INDEX IF NOT EXISTS idx_books_rating      ON books(rating);
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

        // Record that schema version 1 has been applied
        self.conn.execute(
            "INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (1, ?1)",
            rusqlite::params![chrono::Utc::now().to_rfc3339()],
        )?;

        // ── Migration v2: add doi / arxiv_id columns if they are missing ──
        // These columns were added after the initial schema; existing databases
        // need an ALTER TABLE to pick them up.
        let has_doi: bool = self
            .conn
            .prepare("SELECT 1 FROM pragma_table_info('books') WHERE name='doi'")?
            .exists([])?;
        if !has_doi {
            self.conn.execute_batch(
                "ALTER TABLE books ADD COLUMN doi TEXT;
                 ALTER TABLE books ADD COLUMN arxiv_id TEXT;",
            )?;
            // Create the new indexes now that the columns exist
            self.conn.execute_batch(
                "CREATE INDEX IF NOT EXISTS idx_books_doi      ON books(doi);
                 CREATE INDEX IF NOT EXISTS idx_books_arxiv_id ON books(arxiv_id);",
            )?;
            self.conn.execute(
                "INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (2, ?1)",
                rusqlite::params![chrono::Utc::now().to_rfc3339()],
            )?;
        }

        // ── Migration v3: add disk_path column to folders ─────────────
        let has_disk_path: bool = self
            .conn
            .prepare("SELECT 1 FROM pragma_table_info('folders') WHERE name='disk_path'")?
            .exists([])?;
        if !has_disk_path {
            self.conn.execute_batch(
                "ALTER TABLE folders ADD COLUMN disk_path TEXT;",
            )?;
            self.conn.execute_batch(
                "CREATE INDEX IF NOT EXISTS idx_folders_disk_path ON folders(disk_path);",
            )?;
            self.conn.execute(
                "INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (3, ?1)",
                rusqlite::params![chrono::Utc::now().to_rfc3339()],
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

        // Extract doi / arxiv_id from identifiers if present
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
    /// Also purges DB entries whose JSON card no longer exists on disk.
    pub fn sync_from_cards(&self, cards_dir: &std::path::Path) -> Result<usize> {
        let cards = crate::storage::json_cards::list_cards(cards_dir)?;
        let count = cards.len();

        // Collect IDs of all cards on disk
        let disk_ids: std::collections::HashSet<String> = cards.iter()
            .map(|c| c.id.to_string())
            .collect();

        // Upsert all cards from disk
        for card in &cards {
            self.upsert_book(card)?;
        }

        // Purge stale DB entries: delete rows whose ID is not on disk
        let mut stmt = self.conn.prepare("SELECT id FROM books")?;
        let db_ids: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();

        for db_id in &db_ids {
            if !disk_ids.contains(db_id) {
                let _ = self.conn.execute("DELETE FROM books WHERE id = ?1", params![db_id]);
            }
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

    // ─── Folder CRUD ────────────────────────────────────────

    /// Create a new folder. Returns the new folder's ID.
    pub fn create_folder(
        &self,
        name: &str,
        parent_id: Option<&str>,
        library_id: Option<&str>,
    ) -> Result<String> {
        let id = uuid::Uuid::now_v7().to_string();
        self.conn.execute(
            "INSERT INTO folders (id, name, parent_id, library_id) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![id, name, parent_id, library_id],
        )?;
        Ok(id)
    }

    /// List folders, optionally filtering by parent folder.
    /// Pass `None` to list top-level folders.
    pub fn list_folders(&self, parent_id: Option<&str>) -> Result<Vec<Folder>> {
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

    /// Delete a folder by ID. Books in the folder are NOT deleted.
    pub fn delete_folder(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM folders WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }

    /// Rename a folder.
    pub fn rename_folder(&self, id: &str, new_name: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE folders SET name = ?1 WHERE id = ?2",
            rusqlite::params![new_name, id],
        )?;
        Ok(())
    }

    /// Create a folder with a disk path (relative to library root).
    pub fn create_folder_with_path(
        &self,
        name: &str,
        parent_id: Option<&str>,
        library_id: Option<&str>,
        disk_path: &str,
    ) -> Result<String> {
        let id = uuid::Uuid::now_v7().to_string();
        self.conn.execute(
            "INSERT INTO folders (id, name, parent_id, library_id, disk_path) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, name, parent_id, library_id, disk_path],
        )?;
        Ok(id)
    }

    /// Find a folder by its disk path (relative).
    pub fn find_folder_by_disk_path(&self, disk_path: &str) -> Result<Option<String>> {
        let result = self.conn.query_row(
            "SELECT id FROM folders WHERE disk_path = ?1",
            rusqlite::params![disk_path],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(OmniscopeError::Database(e)),
        }
    }

    /// List all folder disk_path values from the database.
    pub fn list_all_folder_paths(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT disk_path FROM folders WHERE disk_path IS NOT NULL",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// List all file paths from the books table.
    pub fn list_all_file_paths(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT file_path FROM books WHERE file_path IS NOT NULL",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }
}

/// A folder in the library hierarchy.
#[derive(Debug, Clone)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub library_id: Option<String>,
    pub disk_path: Option<String>,
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

    #[test]
    fn test_upsert_with_identifiers() {
        let db = Database::open_in_memory().unwrap();
        let mut card = make_test_card("Attention Is All You Need");
        card.identifiers = Some(crate::models::ScientificIdentifiers {
            doi: Some("10.5555/3295222.3295349".to_string()),
            arxiv_id: Some("1706.03762".to_string()),
            ..Default::default()
        });
        db.upsert_book(&card).unwrap();

        let id = card.id.to_string();
        let summary = db.get_book_summary(&id).unwrap();
        assert_eq!(summary.title, "Attention Is All You Need");
    }

    #[test]
    fn test_folder_crud() {
        let db = Database::open_in_memory().unwrap();

        // Create a top-level folder
        let folder_id = db.create_folder("Papers", None, None).unwrap();
        assert!(!folder_id.is_empty());

        let folders = db.list_folders(None).unwrap();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].name, "Papers");

        // Rename it
        db.rename_folder(&folder_id, "Research Papers").unwrap();
        let folders = db.list_folders(None).unwrap();
        assert_eq!(folders[0].name, "Research Papers");

        // Create a child folder
        let sub_id = db.create_folder("AI", Some(&folder_id), None).unwrap();
        let sub_folders = db.list_folders(Some(&folder_id)).unwrap();
        assert_eq!(sub_folders.len(), 1);
        assert_eq!(sub_folders[0].id, sub_id);

        // Delete the parent; ON DELETE SET NULL — child becomes top-level
        db.delete_folder(&folder_id).unwrap();
        let top_level = db.list_folders(None).unwrap();
        // Child now surfaces as top-level (parent_id = NULL)
        assert_eq!(top_level.len(), 1);
        assert_eq!(top_level[0].id, sub_id);

        // Delete the child too
        db.delete_folder(&sub_id).unwrap();
        let empty = db.list_folders(None).unwrap();
        assert!(empty.is_empty());
    }
}
