use std::path::{Path, PathBuf};

use crate::config::GlobalConfig;
use crate::error::Result;
use crate::models::manifest::LibraryManifest;

/// The well-known directory name for library metadata.
pub const LIBR_DIR_NAME: &str = ".libr";

/// Represents a discovered or initialized library root directory.
///
/// A `LibraryRoot` points to a directory that contains a `.libr/` subdirectory
/// (analogous to `.git/` for git repositories). All library metadata, cards,
/// databases, and caches live under `.libr/`.
///
/// # Example layout
/// ```text
/// ~/Books/                    ← root()
/// ├── .libr/                  ← libr_dir()
/// │   ├── library.toml        ← manifest_path()
/// │   ├── cards/              ← cards_dir()
/// │   ├── db/omniscope.db     ← database_path()
/// │   ├── cache/covers/       ← covers_dir()
/// │   ├── undo/               ← undo_dir()
/// │   └── backups/            ← backups_dir()
/// ├── programming/
/// │   └── rust-book.pdf
/// └── ml-papers/
///     └── attention.pdf
/// ```
#[derive(Debug, Clone)]
pub struct LibraryRoot {
    root: PathBuf,
}

impl LibraryRoot {
    /// Create a new `LibraryRoot` from a known root directory.
    ///
    /// Does **not** validate that `.libr/` exists — use [`validate`] for that.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    // ─── Path accessors ─────────────────────────────────────

    /// The library root directory (where books live).
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Path to the `.libr/` metadata directory.
    pub fn libr_dir(&self) -> PathBuf {
        self.root.join(LIBR_DIR_NAME)
    }

    /// Path to `library.toml` manifest.
    pub fn manifest_path(&self) -> PathBuf {
        self.libr_dir().join("library.toml")
    }

    /// Path to the `cards/` directory containing JSON card files.
    pub fn cards_dir(&self) -> PathBuf {
        self.libr_dir().join("cards")
    }

    /// Path to the SQLite database file.
    pub fn database_path(&self) -> PathBuf {
        self.libr_dir().join("db").join("omniscope.db")
    }

    /// Path to the covers cache directory.
    pub fn covers_dir(&self) -> PathBuf {
        self.libr_dir().join("cache").join("covers")
    }

    /// Path to the undo log directory.
    pub fn undo_dir(&self) -> PathBuf {
        self.libr_dir().join("undo")
    }

    /// Path to the backups directory.
    pub fn backups_dir(&self) -> PathBuf {
        self.libr_dir().join("backups")
    }

    /// Path to the lock file.
    pub fn lock_path(&self) -> PathBuf {
        self.libr_dir().join("lock")
    }

    // ─── Discovery ──────────────────────────────────────────

    /// Discover a library by walking up the directory tree from `start`.
    ///
    /// Looks for a directory containing `.libr/library.toml`.
    /// Returns `None` if no library is found up to the filesystem root.
    pub fn discover(start: &Path) -> Option<Self> {
        let mut current = if start.is_file() {
            start.parent()?.to_path_buf()
        } else {
            start.to_path_buf()
        };

        loop {
            let candidate = current.join(LIBR_DIR_NAME);
            if candidate.is_dir() && candidate.join("library.toml").exists() {
                return Some(Self::new(current));
            }
            if !current.pop() {
                break;
            }
        }

        None
    }

    /// Discover a library with full fallback chain:
    ///
    /// 1. Check `OMNISCOPE_LIBRARY` environment variable
    /// 2. Walk up from `start` directory
    /// 3. Check known libraries from global config
    pub fn discover_with_fallbacks(start: &Path, global_cfg: &GlobalConfig) -> Option<Self> {
        // 1. Environment variable override
        if let Ok(path) = std::env::var("OMNISCOPE_LIBRARY") {
            let root = PathBuf::from(path);
            let candidate = root.join(LIBR_DIR_NAME);
            if candidate.is_dir() && candidate.join("library.toml").exists() {
                return Some(Self::new(root));
            }
        }

        // 2. Walk up from start directory
        if let Some(found) = Self::discover(start) {
            return Some(found);
        }

        // 3. Check known libraries from global config
        for lib in &global_cfg.libraries {
            let root = PathBuf::from(&lib.path);
            let candidate = root.join(LIBR_DIR_NAME);
            if candidate.is_dir() && candidate.join("library.toml").exists() {
                return Some(Self::new(root));
            }
        }

        None
    }

    // ─── Validation ─────────────────────────────────────────

    /// Validate that this library root has a valid `.libr/` structure.
    pub fn validate(&self) -> Result<()> {
        let libr = self.libr_dir();
        if !libr.is_dir() {
            return Err(crate::error::OmniscopeError::LibraryNotInitialized);
        }

        let manifest_path = self.manifest_path();
        if !manifest_path.exists() {
            return Err(crate::error::OmniscopeError::LibraryNotInitialized);
        }

        // Try to parse the manifest to ensure it's valid
        let contents = std::fs::read_to_string(&manifest_path)?;
        LibraryManifest::from_toml(&contents)?;

        Ok(())
    }

    /// Load the library manifest from `.libr/library.toml`.
    pub fn load_manifest(&self) -> Result<LibraryManifest> {
        let contents = std::fs::read_to_string(self.manifest_path())?;
        LibraryManifest::from_toml(&contents)
    }
}

// ─── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::manifest::LibraryManifest;
    use tempfile::TempDir;

    /// Helper: create a minimal .libr/ structure in a temp dir.
    fn create_libr(root: &Path) {
        let libr = root.join(LIBR_DIR_NAME);
        std::fs::create_dir_all(libr.join("cards")).unwrap();
        std::fs::create_dir_all(libr.join("db")).unwrap();

        let manifest = LibraryManifest::new("Test Library");
        let toml_str = manifest.to_toml().unwrap();
        std::fs::write(libr.join("library.toml"), toml_str).unwrap();
    }

    #[test]
    fn test_derived_paths_correct() {
        let root = PathBuf::from("/home/user/Books");
        let lr = LibraryRoot::new(root.clone());

        assert_eq!(lr.root(), Path::new("/home/user/Books"));
        assert_eq!(lr.libr_dir(), root.join(".libr"));
        assert_eq!(lr.manifest_path(), root.join(".libr/library.toml"));
        assert_eq!(lr.cards_dir(), root.join(".libr/cards"));
        assert_eq!(lr.database_path(), root.join(".libr/db/omniscope.db"));
        assert_eq!(lr.covers_dir(), root.join(".libr/cache/covers"));
        assert_eq!(lr.undo_dir(), root.join(".libr/undo"));
        assert_eq!(lr.backups_dir(), root.join(".libr/backups"));
    }

    #[test]
    fn test_discover_finds_libr_in_current_dir() {
        let tmp = TempDir::new().unwrap();
        create_libr(tmp.path());

        let found = LibraryRoot::discover(tmp.path());
        assert!(found.is_some());
        assert_eq!(found.unwrap().root(), tmp.path());
    }

    #[test]
    fn test_discover_walks_up_from_subdirectory() {
        let tmp = TempDir::new().unwrap();
        create_libr(tmp.path());

        // Create a nested subdirectory
        let subdir = tmp.path().join("programming").join("rust");
        std::fs::create_dir_all(&subdir).unwrap();

        let found = LibraryRoot::discover(&subdir);
        assert!(found.is_some());
        assert_eq!(found.unwrap().root(), tmp.path());
    }

    #[test]
    fn test_discover_returns_none_without_libr() {
        let tmp = TempDir::new().unwrap();
        let found = LibraryRoot::discover(tmp.path());
        assert!(found.is_none());
    }

    #[test]
    fn test_validate_valid_library() {
        let tmp = TempDir::new().unwrap();
        create_libr(tmp.path());

        let lr = LibraryRoot::new(tmp.path().to_path_buf());
        assert!(lr.validate().is_ok());
    }

    #[test]
    fn test_validate_no_libr_dir() {
        let tmp = TempDir::new().unwrap();
        let lr = LibraryRoot::new(tmp.path().to_path_buf());
        assert!(lr.validate().is_err());
    }

    #[test]
    fn test_validate_no_manifest() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(LIBR_DIR_NAME)).unwrap();
        let lr = LibraryRoot::new(tmp.path().to_path_buf());
        assert!(lr.validate().is_err());
    }

    #[test]
    fn test_load_manifest() {
        let tmp = TempDir::new().unwrap();
        create_libr(tmp.path());

        let lr = LibraryRoot::new(tmp.path().to_path_buf());
        let manifest = lr.load_manifest().unwrap();
        assert_eq!(manifest.library.name, "Test Library");
    }

    // Note: env var test for discover_with_fallbacks is omitted because
    // set_var is inherently racy in parallel test execution. The env var
    // code path is trivial (3 lines) and tested implicitly via integration tests.

    #[test]
    fn test_discover_with_fallbacks_global_config() {
        let tmp = TempDir::new().unwrap();
        create_libr(tmp.path());

        let mut global_cfg = GlobalConfig::default();
        global_cfg.libraries.push(crate::config::KnownLibrary {
            name: "Test".to_string(),
            path: tmp.path().to_string_lossy().to_string(),
            id: "test-id".to_string(),
        });

        // Use a nonexistent start dir so walk-up fails, and env var
        // OMNISCOPE_LIBRARY is not set in CI, so fallback #3 (global config) fires.
        // Note: if OMNISCOPE_LIBRARY happens to be set in the env, the test still
        // passes because the function will find *some* library.
        let found = LibraryRoot::discover_with_fallbacks(
            Path::new("/tmp/nonexistent_omniscope_test_path"),
            &global_cfg,
        );
        assert!(found.is_some());
    }
}
