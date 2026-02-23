#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Result, ScienceError};
use crate::http::{DiskCache, RateLimitedClient};
use crate::identifiers::isbn::Isbn;
use crate::sources::{
    DownloadUrl, ExternalSource, Metadata, RateLimit, SearchResult, SourceStatus, SourceType,
};

const BASE_URL: &str = "https://openlibrary.org";
const CACHE_TTL_SECS: u64 = 7 * 24 * 60 * 60;
#[cfg(test)]
static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpenLibraryWork {
    pub title: String,
    pub authors: Vec<String>,
    pub publishers: Vec<String>,
    pub publish_date: Option<String>,
    pub subjects: Vec<String>,
    pub cover_url: Option<String>,
    pub openlibrary_id: Option<String>,
}

impl OpenLibraryWork {
    pub fn from_json(v: &Value) -> Self {
        let title = v
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        let authors = v
            .get("authors")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| item.get("name").and_then(Value::as_str))
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                v.get("author_name")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(Value::as_str)
                            .map(ToOwned::to_owned)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            });

        let publishers = v
            .get("publishers")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        item.get("name")
                            .and_then(Value::as_str)
                            .or_else(|| item.as_str())
                    })
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                v.get("publisher")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(Value::as_str)
                            .map(ToOwned::to_owned)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            });

        let publish_date = v
            .get("publish_date")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .or_else(|| {
                v.get("first_publish_year")
                    .and_then(Value::as_i64)
                    .map(|year| year.to_string())
            });

        let subjects = v
            .get("subjects")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        item.get("name")
                            .and_then(Value::as_str)
                            .or_else(|| item.as_str())
                    })
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let cover_url = v
            .get("cover")
            .and_then(Value::as_object)
            .and_then(|cover| {
                cover
                    .get("large")
                    .or_else(|| cover.get("medium"))
                    .or_else(|| cover.get("small"))
            })
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .or_else(|| {
                v.get("cover_i")
                    .and_then(Value::as_i64)
                    .map(|id| format!("https://covers.openlibrary.org/b/id/{id}-L.jpg"))
            });

        let openlibrary_id = v
            .get("key")
            .and_then(Value::as_str)
            .map(|key| key.trim_start_matches("/books/").to_string())
            .or_else(|| {
                v.get("identifiers")
                    .and_then(|ids| ids.get("openlibrary"))
                    .and_then(Value::as_array)
                    .and_then(|arr| arr.first())
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            });

        Self {
            title,
            authors,
            publishers,
            publish_date,
            subjects,
            cover_url,
            openlibrary_id,
        }
    }
}

pub struct OpenLibrarySource {
    pub client: RateLimitedClient,
    pub cache: DiskCache,
    base_url: String,
}

impl OpenLibrarySource {
    pub fn new() -> Self {
        Self::with_config(
            BASE_URL.to_string(),
            Duration::from_millis(500),
            Duration::from_secs(CACHE_TTL_SECS),
            "openlibrary".to_string(),
        )
    }

    pub async fn fetch_by_isbn(&self, isbn: &Isbn) -> Result<OpenLibraryWork> {
        let cache_key = format!("isbn:{}", isbn.isbn13);
        if let Some(cached) = self.cache.get::<OpenLibraryWork>(&cache_key).await {
            return Ok(cached);
        }

        let mut url = parse_base_url(&self.base_url)?;
        {
            let mut segs = url
                .path_segments_mut()
                .map_err(|_| ScienceError::Parse("invalid Open Library base URL".to_string()))?;
            segs.push("api");
            segs.push("books");
        }
        let bibkey = format!("ISBN:{}", isbn.isbn13);
        url.query_pairs_mut()
            .append_pair("bibkeys", &bibkey)
            .append_pair("format", "json")
            .append_pair("jscmd", "data");

        let body = self.client.get(url.as_str()).await?;
        let json: Value =
            serde_json::from_str(&body).map_err(|e| ScienceError::Parse(e.to_string()))?;

        let key = format!("ISBN:{}", isbn.isbn13);
        let Some(raw_work) = json.get(&key) else {
            return Err(ScienceError::ApiError(
                "openlibrary".to_string(),
                format!("book not found for ISBN {}", isbn.isbn13),
            ));
        };

        let work = OpenLibraryWork::from_json(raw_work);
        self.cache.set(&cache_key, &work).await;
        Ok(work)
    }

    pub async fn search_by_title(&self, title: &str) -> Result<Vec<OpenLibraryWork>> {
        let cache_key = format!("title:{}", title.trim().to_lowercase());
        if let Some(cached) = self.cache.get::<Vec<OpenLibraryWork>>(&cache_key).await {
            return Ok(cached);
        }

        let mut url = parse_base_url(&self.base_url)?;
        {
            let mut segs = url
                .path_segments_mut()
                .map_err(|_| ScienceError::Parse("invalid Open Library base URL".to_string()))?;
            segs.push("search.json");
        }
        url.query_pairs_mut()
            .append_pair("title", title)
            .append_pair("limit", "10");

        let body = self.client.get(url.as_str()).await?;
        let json: Value =
            serde_json::from_str(&body).map_err(|e| ScienceError::Parse(e.to_string()))?;

        let works = json
            .get("docs")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .map(OpenLibraryWork::from_json)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        self.cache.set(&cache_key, &works).await;
        Ok(works)
    }

    fn with_config(
        base_url: String,
        min_interval: Duration,
        cache_ttl: Duration,
        cache_namespace: String,
    ) -> Self {
        Self {
            client: RateLimitedClient::new(min_interval, 3, "omniscope-science/0.1"),
            cache: DiskCache::new(&cache_namespace, cache_ttl),
            base_url,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_tests(base_url: String) -> Self {
        Self::with_config(
            base_url,
            Duration::from_millis(1),
            Duration::from_secs(60),
            format!(
                "openlibrary_test_{}_{}",
                std::process::id(),
                TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
            ),
        )
    }
}

impl Default for OpenLibrarySource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExternalSource for OpenLibrarySource {
    fn name() -> &'static str {
        "openlibrary"
    }

    fn source_type() -> SourceType {
        SourceType::BookMetadata
    }

    fn requires_auth() -> bool {
        false
    }

    fn rate_limit() -> RateLimit {
        RateLimit {
            requests_per_second: 2.0,
        }
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let works = self.search_by_title(query).await?;
        Ok(works
            .into_iter()
            .map(|work| SearchResult {
                title: work.title,
                authors: work.authors,
                year: work
                    .publish_date
                    .as_deref()
                    .and_then(parse_year_from_date_string),
                identifier: work.openlibrary_id,
                source: Self::name().to_string(),
                relevance_score: 100.0,
            })
            .collect())
    }

    async fn fetch_metadata(&self, id: &str) -> Result<Option<Metadata>> {
        let isbn = match Isbn::parse(id) {
            Ok(isbn) => isbn,
            Err(_) => return Ok(None),
        };

        let work = self.fetch_by_isbn(&isbn).await?;
        Ok(Some(Metadata {
            title: work.title,
            authors: work.authors,
            year: work
                .publish_date
                .as_deref()
                .and_then(parse_year_from_date_string),
            abstract_text: None,
            doi: None,
            isbn: Some(isbn.isbn13),
            publisher: work.publishers.first().cloned(),
            journal: None,
            volume: None,
            issue: None,
            pages: None,
        }))
    }

    async fn find_download_url(&self, _id: &str) -> Result<Option<DownloadUrl>> {
        Ok(None)
    }

    async fn health_check(&self) -> SourceStatus {
        let start = Instant::now();
        let available = self.search_by_title("attention").await.is_ok();
        SourceStatus {
            available,
            latency_ms: Some(start.elapsed().as_millis() as u64),
            last_checked: Some(Utc::now()),
            mirror: None,
        }
    }
}

fn parse_base_url(base_url: &str) -> Result<Url> {
    Url::parse(base_url).map_err(|e| ScienceError::Parse(format!("invalid URL {base_url}: {e}")))
}

fn parse_year_from_date_string(input: &str) -> Option<i32> {
    input.chars().collect::<Vec<_>>().windows(4).find_map(|w| {
        let candidate = w.iter().collect::<String>();
        if candidate.chars().all(|c| c.is_ascii_digit()) {
            candidate.parse::<i32>().ok()
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_search_doc() {
        let doc = json!({
            "title": "Deep Learning",
            "author_name": ["Ian Goodfellow", "Yoshua Bengio"],
            "publisher": ["MIT Press"],
            "first_publish_year": 2016,
            "cover_i": 12345,
            "key": "/books/OL123M"
        });

        let work = OpenLibraryWork::from_json(&doc);
        assert_eq!(work.title, "Deep Learning");
        assert_eq!(work.authors.len(), 2);
        assert_eq!(work.openlibrary_id.as_deref(), Some("OL123M"));
    }
}
