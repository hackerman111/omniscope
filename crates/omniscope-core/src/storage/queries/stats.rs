use rusqlite::Connection;
use std::sync::MutexGuard;

use crate::error::Result;
use crate::models::{LibraryStats, ReadStatus};

pub struct LibraryStatsQuery<'a> {
    conn: MutexGuard<'a, Connection>,
}

impl<'a> LibraryStatsQuery<'a> {
    pub fn new(conn: MutexGuard<'a, Connection>) -> Self {
        Self { conn }
    }

    pub fn get_stats(&self) -> Result<LibraryStats> {
        let total: usize = self
            .conn
            .query_row("SELECT COUNT(*) FROM books", [], |row| {
                row.get::<_, i64>(0).map(|n| n as usize)
            })?;

        let unread: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM books WHERE read_status = 'unread'",
            [],
            |row| row.get::<_, i64>(0).map(|n| n as usize),
        )?;

        let reading: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM books WHERE read_status = 'reading'",
            [],
            |row| row.get::<_, i64>(0).map(|n| n as usize),
        )?;

        let read: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM books WHERE read_status = 'read'",
            [],
            |row| row.get::<_, i64>(0).map(|n| n as usize),
        )?;

        let dnf: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM books WHERE read_status = 'dnf'",
            [],
            |row| row.get::<_, i64>(0).map(|n| n as usize),
        )?;

        let with_file: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM books WHERE file_path IS NOT NULL",
            [],
            |row| row.get::<_, i64>(0).map(|n| n as usize),
        )?;

        let with_summary: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM books WHERE summary IS NOT NULL AND summary != ''",
            [],
            |row| row.get::<_, i64>(0).map(|n| n as usize),
        )?;

        let pdf: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM books WHERE file_format = 'pdf'",
            [],
            |row| row.get::<_, i64>(0).map(|n| n as usize),
        )?;

        let epub: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM books WHERE file_format = 'epub'",
            [],
            |row| row.get::<_, i64>(0).map(|n| n as usize),
        )?;

        let other_format: usize = total.saturating_sub(pdf).saturating_sub(epub);

        Ok(LibraryStats {
            total,
            unread,
            reading,
            read,
            dnf,
            with_file,
            with_summary,
            pdf,
            epub,
            other_format,
        })
    }

    pub fn count_by_status(&self, status: ReadStatus) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM books WHERE read_status = ?1",
            rusqlite::params![status.to_string()],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    pub fn count_by_rating(&self, rating: u8) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM books WHERE rating = ?1",
            rusqlite::params![rating],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    pub fn count_by_year(&self, year: i32) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM books WHERE year = ?1",
            rusqlite::params![year],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }
}
