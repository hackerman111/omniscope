use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Root application configuration, loaded from `~/.config/omniscope/config.toml`.
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

// ─── Load / Save ───────────────────────────────────────────

impl AppConfig {
    /// Standard config file path: `~/.config/omniscope/config.toml`
    pub fn config_path() -> PathBuf {
        // Allow override via env var
        if let Ok(path) = std::env::var("OMNISCOPE_CONFIG") {
            return PathBuf::from(path);
        }

        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("omniscope")
            .join("config.toml")
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

    // ─── Derived paths ─────────────────────────────────────

    /// Path to the cards directory.
    pub fn cards_dir(&self) -> PathBuf {
        PathBuf::from(&self.core.library_path).join("db").join("cards")
    }

    /// Path to the SQLite database file.
    pub fn database_path(&self) -> PathBuf {
        PathBuf::from(&self.core.library_path)
            .join("db")
            .join("omniscope.db")
    }

    /// Path to the covers cache directory.
    pub fn covers_dir(&self) -> PathBuf {
        PathBuf::from(&self.core.library_path).join("covers")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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
}
