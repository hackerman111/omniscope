use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use reqwest::Url;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::warn;

use crate::error::{Result, ScienceError};
use crate::http::{DiskCache, RateLimitedClient};
use crate::identifiers::doi::Doi;
use crate::sources::{
    DownloadUrl, ExternalSource, Metadata, RateLimit, SearchResult, SourceStatus, SourceType,
};

const BASE_URL: &str = "https://api.core.ac.uk/v3";
const CACHE_TTL_SECS: u64 = 7 * 24 * 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CoreWork {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub abstract_text: Option<String>,
    pub doi: Option<String>,
    pub download_url: Option<String>,
    pub year: Option<i32>,
}

impl CoreWork {
    pub fn from_json(v: &Value) -> Self {
        let id = v
            .get("id")
            .map(|id| {
                if let Some(text) = id.as_str() {
                    text.to_string()
                } else {
                    id.to_string()
                }
            })
            .unwrap_or_default();

        let title = v
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        let authors = parse_authors(v.get("authors"));

        let abstract_text = v
            .get("abstract")
            .or_else(|| v.get("description"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned);

        let doi = v
            .get("doi")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .or_else(|| {
                v.get("identifiers")
                    .and_then(Value::as_array)
                    .and_then(|arr| {
                        arr.iter().find_map(|item| {
                            item.as_str().and_then(|text| {
                                let lower = text.to_ascii_lowercase();
                                if lower.starts_with("10.") {
                                    Some(text.to_string())
                                } else {
                                    None
                                }
                            })
                        })
                    })
            });

        let download_url = v
            .get("downloadUrl")
            .or_else(|| v.get("download_url"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);

        let year = v
            .get("yearPublished")
            .or_else(|| v.get("year"))
            .and_then(Value::as_i64)
            .and_then(|n| i32::try_from(n).ok());

        Self {
            id,
            title,
            authors,
            abstract_text,
            doi,
            download_url,
            year,
        }
    }
}

pub struct CoreSource {
    pub client: RateLimitedClient,
    pub cache: DiskCache,
    pub api_key: String,
    base_url: String,
}

impl CoreSource {
    pub fn new(api_key: String) -> Self {
        Self::with_config(
            BASE_URL.to_string(),
            api_key,
            Duration::from_millis(200),
            Duration::from_secs(CACHE_TTL_SECS),
            "core".to_string(),
        )
    }

    pub async fn search_works(&self, query: &str) -> Result<Vec<CoreWork>> {
        if self.api_key.trim().is_empty() {
            warn!("CORE API key is empty; returning empty result set");
            return Ok(Vec::new());
        }

        let cache_key = format!("search:{}", query.trim().to_lowercase());
        if let Some(cached) = self.cache.get::<Vec<CoreWork>>(&cache_key).await {
            return Ok(cached);
        }

        let mut url = parse_base_url(&self.base_url)?;
        {
            let mut segs = url
                .path_segments_mut()
                .map_err(|_| ScienceError::Parse("invalid CORE base URL".to_string()))?;
            segs.push("search");
            segs.push("works");
        }
        url.query_pairs_mut()
            .append_pair("q", query)
            .append_pair("limit", "10");

        let body = self
            .client
            .get_with_headers(url.as_str(), self.auth_headers()?)
            .await?;
        let json: Value =
            serde_json::from_str(&body).map_err(|e| ScienceError::Parse(e.to_string()))?;

        let works = json
            .get("results")
            .and_then(Value::as_array)
            .map(|arr| arr.iter().map(CoreWork::from_json).collect::<Vec<_>>())
            .unwrap_or_default();

        self.cache.set(&cache_key, &works).await;
        Ok(works)
    }

    pub async fn search(&self, query: &str) -> Result<Vec<CoreWork>> {
        self.search_works(query).await
    }

    pub async fn fetch_by_doi(&self, doi: &Doi) -> Result<Option<CoreWork>> {
        if self.api_key.trim().is_empty() {
            warn!("CORE API key is empty; fetch_by_doi skipped");
            return Ok(None);
        }

        let cache_key = format!("doi:{}", doi.normalized);
        if let Some(cached) = self.cache.get::<Option<CoreWork>>(&cache_key).await {
            return Ok(cached);
        }

        let mut url = parse_base_url(&self.base_url)?;
        {
            let mut segs = url
                .path_segments_mut()
                .map_err(|_| ScienceError::Parse("invalid CORE base URL".to_string()))?;
            segs.push("works");
            segs.push(&format!("doi:{}", doi.normalized));
        }

        let body = self
            .client
            .get_with_headers(url.as_str(), self.auth_headers()?)
            .await?;
        let json: Value =
            serde_json::from_str(&body).map_err(|e| ScienceError::Parse(e.to_string()))?;

        let work = if json.is_null() {
            None
        } else {
            Some(CoreWork::from_json(&json))
        };

        self.cache.set(&cache_key, &work).await;
        Ok(work)
    }

    fn with_config(
        base_url: String,
        api_key: String,
        min_interval: Duration,
        cache_ttl: Duration,
        cache_namespace: String,
    ) -> Self {
        Self {
            client: RateLimitedClient::new(min_interval, 3, "omniscope-science/0.1"),
            cache: DiskCache::new(&cache_namespace, cache_ttl),
            api_key,
            base_url,
        }
    }

    fn auth_headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        let value = HeaderValue::from_str(&format!("Bearer {}", self.api_key.trim()))
            .map_err(|e| ScienceError::Parse(e.to_string()))?;
        headers.insert(AUTHORIZATION, value);
        Ok(headers)
    }
}

#[async_trait]
impl ExternalSource for CoreSource {
    fn name() -> &'static str {
        "core"
    }

    fn source_type() -> SourceType {
        SourceType::Search
    }

    fn requires_auth() -> bool {
        true
    }

    fn rate_limit() -> RateLimit {
        RateLimit {
            requests_per_second: 5.0,
        }
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let works = CoreSource::search(self, query).await?;
        Ok(works
            .into_iter()
            .map(|work| SearchResult {
                title: work.title,
                authors: work.authors,
                year: work.year,
                identifier: Some(work.id),
                source: Self::name().to_string(),
                relevance_score: 100.0,
            })
            .collect())
    }

    async fn fetch_metadata(&self, id: &str) -> Result<Option<Metadata>> {
        let doi = match Doi::parse(id) {
            Ok(doi) => doi,
            Err(_) => return Ok(None),
        };

        let Some(work) = self.fetch_by_doi(&doi).await? else {
            return Ok(None);
        };

        Ok(Some(Metadata {
            title: work.title,
            authors: work.authors,
            year: work.year,
            abstract_text: work.abstract_text,
            doi: work.doi,
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

        let Some(work) = self.fetch_by_doi(&doi).await? else {
            return Ok(None);
        };

        Ok(work.download_url.map(|url| DownloadUrl {
            url,
            source_name: Self::name().to_string(),
            requires_redirect: false,
        }))
    }

    async fn health_check(&self) -> SourceStatus {
        SourceStatus {
            available: !self.api_key.trim().is_empty(),
            latency_ms: None,
            last_checked: Some(Utc::now()),
            mirror: None,
        }
    }
}

fn parse_base_url(base_url: &str) -> Result<Url> {
    Url::parse(base_url).map_err(|e| ScienceError::Parse(format!("invalid URL {base_url}: {e}")))
}

fn parse_authors(value: Option<&Value>) -> Vec<String> {
    if let Some(authors) = value.and_then(Value::as_array) {
        return authors
            .iter()
            .filter_map(|author| {
                author.as_str().map(ToOwned::to_owned).or_else(|| {
                    author
                        .get("name")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned)
                })
            })
            .collect();
    }

    if let Some(authors) = value.and_then(Value::as_str) {
        return authors
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .collect();
    }

    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn search_returns_empty_without_api_key() {
        let source = CoreSource::new(String::new());
        let items = source.search("test").await.unwrap();
        assert!(items.is_empty());
    }
}
