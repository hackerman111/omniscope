pub trait ExternalSource: Send + Sync {
    fn name(&self) -> &str;
}

pub struct SearchResult {
    pub title: String,
    pub authors: Vec<String>,
    pub year: Option<i32>,
    pub identifier: Option<String>,
    pub source: String,
    pub relevance_score: f32,
}

pub struct DownloadUrl {
    pub url: String,
    pub source_name: String,
    pub requires_redirect: bool,
}

#[derive(Debug, Clone)]
pub enum SourceType {
    AcademicMetadata,
    BookMetadata,
    Search,
    Download,
    OpenAccess,
}

#[derive(Debug, Clone)]
pub struct SourceStatus {
    pub available: bool,
    pub latency_ms: Option<u64>,
    pub last_checked: Option<chrono::DateTime<chrono::Utc>>,
    pub mirror: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RateLimit {
    pub requests_per_second: f32,
}

pub mod crossref;
pub mod semantic_scholar;
pub mod openalex;
pub mod unpaywall;
pub mod openlibrary;
pub mod core_ac;
pub mod annas_archive;
pub mod scihub;
