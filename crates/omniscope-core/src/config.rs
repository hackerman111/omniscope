use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;

// ─── GlobalConfig ──────────────────────────────────────────
// Lives at ~/.config/omniscope/config.toml
// Contains ONLY global settings + library registry.
// Per-library settings live in .libr/library.toml.

/// Entry in the global library registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownLibrary {
    pub name: String,
    pub path: String,
    pub id: String,
}

/// Global settings (theme, editor, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GlobalSettings {
    pub theme: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_editor: Option<String>,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            theme: "catppuccin-mocha".to_string(),
            default_editor: None,
        }
    }
}

/// Global configuration: `~/.config/omniscope/config.toml`.
///
/// This contains settings that are NOT per-library:
/// - UI theme, editor preferences
/// - AI provider credentials
/// - Search engine settings
/// - Known library registry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GlobalConfig {
    pub global: GlobalSettings,
    pub ui: UiConfig,
    pub ai: AiConfig,
    pub search: SearchConfig,
    pub viewers: ViewersConfig,
    pub server: ServerConfig,
    #[serde(default)]
    pub libraries: Vec<KnownLibrary>,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            global: GlobalSettings::default(),
            ui: UiConfig::default(),
            ai: AiConfig::default(),
            search: SearchConfig::default(),
            viewers: ViewersConfig::default(),
            server: ServerConfig::default(),
            libraries: Vec::new(),
        }
    }
}

impl GlobalConfig {
    /// Standard global config file path: `~/.config/omniscope/config.toml`.
    pub fn config_path() -> PathBuf {
        if let Ok(path) = std::env::var("OMNISCOPE_CONFIG") {
            return PathBuf::from(path);
        }
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("omniscope")
            .join("config.toml")
    }

    /// Load global config from disk, falling back to defaults.
    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        Self::load_from(&path)
    }

    /// Load from a specific path.
    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Save global config.
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        self.save_to(&path)
    }

    /// Save to a specific path.
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let toml_str = toml::to_string_pretty(self)?;
        std::fs::write(path, toml_str)?;
        Ok(())
    }

    // ─── Library registry operations ────────────────────────

    /// Add a library to the known libraries registry.
    pub fn add_library(&mut self, name: &str, path: &Path, id: &str) {
        // Remove any existing entry with the same path
        self.libraries.retain(|l| l.path != path.to_string_lossy().as_ref());
        self.libraries.push(KnownLibrary {
            name: name.to_string(),
            path: path.to_string_lossy().to_string(),
            id: id.to_string(),
        });
    }

    /// Remove a library from the registry by path.
    pub fn remove_library(&mut self, path: &Path) {
        let path_str = path.to_string_lossy();
        self.libraries.retain(|l| l.path != path_str.as_ref());
    }

    /// Update a library path (e.g. after `mv` of the library directory).
    pub fn update_library_path(&mut self, old: &Path, new: &Path) {
        let old_str = old.to_string_lossy();
        for lib in &mut self.libraries {
            if lib.path == old_str.as_ref() {
                lib.path = new.to_string_lossy().to_string();
                break;
            }
        }
    }

    /// Get the list of known libraries.
    pub fn known_libraries(&self) -> &[KnownLibrary] {
        &self.libraries
    }
}

// ─── AppConfig ─────────────────────────────────────────────
// Runtime config that merges GlobalConfig + optional per-library settings.
// This is what the rest of the application actually uses.

/// Root application configuration.
///
/// Legacy compatibility: this struct still has `core.library_path` so that
/// code that hasn't been migrated to `LibraryRoot` yet continues to work.
/// New code should use `LibraryRoot` directly for path derivation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub core: CoreConfig,
    pub ui: UiConfig,
    pub ai: AiConfig,
    pub search: SearchConfig,
    pub viewers: ViewersConfig,
    pub server: ServerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CoreConfig {
    pub library_path: String,
    pub books_directory: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_import_directory: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub theme: String,
    pub image_protocol: String,
    pub show_covers: bool,
    pub list_style: String,
    pub default_sort: String,
    pub panel_sizes: [u16; 3],
    pub fuzzy_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    pub provider: String,
    pub model: String,
    pub api_key_env: String,
    pub max_tokens: u32,
    pub temperature: f64,
    pub auto_index: bool,
    pub auto_summary: bool,
    pub library_map_cache: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchConfig {
    pub fuzzy_engine: String,
    pub frecency_enabled: bool,
    pub semantic_search: bool,
    pub highlight_matches: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewersConfig {
    pub pdf: String,
    pub epub: String,
    pub djvu: String,
    pub mobi: String,
    pub txt: String,
    pub html: String,

    #[serde(default)]
    pub alternatives: HashMap<String, Vec<String>>,

    #[serde(default)]
    pub overrides: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub enabled: bool,
    pub port: u16,
    pub host: String,
    pub auth_token_env: String,
}

// ─── Defaults ──────────────────────────────────────────────

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            core: CoreConfig::default(),
            ui: UiConfig::default(),
            ai: AiConfig::default(),
            search: SearchConfig::default(),
            viewers: ViewersConfig::default(),
            server: ServerConfig::default(),
        }
    }
}

impl Default for CoreConfig {
    fn default() -> Self {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("omniscope");
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));

        Self {
            library_path: data_dir.to_string_lossy().to_string(),
            books_directory: home.join("Books").to_string_lossy().to_string(),
            auto_import_directory: None,
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "catppuccin-mocha".to_string(),
            image_protocol: "auto".to_string(),
            show_covers: true,
            list_style: "detailed".to_string(),
            default_sort: "added_desc".to_string(),
            panel_sizes: [20, 50, 30],
            fuzzy_threshold: 0.6,
        }
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            api_key_env: "ANTHROPIC_API_KEY".to_string(),
            max_tokens: 4096,
            temperature: 0.1,
            auto_index: true,
            auto_summary: false,
            library_map_cache: "1h".to_string(),
        }
    }
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            fuzzy_engine: "nucleo".to_string(),
            frecency_enabled: true,
            semantic_search: false,
            highlight_matches: true,
        }
    }
}

impl Default for ViewersConfig {
    fn default() -> Self {
        Self {
            pdf: "xdg-open".to_string(),
            epub: "xdg-open".to_string(),
            djvu: "xdg-open".to_string(),
            mobi: "xdg-open".to_string(),
            txt: "$EDITOR".to_string(),
            html: "$BROWSER".to_string(),
            alternatives: HashMap::new(),
            overrides: HashMap::new(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 8080,
            host: "127.0.0.1".to_string(),
            auth_token_env: "OMNISCOPE_TOKEN".to_string(),
        }
    }
}

// ─── AppConfig: Load / Save / Paths ────────────────────────

impl AppConfig {
    /// Standard config file path: `~/.config/omniscope/config.toml`
    pub fn config_path() -> PathBuf {
        GlobalConfig::config_path()
    }

    /// Load config from disk, falling back to defaults if file doesn't exist.
    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        Self::load_from(&path)
    }

    /// Load config from a specific path.
    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Save config to the standard path.
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        self.save_to(&path)
    }

    /// Save config to a specific path.
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let toml_str = toml::to_string_pretty(self)?;
        std::fs::write(path, toml_str)?;
        Ok(())
    }

    /// Construct AppConfig from a GlobalConfig (taking UI/AI/etc fields).
    pub fn from_global(global: &GlobalConfig) -> Self {
        Self {
            core: CoreConfig::default(),
            ui: global.ui.clone(),
            ai: global.ai.clone(),
            search: global.search.clone(),
            viewers: global.viewers.clone(),
            server: global.server.clone(),
        }
    }

    // ─── Derived paths (legacy — prefer LibraryRoot) ────────

    /// Path to the cards directory.
    ///
    /// **Legacy**: new code should use `LibraryRoot::cards_dir()`.
    pub fn cards_dir(&self) -> PathBuf {
        PathBuf::from(&self.core.library_path).join("db").join("cards")
    }

    /// Path to the SQLite database file.
    ///
    /// **Legacy**: new code should use `LibraryRoot::database_path()`.
    pub fn database_path(&self) -> PathBuf {
        PathBuf::from(&self.core.library_path)
            .join("db")
            .join("omniscope.db")
    }

    /// Path to the covers cache directory.
    ///
    /// **Legacy**: new code should use `LibraryRoot::covers_dir()`.
    pub fn covers_dir(&self) -> PathBuf {
        PathBuf::from(&self.core.library_path).join("covers")
    }

    /// Returns the library root path.
    pub fn library_path(&self) -> PathBuf {
        PathBuf::from(&self.core.library_path)
    }

    /// Override the library root path (used for OMNISCOPE_LIBRARY_PATH env var).
    pub fn set_library_path(&mut self, path: PathBuf) {
        self.core.library_path = path.to_string_lossy().to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ─── AppConfig tests (legacy compat) ────────────────────

    #[test]
    fn test_default_config_is_valid() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.ui.theme, "catppuccin-mocha");
        assert_eq!(cfg.server.port, 8080);
        assert!(!cfg.core.library_path.is_empty());
    }

    #[test]
    fn test_config_toml_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");

        let cfg = AppConfig::default();
        cfg.save_to(&path).unwrap();

        let loaded = AppConfig::load_from(&path).unwrap();
        assert_eq!(loaded.ui.theme, cfg.ui.theme);
        assert_eq!(loaded.server.port, cfg.server.port);
        assert_eq!(loaded.ai.provider, cfg.ai.provider);
    }

    #[test]
    fn test_load_nonexistent_returns_default() {
        let cfg = AppConfig::load_from(Path::new("/tmp/nonexistent_omniscope_config.toml")).unwrap();
        assert_eq!(cfg.ui.theme, "catppuccin-mocha");
    }

    #[test]
    fn test_derived_paths() {
        let cfg = AppConfig::default();
        let cards = cfg.cards_dir();
        assert!(cards.to_string_lossy().contains("cards"));
        let db = cfg.database_path();
        assert!(db.to_string_lossy().contains("omniscope.db"));
    }

    // ─── GlobalConfig tests ─────────────────────────────────

    #[test]
    fn test_global_config_default() {
        let gc = GlobalConfig::default();
        assert_eq!(gc.global.theme, "catppuccin-mocha");
        assert!(gc.libraries.is_empty());
    }

    #[test]
    fn test_global_config_toml_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("global.toml");

        let mut gc = GlobalConfig::default();
        gc.add_library("Test", Path::new("/home/user/Books"), "ulid-123");
        gc.save_to(&path).unwrap();

        let loaded = GlobalConfig::load_from(&path).unwrap();
        assert_eq!(loaded.libraries.len(), 1);
        assert_eq!(loaded.libraries[0].name, "Test");
        assert_eq!(loaded.libraries[0].path, "/home/user/Books");
        assert_eq!(loaded.libraries[0].id, "ulid-123");
    }

    #[test]
    fn test_global_config_add_library_dedupes() {
        let mut gc = GlobalConfig::default();
        gc.add_library("First", Path::new("/Books"), "id1");
        gc.add_library("Second", Path::new("/Books"), "id2");
        // Same path should replace
        assert_eq!(gc.libraries.len(), 1);
        assert_eq!(gc.libraries[0].name, "Second");
    }

    #[test]
    fn test_global_config_remove_library() {
        let mut gc = GlobalConfig::default();
        gc.add_library("Test", Path::new("/Books"), "id1");
        assert_eq!(gc.libraries.len(), 1);

        gc.remove_library(Path::new("/Books"));
        assert!(gc.libraries.is_empty());
    }

    #[test]
    fn test_global_config_update_path() {
        let mut gc = GlobalConfig::default();
        gc.add_library("Lib", Path::new("/old/path"), "id1");

        gc.update_library_path(Path::new("/old/path"), Path::new("/new/path"));
        assert_eq!(gc.libraries[0].path, "/new/path");
        assert_eq!(gc.libraries[0].name, "Lib"); // name unchanged
    }

    #[test]
    fn test_app_config_from_global() {
        let mut gc = GlobalConfig::default();
        gc.ui.theme = "gruvbox-dark".to_string();
        gc.ai.provider = "ollama".to_string();

        let app = AppConfig::from_global(&gc);
        assert_eq!(app.ui.theme, "gruvbox-dark");
        assert_eq!(app.ai.provider, "ollama");
    }
}
