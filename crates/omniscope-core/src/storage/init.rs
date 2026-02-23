use std::path::Path;

use crate::error::{OmniscopeError, Result};
use crate::models::manifest::LibraryManifest;
use crate::storage::library_root::LibraryRoot;

/// Options for `init_library`.
#[derive(Default)]
pub struct InitOptions {
    /// Custom name for the library (defaults to directory name).
    pub name: Option<String>,
    /// Create the root directory if it doesn't exist.
    pub create_dir: bool,
    /// Scan existing files and create cards after init.
    pub scan_existing: bool,
}

impl InitOptions {
    /// Minimal init — no scanning, no directory creation.
    pub fn minimal() -> Self {
        Self::default()
    }
}

/// All subdirectories created inside `.libr/`.
const LIBR_SUBDIRS: &[&str] = &[
    "cards",
    "db",
    "db/tantivy",
    "vectors",
    "cache",
    "cache/covers",
    "cache/crossref",
    "cache/s2",
    "cache/annas",
    "undo",
    "backups",
];

/// Initialize a new Omniscope library in the given directory.
///
/// Creates the `.libr/` structure, writes `library.toml`, and initializes
/// the SQLite database with the full schema.
///
/// # Errors
///
/// Returns an error if:
/// - The directory doesn't exist (and `opts.create_dir` is false)
/// - A `.libr/` directory already exists at the target
/// - Filesystem or database operations fail
pub fn init_library(root: &Path, opts: InitOptions) -> Result<LibraryRoot> {
    // 1. Create directory if requested
    if opts.create_dir {
        std::fs::create_dir_all(root)?;
    }

    // Check directory exists
    if !root.exists() {
        return Err(OmniscopeError::DirectoryNotFound(
            root.display().to_string(),
        ));
    }

    // Check no existing library
    let libr = root.join(".libr");
    if libr.exists() {
        return Err(OmniscopeError::LibraryAlreadyExists(
            root.display().to_string(),
        ));
    }

    // 2. Create .libr/ subdirectory structure
    for dir in LIBR_SUBDIRS {
        std::fs::create_dir_all(libr.join(dir))?;
    }

    // 3. Create library.toml manifest
    let name = opts.name.unwrap_or_else(|| {
        root.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("My Library")
            .to_string()
    });
    let manifest = LibraryManifest::new(name);
    let toml_str = manifest.to_toml()?;
    std::fs::write(libr.join("library.toml"), toml_str)?;

    // 4. Create SQLite database with schema
    let library_root = LibraryRoot::new(root.to_path_buf());
    let db = crate::storage::database::Database::open(&library_root.database_path())?;
    db.init_schema()?;

    Ok(library_root)
}

// ─── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_init_creates_directory_structure() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("my-books");
        std::fs::create_dir_all(&root).unwrap();

        let lr = init_library(&root, InitOptions::minimal()).unwrap();

        // Check all expected directories exist
        assert!(lr.libr_dir().is_dir());
        assert!(lr.cards_dir().is_dir());
        assert!(lr.database_path().parent().unwrap().is_dir());
        assert!(lr.covers_dir().is_dir());
        assert!(lr.undo_dir().is_dir());
        assert!(lr.backups_dir().is_dir());

        // Check extra cache dirs
        assert!(lr.libr_dir().join("cache/crossref").is_dir());
        assert!(lr.libr_dir().join("cache/s2").is_dir());
        assert!(lr.libr_dir().join("cache/annas").is_dir());
        assert!(lr.libr_dir().join("db/tantivy").is_dir());
        assert!(lr.libr_dir().join("vectors").is_dir());
    }

    #[test]
    fn test_init_creates_valid_manifest() {
        let tmp = TempDir::new().unwrap();
        let lr = init_library(tmp.path(), InitOptions::minimal()).unwrap();

        let manifest = lr.load_manifest().unwrap();
        // Default name from directory basename
        let expected_name = tmp
            .path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert_eq!(manifest.library.name, expected_name);
        assert!(!manifest.library.id.is_empty());
        assert_eq!(manifest.library.version, 1);
    }

    #[test]
    fn test_init_creates_sqlite_with_schema() {
        let tmp = TempDir::new().unwrap();
        let lr = init_library(tmp.path(), InitOptions::minimal()).unwrap();

        // Verify we can open the DB and it has tables
        let db = crate::storage::database::Database::open(&lr.database_path()).unwrap();
        // list_books should work (returns empty)
        let books = db.list_books(10, 0).unwrap();
        assert!(books.is_empty());
    }

    #[test]
    fn test_init_fails_if_already_exists() {
        let tmp = TempDir::new().unwrap();
        init_library(tmp.path(), InitOptions::minimal()).unwrap();

        // Second init should fail
        let result = init_library(tmp.path(), InitOptions::minimal());
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("already exists"));
    }

    #[test]
    fn test_init_with_custom_name() {
        let tmp = TempDir::new().unwrap();
        let opts = InitOptions {
            name: Some("Programming Books".to_string()),
            ..InitOptions::minimal()
        };
        let lr = init_library(tmp.path(), opts).unwrap();

        let manifest = lr.load_manifest().unwrap();
        assert_eq!(manifest.library.name, "Programming Books");
    }

    #[test]
    fn test_init_with_create_dir() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("new-library-dir");
        assert!(!root.exists());

        let opts = InitOptions {
            create_dir: true,
            ..InitOptions::minimal()
        };
        let lr = init_library(&root, opts).unwrap();
        assert!(lr.root().exists());
        assert!(lr.libr_dir().is_dir());
    }

    #[test]
    fn test_init_fails_nonexistent_dir_without_create() {
        let result = init_library(
            Path::new("/tmp/omniscope_test_nonexistent_42_1234"),
            InitOptions::minimal(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_init_discover_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let lr = init_library(tmp.path(), InitOptions::minimal()).unwrap();

        // Create a nested subdirectory
        let subdir = tmp.path().join("programming").join("rust");
        std::fs::create_dir_all(&subdir).unwrap();

        // Discover from the subdirectory
        let found = LibraryRoot::discover(&subdir).unwrap();
        assert_eq!(found.root(), lr.root());
    }
}
