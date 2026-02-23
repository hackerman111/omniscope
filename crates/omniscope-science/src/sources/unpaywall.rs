use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Result, ScienceError};
use crate::http::{DiskCache, RateLimitedClient};
use crate::identifiers::doi::Doi;
use crate::sources::{
    DownloadUrl, ExternalSource, Metadata, RateLimit, SearchResult, SourceStatus, SourceType,
};

const BASE_URL: &str = "https://api.unpaywall.org/v2";
const CACHE_TTL_SECS: u64 = 7 * 24 * 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OaLocation {
    pub url: Option<String>,
    pub url_for_pdf: Option<String>,
    pub host_type: Option<String>,
    pub license: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnpaywallResult {
    pub doi: String,
    pub is_oa: bool,
    pub oa_status: Option<String>,
    pub best_oa_location: Option<OaLocation>,
    pub oa_locations: Vec<OaLocation>,
    pub journal_is_oa: bool,
}

impl UnpaywallResult {
    pub fn from_json(v: &Value) -> Self {
        Self {
            doi: v
                .get("doi")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            is_oa: v.get("is_oa").and_then(Value::as_bool).unwrap_or(false),
            oa_status: v
                .get("oa_status")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            best_oa_location: v.get("best_oa_location").and_then(parse_oa_location),
            oa_locations: v
                .get("oa_locations")
                .and_then(Value::as_array)
                .map(|arr| arr.iter().filter_map(parse_oa_location).collect())
                .unwrap_or_default(),
            journal_is_oa: v
                .get("journal_is_oa")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }
    }

    pub fn best_pdf_url(&self) -> Option<&str> {
        self.best_oa_location
            .as_ref()
            .and_then(|loc| loc.url_for_pdf.as_deref().or(loc.url.as_deref()))
            .or_else(|| {
                self.oa_locations.iter().find_map(|loc| {
                    loc.url_for_pdf
                        .as_deref()
                        .or(loc.url.as_deref())
                        .filter(|url| !url.trim().is_empty())
                })
            })
    }
}

pub struct UnpaywallSource {
    pub client: RateLimitedClient,
    pub cache: DiskCache,
    pub email: String,
    base_url: String,
}

impl UnpaywallSource {
    pub fn new(email: String) -> Self {
        Self::with_config(
            BASE_URL.to_string(),
            email,
            Duration::from_millis(200),
            Duration::from_secs(CACHE_TTL_SECS),
            "unpaywall".to_string(),
        )
    }

    pub async fn check_oa(&self, doi: &Doi) -> Result<UnpaywallResult> {
        let cache_key = doi.normalized.to_string();
        if let Some(cached) = self.cache.get::<UnpaywallResult>(&cache_key).await {
            return Ok(cached);
        }

        let mut url = parse_base_url(&self.base_url)?;
        {
            let mut segs = url
                .path_segments_mut()
                .map_err(|_| ScienceError::Parse("invalid Unpaywall base URL".to_string()))?;
            segs.push(&doi.normalized);
        }
        url.query_pairs_mut().append_pair("email", &self.email);

        let body = self.client.get(url.as_str()).await?;
        let json: Value =
            serde_json::from_str(&body).map_err(|e| ScienceError::Parse(e.to_string()))?;
        let result = UnpaywallResult::from_json(&json);

        self.cache.set(&cache_key, &result).await;
        Ok(result)
    }

    fn with_config(
        base_url: String,
        email: String,
        min_interval: Duration,
        cache_ttl: Duration,
        cache_namespace: String,
    ) -> Self {
        Self {
            client: RateLimitedClient::new(min_interval, 3, "omniscope-science/0.1"),
            cache: DiskCache::new(&cache_namespace, cache_ttl),
            email,
            base_url,
        }
    }
}

#[async_trait]
impl ExternalSource for UnpaywallSource {
    fn name() -> &'static str {
        "unpaywall"
    }

    fn source_type() -> SourceType {
        SourceType::OpenAccess
    }

    fn requires_auth() -> bool {
        false
    }

    fn rate_limit() -> RateLimit {
        RateLimit {
            requests_per_second: 5.0,
        }
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let doi = match Doi::parse(query) {
            Ok(doi) => doi,
            Err(_) => return Ok(Vec::new()),
        };

        let oa = self.check_oa(&doi).await?;
        if !oa.is_oa {
            return Ok(Vec::new());
        }

        Ok(vec![SearchResult {
            title: doi.normalized.clone(),
            authors: Vec::new(),
            year: None,
            identifier: Some(doi.normalized),
            source: Self::name().to_string(),
            relevance_score: 100.0,
        }])
    }

    async fn fetch_metadata(&self, id: &str) -> Result<Option<Metadata>> {
        let doi = match Doi::parse(id) {
            Ok(doi) => doi,
            Err(_) => return Ok(None),
        };

        let oa = self.check_oa(&doi).await?;
        Ok(Some(Metadata {
            title: String::new(),
            authors: Vec::new(),
            year: None,
            abstract_text: None,
            doi: Some(oa.doi),
            isbn: None,
            publisher: None,
            journal: None,
            volume: None,
            issue: None,
            pages: None,
        }))
    }

    async fn find_download_url(&self, id: &str) -> Result<Option<DownloadUrl>> {
        let doi = match Doi::parse(id) {
            Ok(doi) => doi,
            Err(_) => return Ok(None),
        };
        let oa = self.check_oa(&doi).await?;
        Ok(oa.best_pdf_url().map(|url| DownloadUrl {
            url: url.to_string(),
            source_name: Self::name().to_string(),
            requires_redirect: false,
        }))
    }

    async fn health_check(&self) -> SourceStatus {
        SourceStatus {
            available: !self.email.trim().is_empty(),
            latency_ms: None,
            last_checked: Some(Utc::now()),
            mirror: None,
        }
    }
}

fn parse_oa_location(v: &Value) -> Option<OaLocation> {
    let obj = v.as_object()?;
    let str_field = |key: &str| {
        obj.get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
    };

    Some(OaLocation {
        url: str_field("url").or_else(|| str_field("url_for_landing_page")),
        url_for_pdf: str_field("url_for_pdf"),
        host_type: str_field("host_type"),
        license: str_field("license"),
        version: str_field("version"),
    })
}

fn parse_base_url(base_url: &str) -> Result<Url> {
    Url::parse(base_url).map_err(|e| ScienceError::Parse(format!("invalid URL {base_url}: {e}")))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn best_pdf_url_prefers_best_oa_location() {
        let value = json!({
            "doi": "10.1000/xyz",
            "is_oa": true,
            "best_oa_location": {
                "url": "https://example.org/landing",
                "url_for_pdf": "https://example.org/file.pdf"
            },
            "oa_locations": [
                {"url_for_pdf": "https://backup.example.org/file.pdf"}
            ]
        });

        let parsed = UnpaywallResult::from_json(&value);
        assert_eq!(parsed.best_pdf_url(), Some("https://example.org/file.pdf"));
    }
}
