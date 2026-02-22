use std::time::Duration;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use scraper::{Html, Selector};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use crate::error::{Result, ScienceError};
use crate::http::{RateLimitedClient, DiskCache};
use crate::sources::{ExternalSource, SourceType, RateLimit, SearchResult, Metadata, DownloadUrl, SourceStatus};
use crate::identifiers::doi::Doi;
use async_trait::async_trait;

const MIRRORS: &[&str] = &[
    "https://sci-hub.se",
    "https://sci-hub.st",
    "https://sci-hub.ru",
    "https://sci-hub.ren",
    "https://sci-hub.mksa.top",
];

pub struct SciHubSource {
    client: RateLimitedClient,
    cache: DiskCache,
    active_mirror: Arc<RwLock<Option<String>>>,
}

impl SciHubSource {
    pub fn new() -> Self {
        Self::with_params(Duration::from_secs(2))
    }

    pub fn with_params(min_interval: Duration) -> Self {
        let client = RateLimitedClient::new(min_interval, 3, "omniscope/0.1");
        let cache = DiskCache::new("scihub", Duration::from_secs(24 * 3600)); // 24h cache
        
        Self {
            client,
            cache,
            active_mirror: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn init(&self) -> Result<()> {
        for mirror in MIRRORS {
            if self.client.get(mirror).await.is_ok() {
                let mut active = self.active_mirror.write().await;
                *active = Some(mirror.to_string());
                tracing::info!("Initialized Sci-Hub with mirror: {}", mirror);
                return Ok(());
            }
        }
        tracing::warn!("No Sci-Hub mirrors available during init");
        Ok(())
    }

    async fn get_or_find_mirror(&self) -> Result<String> {
        {
            let active = self.active_mirror.read().await;
            if let Some(m) = &*active {
                return Ok(m.clone());
            }
        }

        // Try to find one
        for mirror in MIRRORS {
            if self.client.get(mirror).await.is_ok() {
                let mut active = self.active_mirror.write().await;
                *active = Some(mirror.to_string());
                return Ok(mirror.to_string());
            }
        }

        Err(ScienceError::ApiError("SciHub".to_string(), "No available mirrors".to_string()))
    }

    pub async fn fetch_by_doi(&self, doi: &Doi) -> Result<Option<SciHubResult>> {
        let key = format!("doi:{}", doi.normalized);
        if let Some(cached) = self.cache.get::<SciHubResult>(&key).await {
            return Ok(Some(cached));
        }

        let mirror = self.get_or_find_mirror().await?;
        let url = format!("{}/{}", mirror, doi.normalized);
        
        let html = self.client.get(&url).await?;
        if html.contains("article not found") {
            return Ok(None);
        }

        let result = self.parse_scihub_page(&html, &mirror)?;
        self.cache.set(&key, &result).await;
        Ok(Some(result))
    }

    pub async fn download_pdf(&self, doi: &Doi, output_dir: &Path, filename: &str) -> Result<PathBuf> {
        let info = self.fetch_by_doi(doi).await?
            .ok_or_else(|| ScienceError::SourceUnavailable("Paper not found on Sci-Hub".to_string()))?;
            
        let pdf_url = info.pdf_url.ok_or_else(|| ScienceError::SourceUnavailable("No PDF URL found".to_string()))?;
        
        let path = output_dir.join(filename);
        if path.exists() {
            return Ok(path);
        }

        let mut response = self.client.get_response(&pdf_url).await?;
        let mut file = tokio::fs::File::create(&path).await
            .map_err(|e| ScienceError::Parse(format!("Failed to create file: {}", e)))?;

        while let Some(chunk) = response.chunk().await.map_err(ScienceError::Http)? {
            file.write_all(&chunk).await.map_err(|e| ScienceError::Parse(format!("Write failed: {}", e)))?;
        }

        Ok(path)
    }

    fn parse_scihub_page(&self, html: &str, mirror_base: &str) -> Result<SciHubResult> {
        let document = Html::parse_document(html);
        
        let pdf_selector = Selector::parse("#pdf").unwrap();
        let src_opt = document.select(&pdf_selector)
            .next()
            .and_then(|el| el.value().attr("src"));

        let pdf_url = if let Some(src) = src_opt {
            if src.starts_with("//") {
                Some(format!("https:{}", src))
            } else if src.starts_with("/") {
                Some(format!("{}{}", mirror_base, src))
            } else {
                Some(src.to_string())
            }
        } else {
            // Try embed
            let embed_selector = Selector::parse("embed[type='application/pdf']").unwrap();
            document.select(&embed_selector)
                .next()
                .and_then(|el| el.value().attr("src"))
                .map(|src| {
                     if src.starts_with("//") {
                        format!("https:{}", src)
                    } else if src.starts_with("/") {
                        format!("{}{}", mirror_base, src)
                    } else {
                        src.to_string()
                    }
                })
        };

        let title_selector = Selector::parse("#citation").unwrap();
        let title = document.select(&title_selector)
            .next()
            .map(|el| el.text().collect::<String>());

        Ok(SciHubResult {
            pdf_url,
            title,
        })
    }
}

impl Default for SciHubSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExternalSource for SciHubSource {
    fn name() -> &'static str { "SciHub" }
    fn source_type() -> SourceType { SourceType::Download }
    fn requires_auth() -> bool { false }
    fn rate_limit() -> RateLimit { RateLimit { requests_per_second: 0.5 } }

    async fn search(&self, _query: &str) -> Result<Vec<SearchResult>> {
        Ok(Vec::new())
    }

    async fn fetch_metadata(&self, _id: &str) -> Result<Option<Metadata>> {
        Ok(None)
    }

    async fn find_download_url(&self, id: &str) -> Result<Option<DownloadUrl>> {
        if let Ok(doi) = Doi::parse(id) {
            if let Ok(Some(info)) = self.fetch_by_doi(&doi).await {
                if let Some(url) = info.pdf_url {
                    return Ok(Some(DownloadUrl {
                        url,
                        source_name: "SciHub".to_string(),
                        requires_redirect: true,
                    }));
                }
            }
        }
        Ok(None)
    }

    async fn health_check(&self) -> SourceStatus {
        let available = self.get_or_find_mirror().await.is_ok();
        let mirror = self.active_mirror.read().await.clone();
        
        SourceStatus {
            available,
            latency_ms: None,
            last_checked: Some(chrono::Utc::now()),
            mirror,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SciHubResult {
    pub pdf_url: Option<String>,
    pub title: Option<String>,
}
