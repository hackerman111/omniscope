use rusqlite::Connection;
use std::sync::MutexGuard;

use crate::error::Result;
use crate::frecency;

pub struct FrecencyService<'a> {
    conn: MutexGuard<'a, Connection>,
}

impl<'a> FrecencyService<'a> {
    pub fn new(conn: MutexGuard<'a, Connection>) -> Self {
        Self { conn }
    }

    pub fn record_access(&self, id: &str) -> Result<()> {
        let current: f64 = self
            .conn
            .query_row(
                "SELECT frecency_score FROM books WHERE id = ?1",
                rusqlite::params![id],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        let new_score = current + frecency::calculate_frecency(1, chrono::Utc::now());

        self.conn.execute(
            "UPDATE books SET frecency_score = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![new_score, chrono::Utc::now().to_rfc3339(), id],
        )?;
        Ok(())
    }

    pub fn get_score(&self, id: &str) -> Result<f64> {
        let score: f64 = self
            .conn
            .query_row(
                "SELECT frecency_score FROM books WHERE id = ?1",
                rusqlite::params![id],
                |row| row.get(0),
            )
            .unwrap_or(0.0);
        Ok(score)
    }

    pub fn recalculate_all(&self) -> Result<usize> {
        let mut stmt = self.conn.prepare("SELECT id FROM books")?;
        let ids: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        let mut count = 0;
        for id in &ids {
            self.record_access(id)?;
            count += 1;
        }
        Ok(count)
    }
}
