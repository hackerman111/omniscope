#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use futures::stream::{self, StreamExt};
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Result, ScienceError};
use crate::http::{DiskCache, RateLimitedClient};
use crate::identifiers::doi::Doi;
use crate::sources::{
    DownloadUrl, ExternalSource, Metadata, RateLimit, SearchResult, SourceStatus, SourceType,
};
use crate::types::DocumentType;

const BASE_URL: &str = "https://api.crossref.org";
const CACHE_TTL_SECS: u64 = 7 * 24 * 60 * 60;
const USER_AGENT_DEFAULT: &str = "omniscope/0.1";
const CONFIDENCE_THRESHOLD: f32 = 80.0;

static XML_TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"<[^>]+>").expect("valid regex"));
#[cfg(test)]
static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CrossRefAuthor {
    pub given: Option<String>,
    pub family: Option<String>,
    pub name: Option<String>,
    pub affiliation: Vec<String>,
    pub orcid: Option<String>,
}

impl CrossRefAuthor {
    pub fn display_name(&self) -> Option<String> {
        if let (Some(given), Some(family)) = (&self.given, &self.family) {
            return Some(format!("{given} {family}").trim().to_string());
        }
        self.name.clone().or_else(|| self.family.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossRefWork {
    pub doi: Doi,
    pub title: Vec<String>,
    pub author: Vec<CrossRefAuthor>,
    pub published_year: Option<i32>,
    pub work_type: DocumentType,
    pub container_title: Vec<String>,
    pub publisher: Option<String>,
    pub issn: Vec<String>,
    pub isbn: Vec<String>,
    pub abstract_text: Option<String>,
    pub reference_count: u32,
    pub citation_count: u32,
}

impl CrossRefWork {
    pub fn from_json(v: &Value) -> Result<Self> {
        let doi_raw = v
            .get("DOI")
            .and_then(Value::as_str)
            .ok_or_else(|| ScienceError::Parse("CrossRef work missing DOI".to_string()))?;
        let doi = Doi::parse(doi_raw)?;

        let title = array_of_strings(v.get("title")).unwrap_or_default();
        let author = parse_authors(v.get("author"));
        let published_year = extract_published_year(v);
        let work_type = v
            .get("type")
            .and_then(Value::as_str)
            .map(DocumentType::from_crossref_type)
            .unwrap_or_default();
        let container_title = array_of_strings(v.get("container-title")).unwrap_or_default();
        let publisher = v
            .get("publisher")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned);
        let issn = array_of_strings(v.get("ISSN")).unwrap_or_default();
        let isbn = array_of_strings(v.get("ISBN")).unwrap_or_default();
        let abstract_text = v
            .get("abstract")
            .and_then(Value::as_str)
            .map(strip_xml_tags);
        let reference_count = v
            .get("reference-count")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0);
        let citation_count = v
            .get("is-referenced-by-count")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0);

        Ok(Self {
            doi,
            title,
            author,
            published_year,
            work_type,
            container_title,
            publisher,
            issn,
            isbn,
            abstract_text,
            reference_count,
            citation_count,
        })
    }
}

pub struct CrossRefSource {
    pub client: RateLimitedClient,
    pub cache: DiskCache,
    pub polite_email: Option<String>,
    base_url: String,
}

impl CrossRefSource {
    pub fn new(polite_email: Option<String>) -> Self {
        let user_agent = build_user_agent(polite_email.as_deref());
        Self::with_config(
            BASE_URL.to_string(),
            polite_email,
            Duration::from_millis(100),
            Duration::from_secs(CACHE_TTL_SECS),
            "crossref".to_string(),
            user_agent,
        )
    }

    pub async fn fetch_by_doi(&self, doi: &Doi) -> Result<CrossRefWork> {
        let cache_key = format!("doi:{}", doi.normalized);
        if let Some(cached) = self.cache.get::<CrossRefWork>(&cache_key).await {
            return Ok(cached);
        }

        let mut url = parse_base_url(&self.base_url)?;
        {
            let mut segs = url
                .path_segments_mut()
                .map_err(|_| ScienceError::Parse("invalid CrossRef base URL".to_string()))?;
            segs.push("works");
            segs.push(&doi.normalized);
        }

        let body = self.client.get(url.as_str()).await?;
        let json: Value =
            serde_json::from_str(&body).map_err(|e| ScienceError::Parse(e.to_string()))?;
        let message = json
            .get("message")
            .ok_or_else(|| ScienceError::Parse("CrossRef response missing message".to_string()))?;

        let work = CrossRefWork::from_json(message)?;
        self.cache.set(&cache_key, &work).await;
        Ok(work)
    }

    pub async fn query_by_text(&self, reference: &str) -> Result<Option<(Doi, f32)>> {
        let cache_key = format!("query:{}", reference.trim().to_lowercase());
        if let Some(cached) = self.cache.get::<Option<(Doi, f32)>>(&cache_key).await {
            return Ok(cached);
        }

        let mut url = parse_base_url(&self.base_url)?;
        {
            let mut segs = url
                .path_segments_mut()
                .map_err(|_| ScienceError::Parse("invalid CrossRef base URL".to_string()))?;
            segs.push("works");
        }
        url.query_pairs_mut()
            .append_pair("query.bibliographic", reference)
            .append_pair("rows", "1");

        let body = self.client.get(url.as_str()).await?;
        let json: Value =
            serde_json::from_str(&body).map_err(|e| ScienceError::Parse(e.to_string()))?;

        let Some(item) = json
            .get("message")
            .and_then(|m| m.get("items"))
            .and_then(Value::as_array)
            .and_then(|items| items.first())
        else {
            self.cache.set(&cache_key, &None::<(Doi, f32)>).await;
            return Ok(None);
        };

        let score = item.get("score").and_then(Value::as_f64).unwrap_or(0.0) as f32;
        if score < CONFIDENCE_THRESHOLD {
            self.cache.set(&cache_key, &None::<(Doi, f32)>).await;
            return Ok(None);
        }

        let Some(doi_raw) = item.get("DOI").and_then(Value::as_str) else {
            self.cache.set(&cache_key, &None::<(Doi, f32)>).await;
            return Ok(None);
        };

        let doi = Doi::parse(doi_raw)?;
        let resolved = Some((doi, score));
        self.cache.set(&cache_key, &resolved).await;
        Ok(resolved)
    }

    pub async fn fetch_batch(&self, dois: &[Doi]) -> Result<Vec<CrossRefWork>> {
        stream::iter(dois.iter().cloned())
            .map(|doi| async move { self.fetch_by_doi(&doi).await })
            .buffer_unordered(5)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect()
    }

    fn with_config(
        base_url: String,
        polite_email: Option<String>,
        min_interval: Duration,
        cache_ttl: Duration,
        cache_namespace: String,
        user_agent: String,
    ) -> Self {
        Self {
            client: RateLimitedClient::new(min_interval, 3, &user_agent),
            cache: DiskCache::new(&cache_namespace, cache_ttl),
            polite_email,
            base_url,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_tests(base_url: String) -> Self {
        Self::with_config(
            base_url,
            None,
            Duration::from_millis(1),
            Duration::from_secs(60),
            format!(
                "crossref_test_{}_{}",
                std::process::id(),
                TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
            ),
            USER_AGENT_DEFAULT.to_string(),
        )
    }
}

#[async_trait]
impl ExternalSource for CrossRefSource {
    fn name() -> &'static str {
        "crossref"
    }

    fn source_type() -> SourceType {
        SourceType::AcademicMetadata
    }

    fn requires_auth() -> bool {
        false
    }

    fn rate_limit() -> RateLimit {
        RateLimit {
            requests_per_second: 10.0,
        }
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let mut out = Vec::new();
        if let Some((doi, score)) = self.query_by_text(query).await? {
            out.push(SearchResult {
                title: query.to_string(),
                authors: Vec::new(),
                year: None,
                identifier: Some(doi.normalized),
                source: Self::name().to_string(),
                relevance_score: score,
            });
        }
        Ok(out)
    }

    async fn fetch_metadata(&self, id: &str) -> Result<Option<Metadata>> {
        let doi = match Doi::parse(id) {
            Ok(value) => value,
            Err(_) => return Ok(None),
        };

        let work = self.fetch_by_doi(&doi).await?;
        let title = work.title.first().cloned().unwrap_or_default();
        let authors = work
            .author
            .iter()
            .filter_map(CrossRefAuthor::display_name)
            .collect::<Vec<_>>();

        Ok(Some(Metadata {
            title,
            authors,
            year: work.published_year,
            abstract_text: work.abstract_text,
            doi: Some(work.doi.normalized),
            isbn: work.isbn.first().cloned(),
            publisher: work.publisher,
            journal: work.container_title.first().cloned(),
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
        let mut url = match parse_base_url(&self.base_url) {
            Ok(url) => url,
            Err(_) => {
                return SourceStatus {
                    available: false,
                    latency_ms: None,
                    last_checked: Some(Utc::now()),
                    mirror: None,
                };
            }
        };
        if let Ok(mut segs) = url.path_segments_mut() {
            segs.push("works");
        }
        url.query_pairs_mut().append_pair("rows", "0");

        let available = self.client.get(url.as_str()).await.is_ok();
        SourceStatus {
            available,
            latency_ms: Some(start.elapsed().as_millis() as u64),
            last_checked: Some(Utc::now()),
            mirror: None,
        }
    }
}

fn build_user_agent(polite_email: Option<&str>) -> String {
    match polite_email.map(str::trim).filter(|s| !s.is_empty()) {
        Some(email) => format!("{USER_AGENT_DEFAULT} (mailto:{email})"),
        None => USER_AGENT_DEFAULT.to_string(),
    }
}

fn parse_base_url(base_url: &str) -> Result<Url> {
    Url::parse(base_url).map_err(|e| ScienceError::Parse(format!("invalid URL {base_url}: {e}")))
}

fn parse_authors(value: Option<&Value>) -> Vec<CrossRefAuthor> {
    value
        .and_then(Value::as_array)
        .map(|authors| {
            authors
                .iter()
                .map(|author| {
                    let affiliation = author
                        .get("affiliation")
                        .and_then(Value::as_array)
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(|item| item.get("name").and_then(Value::as_str))
                                .map(ToOwned::to_owned)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();

                    CrossRefAuthor {
                        given: author
                            .get("given")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned),
                        family: author
                            .get("family")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned),
                        name: author
                            .get("name")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned),
                        affiliation,
                        orcid: author
                            .get("ORCID")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned),
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn extract_published_year(v: &Value) -> Option<i32> {
    ["published-print", "published-online", "issued", "created"]
        .iter()
        .find_map(|key| {
            v.get(*key)
                .and_then(|obj| obj.get("date-parts"))
                .and_then(Value::as_array)
                .and_then(|parts| parts.first())
                .and_then(Value::as_array)
                .and_then(|first| first.first())
                .and_then(Value::as_i64)
                .and_then(|year| i32::try_from(year).ok())
        })
}

fn array_of_strings(value: Option<&Value>) -> Option<Vec<String>> {
    value.and_then(Value::as_array).map(|items| {
        items
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>()
    })
}

fn strip_xml_tags(input: &str) -> String {
    let stripped = XML_TAG_RE.replace_all(input, " ");
    stripped
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use mockito::Server;
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn fetch_by_doi_maps_journal_article_type() {
        let mut server = Server::new_async().await;
        let body = json!({
            "status": "ok",
            "message": {
                "DOI": "10.1000/journal",
                "title": ["A Journal Work"],
                "type": "journal-article",
                "issued": { "date-parts": [[2020, 1, 1]] }
            }
        });

        let mock = server
            .mock("GET", "/works/10.1000%2Fjournal")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body.to_string())
            .create_async()
            .await;

        let source = CrossRefSource::new_for_tests(server.url());
        let doi = Doi::parse("10.1000/journal").unwrap();

        let work = source.fetch_by_doi(&doi).await.unwrap();
        assert_eq!(work.work_type, DocumentType::JournalArticle);
        assert_eq!(work.published_year, Some(2020));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn fetch_by_doi_maps_book_type() {
        let mut server = Server::new_async().await;
        let body = json!({
            "status": "ok",
            "message": {
                "DOI": "10.1000/book",
                "title": ["A Book Work"],
                "type": "book",
                "issued": { "date-parts": [[2019]] }
            }
        });

        let mock = server
            .mock("GET", "/works/10.1000%2Fbook")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body.to_string())
            .create_async()
            .await;

        let source = CrossRefSource::new_for_tests(server.url());
        let doi = Doi::parse("10.1000/book").unwrap();

        let work = source.fetch_by_doi(&doi).await.unwrap();
        assert_eq!(work.work_type, DocumentType::Book);
        assert_eq!(work.published_year, Some(2019));

        mock.assert_async().await;
    }
}
