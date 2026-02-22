use std::time::Duration;
use serde::{Deserialize, Serialize};
use crate::error::Result;
use crate::http::{RateLimitedClient, DiskCache};
use crate::sources::{ExternalSource, SourceType, RateLimit, SearchResult, Metadata, DownloadUrl, SourceStatus};
use crate::identifiers::doi::Doi;
use async_trait::async_trait;

pub struct UnpaywallSource {
    client: RateLimitedClient,
    cache: DiskCache,
    email: String,
    base_url: String,
}

impl UnpaywallSource {
    pub fn new(email: String) -> Self {
        Self::with_params(
            "https://api.unpaywall.org/v2",
            Duration::from_millis(200),
            email,
        )
    }

    pub fn with_params(base_url: &str, min_interval: Duration, email: String) -> Self {
        let client = RateLimitedClient::new(min_interval, 3, "omniscope/0.1");
        let cache = DiskCache::new("unpaywall", Duration::from_secs(7 * 24 * 3600));
        
        Self {
            client,
            cache,
            email,
            base_url: base_url.to_string(),
        }
    }

    pub async fn check_oa(&self, doi: &Doi) -> Result<UnpaywallResult> {
        let key = format!("doi:{}", doi.normalized);
        if let Some(cached) = self.cache.get::<UnpaywallResult>(&key).await {
            return Ok(cached);
        }

        let url = format!("{}/{}?email={}", self.base_url, doi.normalized, self.email);
        let res: UnpaywallResult = self.client.get_json(&url).await?;
        
        self.cache.set(&key, &res).await;
        Ok(res)
    }
}

#[async_trait]
impl ExternalSource for UnpaywallSource {
    fn name() -> &'static str { "Unpaywall" }
    fn source_type() -> SourceType { SourceType::OpenAccess }
    fn requires_auth() -> bool { false } // Uses email but no API key
    fn rate_limit() -> RateLimit { RateLimit { requests_per_second: 5.0 } }

    async fn search(&self, _query: &str) -> Result<Vec<SearchResult>> {
        // Unpaywall is for DOI lookups, not general text search
        Ok(Vec::new())
    }

    async fn fetch_metadata(&self, id: &str) -> Result<Option<Metadata>> {
        if let Ok(doi) = Doi::parse(id) {
            let res = self.check_oa(&doi).await?;
            // Unpaywall provides basic metadata too
            Ok(Some(Metadata {
                title: res.title.unwrap_or_default(),
                authors: Vec::new(), // Unpaywall doesn't return full author list reliably in v2
                year: res.published_date.as_ref().and_then(|d| d.split('-').next()).and_then(|s| s.parse().ok()),
                abstract_text: None,
                doi: Some(res.doi),
                isbn: None,
                publisher: res.publisher,
                journal: res.journal_name,
                volume: None,
                issue: None,
                pages: None,
            }))
        } else {
            Ok(None)
        }
    }

    async fn find_download_url(&self, id: &str) -> Result<Option<DownloadUrl>> {
        if let Ok(doi) = Doi::parse(id) {
            let res = self.check_oa(&doi).await?;
            if let Some(url) = res.best_pdf_url() {
                return Ok(Some(DownloadUrl {
                    url: url.to_string(),
                    source_name: "Unpaywall".to_string(),
                    requires_redirect: false,
                }));
            }
        }
        Ok(None)
    }

    async fn health_check(&self) -> SourceStatus {
        SourceStatus {
            available: true,
            latency_ms: None,
            last_checked: Some(chrono::Utc::now()),
            mirror: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnpaywallResult {
    pub doi: String,
    pub is_oa: bool,
    pub oa_status: String,
    pub title: Option<String>,
    pub publisher: Option<String>,
    pub journal_name: Option<String>,
    pub published_date: Option<String>,
    pub best_oa_location: Option<OaLocation>,
    pub oa_locations: Vec<OaLocation>,
    pub updated: String,
    pub journal_is_oa: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OaLocation {
    pub url: String,
    pub url_for_pdf: Option<String>,
    pub url_for_landing_page: Option<String>,
    pub host_type: String,
    pub license: Option<String>,
    pub version: String,
    pub repository_institution: Option<String>,
}

impl UnpaywallResult {
    pub fn best_pdf_url(&self) -> Option<&str> {
        self.best_oa_location.as_ref()
            .and_then(|loc| loc.url_for_pdf.as_deref().or(Some(&loc.url)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    #[tokio::test]
    async fn test_unpaywall_check_oa() {
        let mut server = Server::new_async().await;
        let base_url = server.url();

        let _m = server.mock("GET", "/10.1038/nature14539?email=test@example.com")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "doi": "10.1038/nature14539",
                "is_oa": true,
                "oa_status": "gold",
                "title": "Human-level control through deep reinforcement learning",
                "best_oa_location": {
                    "url": "https://example.com/paper.pdf",
                    "url_for_pdf": "https://example.com/paper.pdf",
                    "host_type": "publisher",
                    "version": "publishedVersion"
                },
                "oa_locations": [],
                "updated": "2023-01-01T00:00:00Z",
                "journal_is_oa": false
            }"#)
            .create_async().await;

        let source = UnpaywallSource::with_params(
            &base_url,
            Duration::from_secs(0),
            "test@example.com".to_string(),
        );
        let doi = Doi::parse("10.1038/nature14539").unwrap();
        let result = source.check_oa(&doi).await.unwrap();

        assert!(result.is_oa);
        assert_eq!(result.oa_status, "gold");
        assert_eq!(result.best_pdf_url().unwrap(), "https://example.com/paper.pdf");
    }
}
