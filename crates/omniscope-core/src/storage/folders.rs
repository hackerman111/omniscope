//! Folder management with real filesystem synchronization.
//!
//! Folders in Omniscope are **real directories on disk** — creating a folder
//! in the database also creates it on the filesystem, and syncing reconciles
//! the two states.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::file_import;
use crate::storage::database::Database;
use crate::storage::library_root::LibraryRoot;

// ─── Folder Templates ──────────────────────────────────────

/// Predefined folder structure templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderTemplate {
    /// For researchers/students:
    /// papers/{reading,read,to-read,by-topic,by-year}, books/{textbooks,reference}, notes/
    Research,
    /// For personal libraries:
    /// fiction/, non-fiction/, reference/, to-read/, favorites/
    Personal,
    /// For technical libraries:
    /// programming/{rust,python,systems,algorithms}, reference/, courses/
    Technical,
}

impl FolderTemplate {
    /// Get the list of relative directory paths for this template.
    pub fn directories(&self) -> Vec<&'static str> {
        match self {
            Self::Research => vec![
                "papers",
                "papers/reading",
                "papers/read",
                "papers/to-read",
                "papers/by-topic",
                "papers/by-year",
                "books",
                "books/textbooks",
                "books/reference",
                "notes",
            ],
            Self::Personal => vec![
                "fiction",
                "non-fiction",
                "reference",
                "to-read",
                "favorites",
            ],
            Self::Technical => vec![
                "programming",
                "programming/rust",
                "programming/python",
                "programming/systems",
                "programming/algorithms",
                "reference",
                "courses",
            ],
        }
    }

    /// Parse a template name from a string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "research" => Some(Self::Research),
            "personal" => Some(Self::Personal),
            "technical" => Some(Self::Technical),
            _ => None,
        }
    }
}

// ─── Scaffold ──────────────────────────────────────────────

/// Apply a folder template: create directories on disk and register in DB.
///
/// Returns the number of directories created.
pub fn scaffold_template(
    library: &LibraryRoot,
    db: &Database,
    template: FolderTemplate,
    dry_run: bool,
) -> Result<Vec<String>> {
    let dirs = template.directories();
    let mut created = Vec::new();

    for rel_path in dirs {
        let disk_path = library.root().join(rel_path);
        if disk_path.exists() {
            continue; // Already exists, skip
        }

        if dry_run {
            created.push(rel_path.to_string());
            continue;
        }

        // Create the directory on disk
        std::fs::create_dir_all(&disk_path)?;

        // Register in DB
        let folder_name = Path::new(rel_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(rel_path);

        let parent_rel = Path::new(rel_path)
            .parent()
            .and_then(|p| p.to_str())
            .filter(|s| !s.is_empty());

        // Find parent folder ID in DB
        let parent_id = if let Some(parent) = parent_rel {
            db.find_folder_by_disk_path(parent).ok().flatten()
        } else {
            None
        };

        let _id = db.create_folder_with_path(
            folder_name,
            parent_id.as_deref(),
            None,
            rel_path,
        )?;

        created.push(rel_path.to_string());
    }

    Ok(created)
}

/// Create a single folder on disk and register it in the DB.
pub fn create_folder_on_disk(
    library: &LibraryRoot,
    db: &Database,
    path: &str,
    parent_id: Option<&str>,
) -> Result<String> {
    let disk_path = library.root().join(path);

    // Create the directory
    std::fs::create_dir_all(&disk_path)?;

    // Register in DB
    let folder_name = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path);

    db.create_folder_with_path(folder_name, parent_id, None, path)
}

// ─── Sync ──────────────────────────────────────────────────

/// Result of synchronizing disk state with database state.
#[derive(Debug, Clone, Default)]
pub struct SyncReport {
    /// Folders that exist on disk but not in the database.
    pub new_on_disk: Vec<String>,
    /// Folders in the database that no longer exist on disk.
    pub missing_on_disk: Vec<String>,
    /// Folders that are in sync.
    pub in_sync: usize,
    /// Files on disk that have no card in the library.
    pub untracked_files: Vec<PathBuf>,
}

impl SyncReport {
    /// True if everything is in sync.
    pub fn is_clean(&self) -> bool {
        self.new_on_disk.is_empty()
            && self.missing_on_disk.is_empty()
            && self.untracked_files.is_empty()
    }
}

/// Scan the library directory tree and compare with the database.
///
/// Ignores the `.libr/` directory itself.
pub fn sync_folders(library: &LibraryRoot, db: &Database) -> Result<SyncReport> {
    let root = library.root();

    // 1. Discover directories on disk (relative paths)
    let disk_folders = scan_disk_directories(root)?;

    // 2. Get folders from DB
    let db_folders = db.list_all_folder_paths()?;
    let db_set: HashSet<String> = db_folders.into_iter().collect();

    let mut report = SyncReport::default();

    for folder in &disk_folders {
        if db_set.contains(folder) {
            report.in_sync += 1;
        } else {
            report.new_on_disk.push(folder.clone());
        }
    }

    let disk_set: HashSet<&str> = disk_folders.iter().map(|s| s.as_str()).collect();
    for db_folder in &db_set {
        if !disk_set.contains(db_folder.as_str()) {
            report.missing_on_disk.push(db_folder.clone());
        }
    }

    // 3. Find untracked files
    report.untracked_files = scan_untracked_files(library, db)?;

    Ok(report)
}

/// Discover all subdirectories under `root`, returning relative paths.
/// Ignores `.libr/` and hidden directories.
fn scan_disk_directories(root: &Path) -> Result<Vec<String>> {
    let mut result = Vec::new();
    scan_dirs_recursive(root, root, &mut result)?;
    Ok(result)
}

fn scan_dirs_recursive(root: &Path, current: &Path, result: &mut Vec<String>) -> Result<()> {
    let entries = match std::fs::read_dir(current) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden directories and .libr/
        if name_str.starts_with('.') {
            continue;
        }

        // Get relative path
        if let Ok(rel) = path.strip_prefix(root) {
            let rel_str = rel.to_string_lossy().to_string();
            // Normalize to forward slashes
            let rel_str = rel_str.replace('\\', "/");
            result.push(rel_str);
        }

        // Recurse
        scan_dirs_recursive(root, &path, result)?;
    }

    Ok(())
}

/// Find files on disk that have no corresponding card in the database.
fn scan_untracked_files(library: &LibraryRoot, db: &Database) -> Result<Vec<PathBuf>> {
    let root = library.root();

    // Get all tracked file paths from DB
    let tracked = db.list_all_file_paths()?;
    let tracked_set: HashSet<String> = tracked.into_iter().collect();

    // Scan all book files on disk
    let disk_files = file_import::scan_directory(root, true)?;

    let mut untracked = Vec::new();
    for card in &disk_files {
        if let Some(ref file) = card.file {
            if !tracked_set.contains(&file.path) {
                untracked.push(PathBuf::from(&file.path));
            }
        }
    }

    Ok(untracked)
}

// ─── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::init::{init_library, InitOptions};
    use tempfile::TempDir;

    #[test]
    fn test_folder_template_research() {
        let dirs = FolderTemplate::Research.directories();
        assert!(dirs.contains(&"papers"));
        assert!(dirs.contains(&"papers/reading"));
        assert!(dirs.contains(&"books/textbooks"));
        assert!(dirs.contains(&"notes"));
    }

    #[test]
    fn test_folder_template_personal() {
        let dirs = FolderTemplate::Personal.directories();
        assert!(dirs.contains(&"fiction"));
        assert!(dirs.contains(&"to-read"));
    }

    #[test]
    fn test_folder_template_from_str() {
        assert_eq!(FolderTemplate::from_str("research"), Some(FolderTemplate::Research));
        assert_eq!(FolderTemplate::from_str("PERSONAL"), Some(FolderTemplate::Personal));
        assert_eq!(FolderTemplate::from_str("unknown"), None);
    }

    #[test]
    fn test_scaffold_dry_run() {
        let tmp = TempDir::new().unwrap();
        let lr = init_library(tmp.path(), InitOptions::minimal()).unwrap();
        let db = Database::open(&lr.database_path()).unwrap();

        let created = scaffold_template(&lr, &db, FolderTemplate::Personal, true).unwrap();
        assert!(!created.is_empty());
        // In dry run, directories should NOT exist
        assert!(!tmp.path().join("fiction").exists());
    }

    #[test]
    fn test_scaffold_creates_directories() {
        let tmp = TempDir::new().unwrap();
        let lr = init_library(tmp.path(), InitOptions::minimal()).unwrap();
        let db = Database::open(&lr.database_path()).unwrap();

        let created = scaffold_template(&lr, &db, FolderTemplate::Personal, false).unwrap();
        assert!(!created.is_empty());
        // Directories should exist
        assert!(tmp.path().join("fiction").is_dir());
        assert!(tmp.path().join("non-fiction").is_dir());
        assert!(tmp.path().join("to-read").is_dir());
    }

    #[test]
    fn test_scaffold_idempotent() {
        let tmp = TempDir::new().unwrap();
        let lr = init_library(tmp.path(), InitOptions::minimal()).unwrap();
        let db = Database::open(&lr.database_path()).unwrap();

        let first = scaffold_template(&lr, &db, FolderTemplate::Personal, false).unwrap();
        assert!(!first.is_empty());

        // Second call should skip existing directories
        let second = scaffold_template(&lr, &db, FolderTemplate::Personal, false).unwrap();
        assert!(second.is_empty());
    }

    #[test]
    fn test_create_folder_on_disk() {
        let tmp = TempDir::new().unwrap();
        let lr = init_library(tmp.path(), InitOptions::minimal()).unwrap();
        let db = Database::open(&lr.database_path()).unwrap();

        let id = create_folder_on_disk(&lr, &db, "my-papers", None).unwrap();
        assert!(!id.is_empty());
        assert!(tmp.path().join("my-papers").is_dir());
    }

    #[test]
    fn test_sync_clean_library() {
        let tmp = TempDir::new().unwrap();
        let lr = init_library(tmp.path(), InitOptions::minimal()).unwrap();
        let db = Database::open(&lr.database_path()).unwrap();

        let report = sync_folders(&lr, &db).unwrap();
        assert!(report.is_clean());
    }

    #[test]
    fn test_sync_detects_new_disk_folders() {
        let tmp = TempDir::new().unwrap();
        let lr = init_library(tmp.path(), InitOptions::minimal()).unwrap();
        let db = Database::open(&lr.database_path()).unwrap();

        // Create a folder on disk NOT through the API
        std::fs::create_dir_all(tmp.path().join("mystery-folder")).unwrap();

        let report = sync_folders(&lr, &db).unwrap();
        assert!(report.new_on_disk.contains(&"mystery-folder".to_string()));
    }

    #[test]
    fn test_sync_detects_missing_folders() {
        let tmp = TempDir::new().unwrap();
        let lr = init_library(tmp.path(), InitOptions::minimal()).unwrap();
        let db = Database::open(&lr.database_path()).unwrap();

        // Create folder through API, then delete from disk
        create_folder_on_disk(&lr, &db, "will-delete", None).unwrap();
        assert!(tmp.path().join("will-delete").is_dir());

        std::fs::remove_dir(tmp.path().join("will-delete")).unwrap();

        let report = sync_folders(&lr, &db).unwrap();
        assert!(report.missing_on_disk.contains(&"will-delete".to_string()));
    }

    #[test]
    fn test_sync_reports_in_sync() {
        let tmp = TempDir::new().unwrap();
        let lr = init_library(tmp.path(), InitOptions::minimal()).unwrap();
        let db = Database::open(&lr.database_path()).unwrap();

        create_folder_on_disk(&lr, &db, "synced-folder", None).unwrap();

        let report = sync_folders(&lr, &db).unwrap();
        assert_eq!(report.in_sync, 1);
        assert!(report.new_on_disk.is_empty());
        assert!(report.missing_on_disk.is_empty());
    }

    #[test]
    fn test_sync_detects_untracked_files() {
        let tmp = TempDir::new().unwrap();
        let lr = init_library(tmp.path(), InitOptions::minimal()).unwrap();
        let db = Database::open(&lr.database_path()).unwrap();

        // Create a book file that has no card
        std::fs::write(tmp.path().join("untracked-book.pdf"), b"fake pdf").unwrap();

        let report = sync_folders(&lr, &db).unwrap();
        assert!(!report.untracked_files.is_empty());
    }

    #[test]
    fn test_scan_ignores_hidden_dirs() {
        let tmp = TempDir::new().unwrap();
        let lr = init_library(tmp.path(), InitOptions::minimal()).unwrap();
        let db = Database::open(&lr.database_path()).unwrap();

        // .libr/ is hidden and should be excluded
        // Create another hidden dir
        std::fs::create_dir_all(tmp.path().join(".hidden")).unwrap();
        std::fs::create_dir_all(tmp.path().join("visible")).unwrap();

        let report = sync_folders(&lr, &db).unwrap();
        // .hidden and .libr should not appear
        assert!(!report.new_on_disk.iter().any(|d| d.starts_with('.')));
        assert!(report.new_on_disk.contains(&"visible".to_string()));
    }
}
