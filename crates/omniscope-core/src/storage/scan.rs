//! Library scanning — discover untracked files and create cards.
//!
//! `scan_library()` walks the library root, finds book files that have
//! no matching card in the database, and optionally creates cards for them.

use std::path::PathBuf;

use crate::error::Result;
use crate::file_import;
use crate::models::BookCard;
use crate::storage::database::Database;
use crate::storage::library_root::LibraryRoot;

/// Result of scanning a library directory.
#[derive(Debug, Clone, Default)]
pub struct ScanResult {
    /// Total book files found on disk.
    pub total_files: usize,
    /// Files that already have cards.
    pub known_files: usize,
    /// Files with no card (newly discovered).
    pub new_files: Vec<PathBuf>,
    /// Cards that were auto-created (if `auto_create` was true).
    pub cards_created: usize,
    /// Files that failed to import.
    pub errors: Vec<(PathBuf, String)>,
}

/// Options for scanning.
pub struct ScanOptions {
    /// Automatically create cards for discovered files.
    pub auto_create_cards: bool,
    /// Recursively scan subdirectories.
    pub recursive: bool,
    /// Specific subdirectory to scan (relative to library root).
    /// If None, scans the entire library.
    pub subdirectory: Option<String>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            auto_create_cards: false,
            recursive: true,
            subdirectory: None,
        }
    }
}

/// Scan a library for untracked book files.
///
/// Walks the library root (or a subdirectory), finds book files with
/// recognized extensions, and compares against the database to find
/// files that don't have cards yet.
pub fn scan_library(
    library: &LibraryRoot,
    db: &Database,
    opts: ScanOptions,
) -> Result<ScanResult> {
    let scan_root = match &opts.subdirectory {
        Some(sub) => library.root().join(sub),
        None => library.root().to_path_buf(),
    };

    // Scan all book files on disk
    let disk_cards = file_import::scan_directory(&scan_root, opts.recursive)?;

    // Get all tracked file paths from DB
    let tracked = db.list_all_file_paths()?;
    let tracked_set: std::collections::HashSet<String> = tracked.into_iter().collect();

    let mut result = ScanResult {
        total_files: disk_cards.len(),
        ..Default::default()
    };

    for card in disk_cards {
        if let Some(ref file) = card.file {
            if tracked_set.contains(&file.path) {
                result.known_files += 1;
            } else {
                let file_path = PathBuf::from(&file.path);
                result.new_files.push(file_path.clone());

                // Auto-create card if requested
                if opts.auto_create_cards {
                    match create_card_from_import(library, db, card) {
                        Ok(()) => result.cards_created += 1,
                        Err(e) => {
                            result.errors.push((file_path, e.to_string()));
                        }
                    }
                }
            }
        }
    }

    Ok(result)
}

/// Create a card and persist it: write JSON, upsert into DB.
fn create_card_from_import(
    library: &LibraryRoot,
    db: &Database,
    card: BookCard,
) -> Result<()> {
    // Write JSON card to .libr/cards/
    crate::storage::json_cards::save_card(&library.cards_dir(), &card)?;

    // Upsert into SQLite
    db.upsert_book(&card)?;

    Ok(())
}

// ─── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::init::{init_library, InitOptions};
    use tempfile::TempDir;

    fn setup() -> (TempDir, LibraryRoot, Database) {
        let tmp = TempDir::new().unwrap();
        let lr = init_library(tmp.path(), InitOptions::minimal()).unwrap();
        let db = Database::open(&lr.database_path()).unwrap();
        (tmp, lr, db)
    }

    #[test]
    fn test_scan_empty_library() {
        let (_tmp, lr, db) = setup();
        let result = scan_library(&lr, &db, ScanOptions::default()).unwrap();
        assert_eq!(result.total_files, 0);
        assert_eq!(result.known_files, 0);
        assert!(result.new_files.is_empty());
    }

    #[test]
    fn test_scan_finds_untracked_files() {
        let (tmp, lr, db) = setup();

        // Drop untracked book files
        std::fs::write(tmp.path().join("new-book.pdf"), b"fake pdf").unwrap();
        std::fs::write(tmp.path().join("another.epub"), b"fake epub").unwrap();
        std::fs::write(tmp.path().join("readme.md"), b"not a book").unwrap();

        let result = scan_library(&lr, &db, ScanOptions::default()).unwrap();
        assert_eq!(result.total_files, 2); // pdf + epub
        assert_eq!(result.known_files, 0);
        assert_eq!(result.new_files.len(), 2);
    }

    #[test]
    fn test_scan_auto_create_cards() {
        let (tmp, lr, db) = setup();

        std::fs::write(tmp.path().join("auto-import.pdf"), b"fake pdf").unwrap();

        let opts = ScanOptions {
            auto_create_cards: true,
            ..Default::default()
        };
        let result = scan_library(&lr, &db, opts).unwrap();
        assert_eq!(result.cards_created, 1);

        // Book should now be in DB
        let books = db.list_books(10, 0).unwrap();
        assert_eq!(books.len(), 1);
    }

    #[test]
    fn test_scan_skips_known_files() {
        let (tmp, lr, db) = setup();

        // First: create a card for a file
        let path = tmp.path().join("known-book.pdf");
        std::fs::write(&path, b"fake pdf").unwrap();

        let opts = ScanOptions {
            auto_create_cards: true,
            ..Default::default()
        };
        let result = scan_library(&lr, &db, opts).unwrap();
        assert_eq!(result.cards_created, 1);

        // Second scan: should find 0 new files
        let result2 = scan_library(&lr, &db, ScanOptions::default()).unwrap();
        assert_eq!(result2.total_files, 1);
        assert_eq!(result2.known_files, 1);
        assert_eq!(result2.new_files.len(), 0);
    }

    #[test]
    fn test_scan_recursive_subdirectories() {
        let (tmp, lr, db) = setup();

        let subdir = tmp.path().join("papers").join("2025");
        std::fs::create_dir_all(&subdir).unwrap();
        std::fs::write(subdir.join("deep-paper.pdf"), b"fake pdf").unwrap();

        let result = scan_library(&lr, &db, ScanOptions::default()).unwrap();
        assert_eq!(result.total_files, 1);
        assert_eq!(result.new_files.len(), 1);
    }

    #[test]
    fn test_scan_specific_subdirectory() {
        let (tmp, lr, db) = setup();

        std::fs::create_dir_all(tmp.path().join("papers")).unwrap();
        std::fs::create_dir_all(tmp.path().join("other")).unwrap();
        std::fs::write(tmp.path().join("papers/paper.pdf"), b"fake pdf").unwrap();
        std::fs::write(tmp.path().join("other/other.pdf"), b"fake pdf").unwrap();

        let opts = ScanOptions {
            subdirectory: Some("papers".to_string()),
            ..Default::default()
        };
        let result = scan_library(&lr, &db, opts).unwrap();
        assert_eq!(result.total_files, 1); // only the one in papers/
    }

    #[test]
    fn test_scan_non_recursive() {
        let (tmp, lr, db) = setup();

        std::fs::write(tmp.path().join("root-book.pdf"), b"fake pdf").unwrap();
        let subdir = tmp.path().join("sub");
        std::fs::create_dir_all(&subdir).unwrap();
        std::fs::write(subdir.join("nested.pdf"), b"fake pdf").unwrap();

        let opts = ScanOptions {
            recursive: false,
            ..Default::default()
        };
        let result = scan_library(&lr, &db, opts).unwrap();
        assert_eq!(result.total_files, 1); // only root-book.pdf
    }
}
