use rusqlite::Connection;

use super::Migration;
use crate::error::Result;

pub struct V2DoiArxiv;

impl Migration for V2DoiArxiv {
    fn version(&self) -> u32 {
        2
    }

    fn description(&self) -> &'static str {
        "Add doi and arxiv_id columns to books table"
    }

    fn up(&self, conn: &Connection) -> Result<()> {
        let has_doi: bool = conn
            .prepare("SELECT 1 FROM pragma_table_info('books') WHERE name='doi'")?
            .exists([])?;

        if !has_doi {
            conn.execute_batch(
                "
                ALTER TABLE books ADD COLUMN doi TEXT;
                ALTER TABLE books ADD COLUMN arxiv_id TEXT;
                CREATE INDEX IF NOT EXISTS idx_books_doi      ON books(doi);
                CREATE INDEX IF NOT EXISTS idx_books_arxiv_id ON books(arxiv_id);
                ",
            )?;
        }
        Ok(())
    }
}
