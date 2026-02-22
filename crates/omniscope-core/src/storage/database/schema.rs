use rusqlite::Connection;

use crate::error::Result;

pub const SCHEMA_VERSION: u32 = 4;

pub fn apply_pragmas(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;
        ",
    )?;
    Ok(())
}

pub fn create_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
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
            frecency_score REAL DEFAULT 0.0,
            file_presence  TEXT NOT NULL DEFAULT '{\"type\": \"never_had_file\"}',
            folder_id      TEXT REFERENCES folders(id) ON DELETE SET NULL
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
            folder_type TEXT NOT NULL CHECK(folder_type IN ('physical', 'virtual', 'library_root')) DEFAULT 'physical',
            parent_id   TEXT REFERENCES folders(id) ON DELETE CASCADE,
            library_id  TEXT,
            disk_path   TEXT,
            icon        TEXT,
            color       TEXT,
            sort_order  INTEGER DEFAULT 0,
            created_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS book_virtual_folders (
            book_id     TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
            folder_id   TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
            added_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (book_id, folder_id)
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
    Ok(())
}

pub fn create_indexes(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE INDEX IF NOT EXISTS idx_books_read_status ON books(read_status);
        CREATE INDEX IF NOT EXISTS idx_books_year        ON books(year);
        CREATE INDEX IF NOT EXISTS idx_books_rating      ON books(rating);
        CREATE INDEX IF NOT EXISTS idx_books_doi         ON books(doi);
        CREATE INDEX IF NOT EXISTS idx_books_arxiv_id    ON books(arxiv_id);
        CREATE INDEX IF NOT EXISTS idx_folders_disk_path ON folders(disk_path);
        CREATE INDEX IF NOT EXISTS idx_folders_parent    ON folders(parent_id);
        CREATE INDEX IF NOT EXISTS idx_books_folder      ON books(folder_id);
        ",
    )?;
    Ok(())
}

pub fn create_fts_table(conn: &Connection) -> Result<()> {
    let has_fts: bool = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='books_fts'")?
        .exists([])?;

    if !has_fts {
        conn.execute_batch(
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

pub fn init_schema(conn: &Connection) -> Result<()> {
    create_tables(conn)?;
    create_indexes(conn)?;
    create_fts_table(conn)?;
    Ok(())
}
