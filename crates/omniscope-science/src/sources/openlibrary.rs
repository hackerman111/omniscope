use std::time::Duration;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::error::{Result, ScienceError};
use crate::http::{RateLimitedClient, DiskCache};
use crate::sources::{ExternalSource, SourceType, RateLimit, SearchResult, Metadata, DownloadUrl, SourceStatus};
use crate::identifiers::isbn::Isbn;
use async_trait::async_trait;
use std::collections::HashMap;

pub struct OpenLibrarySource {
    client: RateLimitedClient,
    cache: DiskCache,
    base_url: String,
}

impl OpenLibrarySource {
    pub fn new() -> Self {
        Self::with_params(
            "https://openlibrary.org",
            Duration::from_millis(500),
        )
    }

    pub fn with_params(base_url: &str, min_interval: Duration) -> Self {
        let client = RateLimitedClient::new(min_interval, 3, "omniscope/0.1");
        let cache = DiskCache::new("openlibrary", Duration::from_secs(7 * 24 * 3600));
        
        Self {
            client,
            cache,
            base_url: base_url.to_string(),
        }
    }

    pub async fn fetch_by_isbn(&self, isbn: &Isbn) -> Result<Option<OpenLibraryWork>> {
        let key = format!("isbn:{}", isbn.isbn13);
        if let Some(cached) = self.cache.get::<OpenLibraryWork>(&key).await {
            return Ok(Some(cached));
        }

        let bibkey = format!("ISBN:{}", isbn.isbn13);
        let url = format!("{}/api/books?bibkeys={}&format=json&jscmd=data", self.base_url, bibkey);
        
        let val: Value = self.client.get_json(&url).await?;
        
        if let Some(book_data) = val.get(&bibkey) {
            let work: OpenLibraryWork = serde_json::from_value(book_data.clone())
                .map_err(|e| ScienceError::Parse(e.to_string()))?;
            self.cache.set(&key, &work).await;
            Ok(Some(work))
        } else {
            Ok(None)
        }
    }

    pub async fn search_by_title(&self, title: &str) -> Result<Vec<OpenLibraryWork>> {
        let url = format!("{}/search.json?title={}&limit=10", self.base_url, urlencoding::encode(title));
        let val: Value = self.client.get_json(&url).await?;
        
        let mut results = Vec::new();
        if let Some(docs) = val["docs"].as_array() {
            for doc in docs {
                let work = OpenLibraryWork {
                    title: doc["title"].as_str().unwrap_or_default().to_string(),
                    authors: doc["author_name"].as_array()
                        .map(|a| a.iter().filter_map(|v| v.as_str()).map(|s| OpenLibraryName { name: s.to_string() }).collect())
                        .unwrap_or_default(),
                    publishers: doc["publisher"].as_array()
                        .map(|a| a.iter().filter_map(|v| v.as_str()).map(|s| OpenLibraryName { name: s.to_string() }).collect())
                        .unwrap_or_default(),
                    publish_date: doc["publish_date"].as_array()
                        .and_then(|a| a.first())
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    subjects: doc["subject"].as_array()
                        .map(|a| a.iter().filter_map(|v| v.as_str()).map(|s| OpenLibraryName { name: s.to_string() }).collect())
                        .unwrap_or_default(),
                    cover: doc["cover_i"].as_i64().map(|id| OpenLibraryCover {
                        small: Some(format!("https://covers.openlibrary.org/b/id/{}-S.jpg", id)),
                        medium: Some(format!("https://covers.openlibrary.org/b/id/{}-M.jpg", id)),
                        large: Some(format!("https://covers.openlibrary.org/b/id/{}-L.jpg", id)),
                    }),
                    identifiers: HashMap::new(),
                };
                results.push(work);
            }
        }
        
        Ok(results)
    }
}

impl Default for OpenLibrarySource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExternalSource for OpenLibrarySource {
    fn name() -> &'static str { "OpenLibrary" }
    fn source_type() -> SourceType { SourceType::BookMetadata }
    fn requires_auth() -> bool { false }
    fn rate_limit() -> RateLimit { RateLimit { requests_per_second: 2.0 } }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let works = self.search_by_title(query).await?;
        Ok(works.into_iter().map(|w| SearchResult {
            title: w.title,
            authors: w.authors.iter().map(|a| a.name.clone()).collect(),
            year: w.publish_date.as_ref().and_then(|d| d.split(' ').next_back()).and_then(|s| s.parse().ok()),
            identifier: None,
            source: "OpenLibrary".to_string(),
            relevance_score: 0.0,
        }).collect())
    }

    async fn fetch_metadata(&self, id: &str) -> Result<Option<Metadata>> {
        if let Ok(isbn) = Isbn::parse(id)
            && let Some(work) = self.fetch_by_isbn(&isbn).await?
        {
            return Ok(Some(work.into_metadata()));
        }
        Ok(None)
    }

    async fn find_download_url(&self, _id: &str) -> Result<Option<DownloadUrl>> {
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
pub struct OpenLibraryWork {
    pub title: String,
    #[serde(default)]
    pub authors: Vec<OpenLibraryName>,
    #[serde(default)]
    pub publishers: Vec<OpenLibraryName>,
    pub publish_date: Option<String>,
    #[serde(default)]
    pub subjects: Vec<OpenLibraryName>,
    pub cover: Option<OpenLibraryCover>,
    #[serde(default)]
    pub identifiers: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenLibraryName {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenLibraryCover {
    pub small: Option<String>,
    pub medium: Option<String>,
    pub large: Option<String>,
}

impl OpenLibraryWork {
    pub fn into_metadata(self) -> Metadata {
        Metadata {
            title: self.title,
            authors: self.authors.into_iter().map(|a| a.name).collect(),
            year: self.publish_date.as_ref().and_then(|d| d.split(' ').next_back()).and_then(|s| s.parse().ok()),
            abstract_text: None,
            doi: None,
            isbn: self.identifiers.get("isbn_13").and_then(|v| v.first().cloned())
                .or_else(|| self.identifiers.get("isbn_10").and_then(|v| v.first().cloned())),
            publisher: self.publishers.first().map(|p| p.name.clone()),
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
    async fn test_openlibrary_fetch_by_isbn() {
        let mut server = Server::new_async().await;
        let base_url = server.url();

        let _m = server.mock("GET", "/api/books?bibkeys=ISBN:9780132350884&format=json&jscmd=data")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "ISBN:9780132350884": {
                    "title": "Clean Code",
                    "publish_date": "2008",
                    "publishers": [{"name": "Prentice Hall"}],
                    "authors": [{"name": "Robert C. Martin"}]
                }
            }"#)
            .create_async().await;

        let source = OpenLibrarySource::with_params(
            &base_url,
            Duration::from_secs(0),
        );
        let isbn = Isbn::parse("9780132350884").unwrap();
        let result = source.fetch_by_isbn(&isbn).await.unwrap();

        assert!(result.is_some());
        let work = result.unwrap();
        assert_eq!(work.title, "Clean Code");
        assert_eq!(work.authors[0].name, "Robert C. Martin");
    }
}
