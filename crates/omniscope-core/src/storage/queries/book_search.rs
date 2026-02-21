use rusqlite::{params, Connection};
use std::str::FromStr;
use std::sync::MutexGuard;

use crate::error::Result;
use crate::models::{BookSummaryView, ReadStatus};
use uuid::Uuid;

pub struct BookSearchQuery<'a> {
    conn: MutexGuard<'a, Connection>,
}

impl<'a> BookSearchQuery<'a> {
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

    pub fn fts(&self, query: &str, limit: usize) -> Result<Vec<BookSummaryView>> {
        let mut stmt = self.conn.prepare(
            "SELECT b.id, b.title, b.authors, b.year, b.file_format, b.rating,
                    b.read_status, b.tags, b.file_path IS NOT NULL, b.frecency_score
             FROM books_fts f
             JOIN books b ON b.rowid = f.rowid
             WHERE books_fts MATCH ?1
             ORDER BY b.frecency_score DESC
             LIMIT ?2",
        )?;

        let rows = stmt
            .query_map(params![query, limit as i64], Self::row_to_summary)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    pub fn by_title(&self, title: &str, limit: usize) -> Result<Vec<BookSummaryView>> {
        let pattern = format!("%{title}%");
        let mut stmt = self.conn.prepare(
            "SELECT id, title, authors, year, file_format, rating, read_status, tags,
                    file_path IS NOT NULL, frecency_score
             FROM books WHERE title LIKE ?1
             ORDER BY frecency_score DESC
             LIMIT ?2",
        )?;

        let rows = stmt
            .query_map(params![pattern, limit as i64], Self::row_to_summary)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    pub fn by_author(&self, author: &str, limit: usize) -> Result<Vec<BookSummaryView>> {
        let pattern = format!("%\"{author}\"%");
        let mut stmt = self.conn.prepare(
            "SELECT id, title, authors, year, file_format, rating, read_status, tags,
                    file_path IS NOT NULL, frecency_score
             FROM books WHERE authors LIKE ?1
             ORDER BY frecency_score DESC
             LIMIT ?2",
        )?;

        let rows = stmt
            .query_map(params![pattern, limit as i64], Self::row_to_summary)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    pub fn autocomplete_tags(&self, prefix: &str, limit: usize) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT tags FROM books")?;
        let mut tag_counts: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();

        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for row in rows {
            let tags_str = row?;
            if let Ok(tags) = serde_json::from_str::<Vec<String>>(&tags_str) {
                for tag in tags {
                    *tag_counts.entry(tag).or_insert(0) += 1;
                }
            }
        }

        let prefix_lower = prefix.to_lowercase();
        let mut result: Vec<String> = tag_counts
            .keys()
            .filter(|tag| tag.to_lowercase().starts_with(&prefix_lower))
            .take(limit)
            .cloned()
            .collect();
        result.sort();
        Ok(result)
    }

    pub fn autocomplete_authors(&self, prefix: &str, limit: usize) -> Result<Vec<String>> {
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

        let prefix_lower = prefix.to_lowercase();
        let mut result: Vec<String> = authors_set
            .into_iter()
            .filter(|author| author.to_lowercase().starts_with(&prefix_lower))
            .take(limit)
            .collect();
        result.sort();
        Ok(result)
    }
}
