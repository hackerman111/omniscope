use std::time::Duration;
use serde::{Deserialize, Serialize};
use crate::error::{Result, ScienceError};
use crate::http::{RateLimitedClient, DiskCache};
use crate::sources::{ExternalSource, SourceType, RateLimit, SearchResult, Metadata, DownloadUrl, SourceStatus};
use crate::identifiers::doi::Doi;
use async_trait::async_trait;

pub struct CoreSource {
    client: RateLimitedClient,
    cache: DiskCache,
    api_key: String,
    base_url: String,
}

impl CoreSource {
    pub fn new(api_key: String) -> Self {
        Self::with_params(
            "https://api.core.ac.uk/v3",
            Duration::from_millis(500), // CORE API limit
            api_key,
        )
    }

    pub fn with_params(base_url: &str, min_interval: Duration, api_key: String) -> Self {
        let client = RateLimitedClient::new(min_interval, 3, "omniscope/0.1");
        let cache = DiskCache::new("core_ac", Duration::from_secs(7 * 24 * 3600));
        
        Self {
            client,
            cache,
            api_key,
            base_url: base_url.to_string(),
        }
    }

    fn get_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        if !self.api_key.is_empty()
            && let Ok(val) = reqwest::header::HeaderValue::from_str(&format!("Bearer {}", self.api_key))
        {
            headers.insert("Authorization", val);
        }
        headers
    }

    pub async fn fetch_by_doi(&self, doi: &Doi) -> Result<Option<CoreWork>> {
        if self.api_key.is_empty() {
            tracing::warn!("CORE API key not set, skipping fetch");
            return Ok(None);
        }

        let key = format!("doi:{}", doi.normalized);
        if let Some(cached) = self.cache.get::<CoreWork>(&key).await {
            return Ok(Some(cached));
        }

        let url = format!("{}/works/doi:{}", self.base_url, doi.normalized);
        let text = self.client.get_with_headers(&url, self.get_headers()).await?;
        let work: CoreWork = serde_json::from_str(&text).map_err(|e| ScienceError::Parse(e.to_string()))?;
        
        self.cache.set(&key, &work).await;
        Ok(Some(work))
    }

    pub async fn search_works(&self, query: &str) -> Result<Vec<CoreWork>> {
        if self.api_key.is_empty() {
            tracing::warn!("CORE API key not set, skipping search");
            return Ok(Vec::new());
        }

        let url = format!("{}/search/works?q={}&limit=10", self.base_url, urlencoding::encode(query));
        let res: CoreSearchResponse = self.client.get_with_headers(&url, self.get_headers())
            .await
            .and_then(|text| serde_json::from_str(&text).map_err(|e| ScienceError::Parse(e.to_string())))?;
            
        Ok(res.results)
    }
}

#[async_trait]
impl ExternalSource for CoreSource {
    fn name() -> &'static str { "CORE" }
    fn source_type() -> SourceType { SourceType::OpenAccess }
    fn requires_auth() -> bool { true }
    fn rate_limit() -> RateLimit { RateLimit { requests_per_second: 2.0 } }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let works = self.search_works(query).await?;
        Ok(works.into_iter().map(|w| SearchResult {
            title: w.title,
            authors: w.authors.into_iter().map(|a| a.name).collect(),
            year: w.year,
            identifier: w.doi.clone(),
            source: "CORE".to_string(),
            relevance_score: 0.0,
        }).collect())
    }

    async fn fetch_metadata(&self, id: &str) -> Result<Option<Metadata>> {
        if let Ok(doi) = Doi::parse(id)
            && let Some(work) = self.fetch_by_doi(&doi).await?
        {
            return Ok(Some(work.into_metadata()));
        }
        Ok(None)
    }

    async fn find_download_url(&self, id: &str) -> Result<Option<DownloadUrl>> {
        if let Ok(doi) = Doi::parse(id)
            && let Some(work) = self.fetch_by_doi(&doi).await?
            && let Some(url) = work.download_url
        {
            return Ok(Some(DownloadUrl {
                url,
                source_name: "CORE".to_string(),
                requires_redirect: false,
            }));
        }
        Ok(None)
    }

    async fn health_check(&self) -> SourceStatus {
        SourceStatus {
            available: !self.api_key.is_empty(),
            latency_ms: None,
            last_checked: Some(chrono::Utc::now()),
            mirror: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreSearchResponse {
    pub results: Vec<CoreWork>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreWork {
    pub id: i64,
    pub title: String,
    #[serde(default)]
    pub authors: Vec<CoreAuthor>,
    pub abstract_text: Option<String>,
    pub doi: Option<String>,
    pub download_url: Option<String>,
    pub year: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreAuthor {
    pub name: String,
}

impl CoreWork {
    pub fn into_metadata(self) -> Metadata {
        Metadata {
            title: self.title,
            authors: self.authors.into_iter().map(|a| a.name).collect(),
            year: self.year,
            abstract_text: self.abstract_text,
            doi: self.doi,
            isbn: None,
            publisher: None,
            journal: None,
            volume: None,
            issue: None,
            pages: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    #[tokio::test]
    async fn test_core_fetch_by_doi() {
        let mut server = Server::new_async().await;
        let base_url = server.url();

        let _m = server.mock("GET", "/works/doi:10.1038/nature14539")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "id": 12345,
                "title": "Human-level control through deep reinforcement learning",
                "authors": [{"name": "Volodymyr Mnih"}],
                "year": 2015,
                "doi": "10.1038/nature14539",
                "download_url": "https://example.com/core.pdf"
            }"#)
            .create_async().await;

        let source = CoreSource::with_params(
            &base_url,
            Duration::from_secs(0),
            "test-key".to_string(),
        );
        let doi = Doi::parse("10.1038/nature14539").unwrap();
        let result = source.fetch_by_doi(&doi).await.unwrap();

        assert!(result.is_some());
        let work = result.unwrap();
        assert_eq!(work.title, "Human-level control through deep reinforcement learning");
        assert_eq!(work.download_url.unwrap(), "https://example.com/core.pdf");
    }
}
