use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScienceConfig {
    pub polite_pool_email: Option<String>,
    pub semantic_scholar_api_key: Option<String>,
    pub core_api_key: Option<String>,
    pub auto_extract_doi_from_pdf: bool,
    pub preferred_pdf_sources: Vec<String>,
    pub download_directory: Option<std::path::PathBuf>,
    pub rename_scheme: Option<String>,
    pub scihub: SciHubConfig,
    pub annas_archive: AnnasArchiveConfig,
    pub export: ExportConfig,
    pub citation_graph: CitationGraphConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SciHubConfig {
    pub enabled: bool,
    pub mirror_check_on_startup: bool,
    pub preferred_mirrors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnnasArchiveConfig {
    pub enabled: bool,
    pub preferred_formats: Vec<String>,
    pub preferred_languages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExportConfig {
    pub default_cite_style: String,
    pub cite_key_scheme: String,
    pub bibtex_utf8: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CitationGraphConfig {
    pub fetch_on_add: bool,
    pub fetch_depth: u32,
    pub max_citations_to_store: u32,
}

impl ScienceConfig {
    pub fn load() -> crate::error::Result<Self> {
        // In a real app, this would load from a file (e.g., ~/.config/omniscope/science.toml)
        // For now, return default.
        Ok(Self::default())
    }
}
