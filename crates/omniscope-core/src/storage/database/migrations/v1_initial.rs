use rusqlite::Connection;

use super::Migration;
use crate::error::Result;
use crate::storage::database::schema;

pub struct V1Initial;

impl Migration for V1Initial {
    fn version(&self) -> u32 {
        1
    }

    fn description(&self) -> &'static str {
        "Initial schema with books, tags, libraries, folders, action_log tables"
    }

    fn up(&self, conn: &Connection) -> Result<()> {
        schema::create_tables(conn)?;
        schema::create_indexes(conn)?;
        schema::create_fts_table(conn)?;
        Ok(())
    }
}
