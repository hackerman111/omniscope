use rusqlite::Connection;

use super::Migration;
use crate::error::Result;

pub struct V3DiskPath;

impl Migration for V3DiskPath {
    fn version(&self) -> u32 {
        3
    }

    fn description(&self) -> &'static str {
        "Add disk_path column to folders table"
    }

    fn up(&self, conn: &Connection) -> Result<()> {
        let has_disk_path: bool = conn
            .prepare("SELECT 1 FROM pragma_table_info('folders') WHERE name='disk_path'")?
            .exists([])?;

        if !has_disk_path {
            conn.execute_batch(
                "
                ALTER TABLE folders ADD COLUMN disk_path TEXT;
                CREATE INDEX IF NOT EXISTS idx_folders_disk_path ON folders(disk_path);
                ",
            )?;
        }
        Ok(())
    }
}
