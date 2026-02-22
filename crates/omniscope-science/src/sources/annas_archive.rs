use std::time::Duration;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use scraper::{Html, Selector};
use crate::error::{Result, ScienceError};
use crate::http::{RateLimitedClient, DiskCache};
use crate::sources::{ExternalSource, SourceType, RateLimit, SearchResult, Metadata, DownloadUrl, SourceStatus};
use async_trait::async_trait;
use regex::Regex;
use once_cell::sync::Lazy;

const MIRRORS: &[&str] = &[
    "https://annas-archive.org",
    "https://annas-archive.se",
    "https://annas-archive.li",
    "https://annas-archive.gs",
];

static YEAR_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(19|20)\d{2}\b").unwrap());
static SIZE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(\d+(\.\d+)?)\s*(MB|KB|GB)").unwrap());

pub struct AnnasArchiveSource {
    client: RateLimitedClient,
    cache: DiskCache,
    active_mirror: Arc<RwLock<String>>,
}

impl AnnasArchiveSource {
    pub fn new() -> Self {
        Self::with_params(Duration::from_secs(2))
    }

    pub fn with_params(min_interval: Duration) -> Self {
        let client = RateLimitedClient::new(min_interval, 3, "omniscope/0.1");
        let cache = DiskCache::new("annas_archive", Duration::from_secs(24 * 3600));
        
        Self {
            client,
            cache,
            active_mirror: Arc::new(RwLock::new(MIRRORS[0].to_string())),
        }
    }

    async fn get_mirror(&self) -> String {
        self.active_mirror.read().await.clone()
    }

    async fn rotate_mirror(&self) {
        let mut active = self.active_mirror.write().await;
        if let Some(pos) = MIRRORS.iter().position(|&m| m == *active) {
            let next = (pos + 1) % MIRRORS.len();
            *active = MIRRORS[next].to_string();
            tracing::info!("Rotated Anna's Archive mirror to: {}", *active);
        }
    }

    async fn fetch_with_mirror_rotation(&self, path: &str) -> Result<String> {
        let mut attempts = 0;
        let max_attempts = MIRRORS.len();

        while attempts < max_attempts {
            let mirror = self.get_mirror().await;
            let url = format!("{}{}", mirror, path);
            
            match self.client.get(&url).await {
                Ok(html) => return Ok(html),
                Err(e) => {
                    tracing::warn!("Failed to fetch from {}: {}", mirror, e);
                    self.rotate_mirror().await;
                    attempts += 1;
                }
            }
        }
        
        Err(ScienceError::ApiError("AnnasArchive".to_string(), "All mirrors failed".to_string()))
    }

    pub async fn search_advanced(&self, query: &AnnasQuery) -> Result<Vec<AnnasResult>> {
        let mut query_params = vec![
            ("q", query.q.clone()),
            ("sort", query.sort.as_str().to_string()),
        ];
        
        if let Some(content) = &query.content {
            query_params.push(("content", content.clone()));
        }
        
        if let Some(ext) = &query.ext {
            query_params.push(("ext", ext.join(",")));
        }
        
        if let Some(lang) = &query.lang {
            query_params.push(("lang", lang.clone()));
        }
        
        let qs = serde_urlencoded::to_string(&query_params).map_err(|e| ScienceError::Parse(e.to_string()))?;
        let path = format!("/search?{}", qs);
        
        let cache_key = format!("search:{}", qs);
        if let Some(cached) = self.cache.get::<Vec<AnnasResult>>(&cache_key).await {
            return Ok(cached);
        }

        let html = self.fetch_with_mirror_rotation(&path).await?;
        let results = self.parse_search_html(&html)?;
        
        self.cache.set(&cache_key, &results).await;
        Ok(results)
    }

    pub async fn get_download_links(&self, md5: &str) -> Result<Vec<DownloadLink>> {
        let path = format!("/md5/{}", md5);
        let html = self.fetch_with_mirror_rotation(&path).await?;
        self.parse_download_links(&html)
    }

    fn parse_search_html(&self, html: &str) -> Result<Vec<AnnasResult>> {
        let document = Html::parse_document(html);
        let md5_link_selector = Selector::parse("a[href^='/md5/']").unwrap();
        
        let mut results = Vec::new();
        
        for link in document.select(&md5_link_selector) {
            let href = link.value().attr("href").unwrap_or("");
            let md5 = href.trim_start_matches("/md5/").to_string();
            let text = link.text().collect::<Vec<_>>().join(" ");
            
            // Basic extraction from text which usually contains "Title Author (Year) [Format] ..."
            results.push(AnnasResult {
                title: text.clone(), // This is still fuzzy without proper selectors, but matches current capabilities
                authors: Vec::new(),
                year: parse_year_from_meta(&text),
                format: parse_format_from_meta(&text),
                size_mb: parse_size_from_meta(&text),
                language: parse_lang_from_meta(&text),
                md5: Some(md5),
                detail_url: href.to_string(),
                isbn: None,
                publisher: None,
                cover_url: None,
            });
        }
        
        results.sort_by(|a, b| a.md5.cmp(&b.md5));
        results.dedup_by(|a, b| a.md5.is_some() && a.md5 == b.md5);
        
        Ok(results)
    }

    fn parse_download_links(&self, html: &str) -> Result<Vec<DownloadLink>> {
        let document = Html::parse_document(html);
        let link_selector = Selector::parse("a").unwrap();
        let mut links = Vec::new();

        for link in document.select(&link_selector) {
            if let Some(href) = link.value().attr("href") {
                let text = link.text().collect::<String>().to_lowercase();
                
                let (source, priority) = if text.contains("libgen") {
                    ("Libgen", 90)
                } else if text.contains("scihub") {
                    ("Sci-Hub", 80)
                } else if text.contains("ipfs") {
                    ("IPFS", 50)
                } else if text.contains("annas archive") || text.contains("download") {
                    ("Anna's Archive", 70)
                } else {
                    continue;
                };

                // Normalize URL
                let url = if href.starts_with('/') {
                    let mirror = "https://annas-archive.org"; // Fallback base
                    format!("{}{}", mirror, href)
                } else {
                    href.to_string()
                };

                links.push(DownloadLink {
                    source: source.to_string(),
                    url,
                    priority,
                });
            }
        }
        
        // Sort by priority descending
        links.sort_by(|a, b| b.priority.cmp(&a.priority));
        Ok(links)
    }
}

fn parse_year_from_meta(text: &str) -> Option<i32> {
    YEAR_REGEX.captures(text)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse().ok())
}

fn parse_format_from_meta(text: &str) -> String {
    let lower = text.to_lowercase();
    if lower.contains("pdf") { "pdf".to_string() }
    else if lower.contains("epub") { "epub".to_string() }
    else if lower.contains("djvu") { "djvu".to_string() }
    else { "unknown".to_string() }
}

fn parse_size_from_meta(text: &str) -> f64 {
    if let Some(caps) = SIZE_REGEX.captures(text)
        && let Some(val) = caps.get(1).and_then(|m| m.as_str().parse::<f64>().ok())
    {
        let unit = caps.get(3).map(|m| m.as_str()).unwrap_or("");
        return match unit {
            "GB" => val * 1024.0,
            "MB" => val,
            "KB" => val / 1024.0,
            _ => val,
        };
    }
    0.0
}

fn parse_lang_from_meta(text: &str) -> String {
    // Very naive heuristic
    if text.contains("English") || text.contains("[en]") { "en".to_string() }
    else if text.contains("Russian") || text.contains("[ru]") { "ru".to_string() }
    else { "unknown".to_string() }
}

impl Default for AnnasArchiveSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExternalSource for AnnasArchiveSource {
    fn name() -> &'static str { "AnnasArchive" }
    fn source_type() -> SourceType { SourceType::Search }
    fn requires_auth() -> bool { false }
    fn rate_limit() -> RateLimit { RateLimit { requests_per_second: 0.5 } }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let q = AnnasQuery {
            q: query.to_string(),
            ext: None,
            lang: None,
            content: None,
            isbn: None,
            doi: None,
            sort: AnnaSort::Relevance,
        };
        
        let results = self.search_advanced(&q).await?;
        
        Ok(results.into_iter().map(|r| SearchResult {
            title: r.title,
            authors: r.authors,
            year: r.year,
            identifier: r.md5,
            source: "AnnasArchive".to_string(),
            relevance_score: 0.0,
        }).collect())
    }

    async fn fetch_metadata(&self, _id: &str) -> Result<Option<Metadata>> {
        Ok(None)
    }

    async fn find_download_url(&self, _id: &str) -> Result<Option<DownloadUrl>> {
        // Not implemented for generic ID, assumes searching via MD5 directly
        Ok(None)
    }

    async fn health_check(&self) -> SourceStatus {
        let mirror = self.get_mirror().await;
        let available = self.client.get(&mirror).await.is_ok();
        
        SourceStatus {
            available,
            latency_ms: None,
            last_checked: Some(chrono::Utc::now()),
            mirror: Some(mirror),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnasQuery {
    pub q: String,
    pub ext: Option<Vec<String>>,
    pub lang: Option<String>,
    pub content: Option<String>,
    pub isbn: Option<String>,
    pub doi: Option<String>,
    pub sort: AnnaSort,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnnaSort {
    Relevance,
    Newest,
    Oldest,
    Largest,
    Smallest,
}

impl AnnaSort {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Relevance => "",
            Self::Newest => "newest",
            Self::Oldest => "oldest",
            Self::Largest => "largest",
            Self::Smallest => "smallest",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnasResult {
    pub title: String,
    pub authors: Vec<String>,
    pub year: Option<i32>,
    pub format: String,
    pub size_mb: f64,
    pub language: String,
    pub md5: Option<String>,
    pub detail_url: String,
    pub isbn: Option<String>,
    pub publisher: Option<String>,
    pub cover_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadLink {
    pub source: String,
    pub url: String,
    pub priority: u8,
}
