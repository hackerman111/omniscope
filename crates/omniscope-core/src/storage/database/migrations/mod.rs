mod v1_initial;
mod v2_doi_arxiv;
mod v3_disk_path;

use chrono::Utc;
use rusqlite::Connection;

use crate::error::Result;

pub trait Migration {
    fn version(&self) -> u32;
    fn description(&self) -> &'static str;
    fn up(&self, conn: &Connection) -> Result<()>;
}

fn record_migration(conn: &Connection, version: u32) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (?1, ?2)",
        rusqlite::params![version, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

fn is_migration_applied(conn: &Connection, version: u32) -> Result<bool> {
    let has_table: bool = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='schema_migrations'")?
        .exists([])?;

    if !has_table {
        return Ok(false);
    }

    let applied: bool = conn
        .prepare("SELECT 1 FROM schema_migrations WHERE version = ?1")?
        .exists(rusqlite::params![version])?;
    Ok(applied)
}

pub fn run_migrations(conn: &Connection) -> Result<()> {
    let migrations: Vec<Box<dyn Migration>> = vec![
        Box::new(v1_initial::V1Initial),
        Box::new(v2_doi_arxiv::V2DoiArxiv),
        Box::new(v3_disk_path::V3DiskPath),
    ];

    for migration in migrations {
        if !is_migration_applied(conn, migration.version())? {
            migration.up(conn)?;
            record_migration(conn, migration.version())?;
        }
    }

    Ok(())
}

pub fn get_applied_versions(conn: &Connection) -> Result<Vec<u32>> {
    let has_table: bool = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='schema_migrations'")?
        .exists([])?;

    if !has_table {
        return Ok(Vec::new());
    }

    let mut stmt = conn.prepare("SELECT version FROM schema_migrations ORDER BY version")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    let mut versions = Vec::new();
    for row in rows {
        versions.push(row?);
    }
    Ok(versions)
}
