use crate::error::Result;
use async_trait::async_trait;

#[async_trait]
pub trait ExternalSource: Send + Sync {
    fn name() -> &'static str where Self: Sized;
    fn source_type() -> SourceType where Self: Sized;
    fn requires_auth() -> bool where Self: Sized;
    fn rate_limit() -> RateLimit where Self: Sized;

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>>;
    async fn fetch_metadata(&self, id: &str) -> Result<Option<Metadata>>;
    async fn find_download_url(&self, id: &str) -> Result<Option<DownloadUrl>>;
    async fn health_check(&self) -> SourceStatus;
}

#[derive(Debug, Clone)]
pub struct Metadata {
    pub title: String,
    pub authors: Vec<String>,
    pub year: Option<i32>,
    pub abstract_text: Option<String>,
    pub doi: Option<String>,
    pub isbn: Option<String>,
    pub publisher: Option<String>,
    pub journal: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub pages: Option<String>,
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub authors: Vec<String>,
    pub year: Option<i32>,
    pub identifier: Option<String>,
    pub source: String,
    pub relevance_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
