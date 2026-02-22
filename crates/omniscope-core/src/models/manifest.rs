use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Schema version for .libr/ directory structure.
/// Increment when the layout of .libr/ changes in a backward-incompatible way.
pub const SCHEMA_VERSION: u32 = 1;

/// The `library.toml` manifest that lives at `.libr/library.toml`.
///
/// This is the identity document for a library — it records the library's
/// name, unique ID, schema version, and any per-library setting overrides.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryManifest {
    pub library: LibraryMeta,

    #[serde(default)]
    pub settings: LibrarySettings,
}

/// Core identity fields for a library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryMeta {
    /// Human-readable name (e.g. "My Library").
    pub name: String,

    /// Globally-unique ULID for this library (stable across moves/renames).
    pub id: String,

    /// Schema version of the `.libr/` layout.
    pub version: u32,

    /// When this library was first initialized.
    pub created_at: DateTime<Utc>,

    /// Version of omniscope that created this library.
    pub omniscope_version: String,

    /// Additional root directories that belong to this same library.
    #[serde(default)]
    pub roots: ExtraRoots,
}

/// Additional root directories that belong to the same logical library.
///
/// Allows a single `.libr/` to track books across multiple directories,
/// e.g. an external drive or a separate "Papers" folder.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtraRoots {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<String>,
}

/// Per-library setting overrides (merged on top of global config).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibrarySettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_viewer_pdf: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,

    #[serde(default)]
    pub auto_index: bool,

    #[serde(default)]
    pub watcher: WatcherConfig,
}

/// Configuration for the automatic filesystem watcher
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherConfig {
    /// Automatically import new files (create instances)
    pub auto_import: bool,
    /// Debounce delay before processing filesystem events
    pub debounce_ms: u64,
    /// Minimum file size to consider an import valid
    pub min_file_size_bytes: u64,
    /// File extensions to automatically track
    pub watch_extensions: Vec<String>,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            auto_import: false, // Default strictly ignores auto import by default
            debounce_ms: 2000,
            min_file_size_bytes: 1024,
            watch_extensions: vec![
                "pdf".to_string(),
                "epub".to_string(),
                "djvu".to_string(),
                "fb2".to_string(),
                "mobi".to_string(),
                "azw3".to_string(),
                "cbz".to_string(),
                "cbr".to_string(),
            ],
        }
    }
}

impl LibraryManifest {
    /// Create a new manifest with the given name.
    /// The ID is generated automatically as a ULID.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            library: LibraryMeta {
                name: name.into(),
                id: ulid::Ulid::new().to_string(),
                version: SCHEMA_VERSION,
                created_at: Utc::now(),
                omniscope_version: env!("CARGO_PKG_VERSION").to_string(),
                roots: ExtraRoots::default(),
            },
            settings: LibrarySettings::default(),
        }
    }

    /// Serialize this manifest to TOML.
    pub fn to_toml(&self) -> crate::error::Result<String> {
        let s = toml::to_string_pretty(self)?;
        Ok(s)
    }

    /// Deserialize a manifest from TOML.
    pub fn from_toml(s: &str) -> crate::error::Result<Self> {
        let m: Self = toml::from_str(s)?;
        Ok(m)
    }
}

// ─── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_new() {
        let m = LibraryManifest::new("My Library");
        assert_eq!(m.library.name, "My Library");
        assert_eq!(m.library.version, SCHEMA_VERSION);
        assert!(!m.library.id.is_empty());
        assert!(m.library.roots.extra.is_empty());
    }

    #[test]
    fn test_manifest_toml_roundtrip() {
        let m = LibraryManifest::new("Test Library");
        let toml_str = m.to_toml().unwrap();
        let restored = LibraryManifest::from_toml(&toml_str).unwrap();

        assert_eq!(restored.library.name, "Test Library");
        assert_eq!(restored.library.id, m.library.id);
        assert_eq!(restored.library.version, m.library.version);
    }

    #[test]
    fn test_manifest_defaults() {
        let m = LibraryManifest::new("X");
        assert!(!m.settings.auto_index);
        assert!(m.settings.language.is_none());
        assert!(m.settings.default_viewer_pdf.is_none());
    }

    #[test]
    fn test_manifest_with_extra_roots() {
        let mut m = LibraryManifest::new("Multi-root");
        m.library.roots.extra = vec![
            "/media/external/Books".to_string(),
            "/home/user/Papers".to_string(),
        ];

        let toml_str = m.to_toml().unwrap();
        let restored = LibraryManifest::from_toml(&toml_str).unwrap();
        assert_eq!(restored.library.roots.extra.len(), 2);
        assert_eq!(restored.library.roots.extra[0], "/media/external/Books");
    }
}
