use std::collections::HashMap;
use std::fmt;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use reqwest::Url;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::{Result, ScienceError};
use crate::http::{DiskCache, RateLimitedClient};
use crate::identifiers::{arxiv::ArxivId, doi::Doi};
use crate::sources::{
    DownloadUrl, ExternalSource, Metadata, RateLimit, SearchResult, SourceStatus, SourceType,
};

const BASE_URL: &str = "https://api.semanticscholar.org/graph/v1";
const RECOMMENDATIONS_URL: &str = "https://api.semanticscholar.org/recommendations/v1";
const CACHE_TTL_SECS: u64 = 7 * 24 * 60 * 60;
const DEFAULT_FIELDS: &str = "paperId,externalIds,title,abstract,year,authors,citationCount,referenceCount,influentialCitationCount,fieldsOfStudy,isOpenAccess,openAccessPdf,tldr";
const SEARCH_FIELDS: &str = "paperId,title,year,authors,citationCount";
const REFERENCE_FIELDS: &str = "paperId,externalIds,title,year,authors";
const API_KEY_HEADER: HeaderName = HeaderName::from_static("x-api-key");
#[cfg(test)]
static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct S2PaperId(String);

impl S2PaperId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn from_doi(doi: &Doi) -> Self {
        Self::new(format!("DOI:{}", doi.normalized))
    }

    pub fn from_arxiv(arxiv_id: &ArxivId) -> Self {
        Self::new(format!("ArXiv:{}", arxiv_id.id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for S2PaperId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for S2PaperId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for S2PaperId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct S2Author {
    pub author_id: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct S2OpenAccessPdf {
    pub url: String,
    pub status: Option<String>,
    pub license: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct S2Tldr {
    pub model: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct S2Paper {
    pub paper_id: String,
    pub external_ids: HashMap<String, String>,
    pub title: String,
    pub abstract_text: Option<String>,
    pub year: Option<i32>,
    pub authors: Vec<S2Author>,
    pub citation_count: u32,
    pub reference_count: u32,
    pub influential_citation_count: u32,
    pub fields_of_study: Vec<String>,
    pub is_open_access: bool,
    pub open_access_pdf: Option<S2OpenAccessPdf>,
    pub tldr: Option<S2Tldr>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct S2Reference {
    pub paper_id: Option<String>,
    pub external_ids: HashMap<String, String>,
    pub title: Option<String>,
    pub year: Option<i32>,
    pub authors: Vec<S2Author>,
}

impl S2Reference {
    pub fn from_json(v: &Value) -> Self {
        let source = v.get("citedPaper").unwrap_or(v);

        let paper_id = source
            .get("paperId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned);

        let external_ids = source
            .get("externalIds")
            .and_then(Value::as_object)
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|val| (k.clone(), val.to_string())))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();

        let title = source
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned);

        let year = source
            .get("year")
            .and_then(Value::as_i64)
            .and_then(|n| i32::try_from(n).ok());

        let authors = source
            .get("authors")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|author| S2Author {
                        author_id: author
                            .get("authorId")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned),
                        name: author
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Self {
            paper_id,
            external_ids,
            title,
            year,
            authors,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.paper_id.is_none()
            && self.title.is_none()
            && self.external_ids.is_empty()
            && self.authors.is_empty()
            && self.year.is_none()
    }
}

impl S2Paper {
    pub fn from_json(v: &Value) -> Result<Self> {
        let paper_id = v
            .get("paperId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        let external_ids = v
            .get("externalIds")
            .and_then(Value::as_object)
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|val| (k.clone(), val.to_string())))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();

        let title = v
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();

        let abstract_text = v
            .get("abstract")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned);

        let year = v
            .get("year")
            .and_then(Value::as_i64)
            .and_then(|n| i32::try_from(n).ok());

        let authors = v
            .get("authors")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|author| S2Author {
                        author_id: author
                            .get("authorId")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned),
                        name: author
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let citation_count = as_u32(v.get("citationCount")).unwrap_or(0);
        let reference_count = as_u32(v.get("referenceCount")).unwrap_or(0);
        let influential_citation_count = as_u32(v.get("influentialCitationCount")).unwrap_or(0);

        let fields_of_study = parse_fields_of_study(v);

        let is_open_access = v
            .get("isOpenAccess")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let open_access_pdf = v.get("openAccessPdf").and_then(|pdf| {
            let url = pdf
                .get("url")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)?;
            Some(S2OpenAccessPdf {
                url,
                status: pdf
                    .get("status")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                license: pdf
                    .get("license")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
            })
        });

        let tldr = v.get("tldr").and_then(|raw| {
            let text = raw
                .get("text")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)?;
            Some(S2Tldr {
                model: raw
                    .get("model")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                text,
            })
        });

        Ok(Self {
            paper_id,
            external_ids,
            title,
            abstract_text,
            year,
            authors,
            citation_count,
            reference_count,
            influential_citation_count,
            fields_of_study,
            is_open_access,
            open_access_pdf,
            tldr,
        })
    }
}

pub struct SemanticScholarSource {
    pub client: RateLimitedClient,
    pub cache: DiskCache,
    pub api_key: Option<String>,
    base_url: String,
    recommendations_base_url: String,
}

impl SemanticScholarSource {
    pub fn new(api_key: Option<String>) -> Self {
        let min_interval = if api_key.as_deref().is_some_and(|k| !k.trim().is_empty()) {
            Duration::from_millis(100)
        } else {
            Duration::from_secs(1)
        };

        Self::with_config(
            BASE_URL.to_string(),
            RECOMMENDATIONS_URL.to_string(),
            api_key,
            min_interval,
            Duration::from_secs(CACHE_TTL_SECS),
            "semantic_scholar".to_string(),
        )
    }

    pub async fn fetch_paper(&self, id: &S2PaperId) -> Result<S2Paper> {
        let cache_key = format!("paper:{}", id.as_str());
        if let Some(cached) = self.cache.get::<S2Paper>(&cache_key).await {
            return Ok(cached);
        }

        let mut url = parse_base_url(&self.base_url)?;
        {
            let mut segs = url.path_segments_mut().map_err(|_| {
                ScienceError::Parse("invalid Semantic Scholar base URL".to_string())
            })?;
            segs.push("paper");
            segs.push(id.as_str());
        }
        url.query_pairs_mut().append_pair("fields", DEFAULT_FIELDS);

        let body = self
            .client
            .get_with_headers(url.as_str(), self.auth_headers()?)
            .await?;
        let json: Value =
            serde_json::from_str(&body).map_err(|e| ScienceError::Parse(e.to_string()))?;

        let paper = S2Paper::from_json(&json)?;
        self.cache.set(&cache_key, &paper).await;
        Ok(paper)
    }

    pub async fn fetch_batch(&self, ids: &[S2PaperId]) -> Result<Vec<S2Paper>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut out = Vec::new();
        for chunk in ids.chunks(500) {
            let mut url = parse_base_url(&self.base_url)?;
            {
                let mut segs = url.path_segments_mut().map_err(|_| {
                    ScienceError::Parse("invalid Semantic Scholar base URL".to_string())
                })?;
                segs.push("paper");
                segs.push("batch");
            }
            url.query_pairs_mut().append_pair("fields", DEFAULT_FIELDS);

            let body = json!({
                "ids": chunk.iter().map(S2PaperId::as_str).collect::<Vec<_>>()
            });

            let response: Value = self
                .client
                .post_json_with_headers(url.as_str(), &body, self.auth_headers()?)
                .await?;

            let papers = if let Some(items) = response.as_array() {
                items
                    .iter()
                    .map(S2Paper::from_json)
                    .collect::<Result<Vec<_>>>()?
            } else if let Some(items) = response.get("data").and_then(Value::as_array) {
                items
                    .iter()
                    .map(S2Paper::from_json)
                    .collect::<Result<Vec<_>>>()?
            } else {
                return Err(ScienceError::Parse(
                    "unexpected Semantic Scholar batch response".to_string(),
                ));
            };

            out.extend(papers);
        }

        Ok(out)
    }

    pub async fn fetch_references(&self, id: &S2PaperId) -> Result<Vec<S2Reference>> {
        let cache_key = format!("references:{}", id.as_str());
        if let Some(cached) = self.cache.get::<Vec<S2Reference>>(&cache_key).await {
            return Ok(cached);
        }

        let mut url = parse_base_url(&self.base_url)?;
        {
            let mut segs = url.path_segments_mut().map_err(|_| {
                ScienceError::Parse("invalid Semantic Scholar base URL".to_string())
            })?;
            segs.push("paper");
            segs.push(id.as_str());
            segs.push("references");
        }
        url.query_pairs_mut()
            .append_pair("fields", REFERENCE_FIELDS);

        let body = self
            .client
            .get_with_headers(url.as_str(), self.auth_headers()?)
            .await?;
        let json: Value =
            serde_json::from_str(&body).map_err(|e| ScienceError::Parse(e.to_string()))?;

        let references = json
            .get("data")
            .and_then(Value::as_array)
            .or_else(|| json.as_array())
            .map(|items| {
                items
                    .iter()
                    .map(S2Reference::from_json)
                    .filter(|reference| !reference.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        self.cache.set(&cache_key, &references).await;
        Ok(references)
    }

    pub async fn get_recommendations(&self, paper_id: &str) -> Result<Vec<S2Paper>> {
        let cache_key = format!("recommendations:{paper_id}");
        if let Some(cached) = self.cache.get::<Vec<S2Paper>>(&cache_key).await {
            return Ok(cached);
        }

        let mut url = parse_base_url(&self.recommendations_base_url)?;
        {
            let mut segs = url.path_segments_mut().map_err(|_| {
                ScienceError::Parse("invalid Semantic Scholar recommendations URL".to_string())
            })?;
            segs.push("papers");
            segs.push("forpaper");
            segs.push(paper_id);
        }
        url.query_pairs_mut()
            .append_pair("limit", "20")
            .append_pair("fields", DEFAULT_FIELDS);

        let body = self
            .client
            .get_with_headers(url.as_str(), self.auth_headers()?)
            .await?;
        let json: Value =
            serde_json::from_str(&body).map_err(|e| ScienceError::Parse(e.to_string()))?;

        let papers = if let Some(items) = json.get("recommendedPapers").and_then(Value::as_array) {
            items
                .iter()
                .map(S2Paper::from_json)
                .collect::<Result<Vec<_>>>()?
        } else if let Some(items) = json.as_array() {
            items
                .iter()
                .map(S2Paper::from_json)
                .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };

        self.cache.set(&cache_key, &papers).await;
        Ok(papers)
    }

    fn with_config(
        base_url: String,
        recommendations_base_url: String,
        api_key: Option<String>,
        min_interval: Duration,
        cache_ttl: Duration,
        cache_namespace: String,
    ) -> Self {
        Self {
            client: RateLimitedClient::new(min_interval, 3, "omniscope-science/0.1"),
            cache: DiskCache::new(&cache_namespace, cache_ttl),
            api_key,
            base_url,
            recommendations_base_url,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_tests(base_url: String) -> Self {
        Self::with_config(
            base_url,
            "https://example.invalid/recommendations/v1".to_string(),
            None,
            Duration::from_millis(1),
            Duration::from_secs(60),
            format!(
                "semantic_scholar_test_{}_{}",
                std::process::id(),
                TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
            ),
        )
    }

    fn auth_headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        if let Some(key) = self
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let value =
                HeaderValue::from_str(key).map_err(|e| ScienceError::Parse(e.to_string()))?;
            headers.insert(API_KEY_HEADER, value);
        }
        Ok(headers)
    }
}

#[async_trait]
impl ExternalSource for SemanticScholarSource {
    fn name() -> &'static str {
        "semantic_scholar"
    }

    fn source_type() -> SourceType {
        SourceType::AcademicMetadata
    }

    fn requires_auth() -> bool {
        false
    }

    fn rate_limit() -> RateLimit {
        RateLimit {
            requests_per_second: 1.0,
        }
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let mut url = parse_base_url(&self.base_url)?;
        {
            let mut segs = url.path_segments_mut().map_err(|_| {
                ScienceError::Parse("invalid Semantic Scholar base URL".to_string())
            })?;
            segs.push("paper");
            segs.push("search");
        }
        url.query_pairs_mut()
            .append_pair("query", query)
            .append_pair("limit", "10")
            .append_pair("fields", SEARCH_FIELDS);

        let body = self
            .client
            .get_with_headers(url.as_str(), self.auth_headers()?)
            .await?;
        let json: Value =
            serde_json::from_str(&body).map_err(|e| ScienceError::Parse(e.to_string()))?;
        let items = json
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let out = items
            .iter()
            .map(|item| {
                let title = item
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let authors = item
                    .get("authors")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.get("name").and_then(Value::as_str))
                            .map(ToOwned::to_owned)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let year = item
                    .get("year")
                    .and_then(Value::as_i64)
                    .and_then(|n| i32::try_from(n).ok());
                let identifier = item
                    .get("paperId")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
                let relevance_score = item
                    .get("citationCount")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0) as f32;

                SearchResult {
                    title,
                    authors,
                    year,
                    identifier,
                    source: Self::name().to_string(),
                    relevance_score,
                }
            })
            .collect::<Vec<_>>();

        Ok(out)
    }

    async fn fetch_metadata(&self, id: &str) -> Result<Option<Metadata>> {
        let paper = self.fetch_paper(&S2PaperId::from(id)).await?;
        let doi = paper.external_ids.get("DOI").cloned();

        Ok(Some(Metadata {
            title: paper.title,
            authors: paper.authors.into_iter().map(|a| a.name).collect(),
            year: paper.year,
            abstract_text: paper.abstract_text,
            doi,
            isbn: None,
            publisher: None,
            journal: None,
            volume: None,
            issue: None,
            pages: None,
        }))
    }

    async fn find_download_url(&self, id: &str) -> Result<Option<DownloadUrl>> {
        let paper = self.fetch_paper(&S2PaperId::from(id)).await?;
        Ok(paper.open_access_pdf.map(|pdf| DownloadUrl {
            url: pdf.url,
            source_name: Self::name().to_string(),
            requires_redirect: false,
        }))
    }

    async fn health_check(&self) -> SourceStatus {
        let start = Instant::now();
        let available = self.search("attention").await.is_ok();
        SourceStatus {
            available,
            latency_ms: Some(start.elapsed().as_millis() as u64),
            last_checked: Some(Utc::now()),
            mirror: None,
        }
    }
}

fn parse_fields_of_study(v: &Value) -> Vec<String> {
    let mut out = v
        .get("fieldsOfStudy")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if out.is_empty()
        && let Some(s2_fields) = v.get("s2FieldsOfStudy").and_then(Value::as_array)
    {
        out = s2_fields
            .iter()
            .filter_map(|entry| entry.get("category").and_then(Value::as_str))
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
    }

    out
}

fn as_u32(value: Option<&Value>) -> Option<u32> {
    value
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
}

fn parse_base_url(base_url: &str) -> Result<Url> {
    Url::parse(base_url).map_err(|e| ScienceError::Parse(format!("invalid URL {base_url}: {e}")))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_external_ids_with_doi_and_arxiv() {
        let value = json!({
            "paperId": "abc123",
            "externalIds": {
                "DOI": "10.1000/xyz",
                "ArXiv": "1706.03762",
                "CorpusId": "123"
            },
            "title": "Attention Is All You Need",
            "authors": [{"name": "Ashish Vaswani"}],
            "citationCount": 500,
            "referenceCount": 30,
            "influentialCitationCount": 100,
            "fieldsOfStudy": ["Computer Science"],
            "isOpenAccess": true
        });

        let paper = S2Paper::from_json(&value).unwrap();

        assert_eq!(
            paper.external_ids.get("DOI"),
            Some(&"10.1000/xyz".to_string())
        );
        assert_eq!(
            paper.external_ids.get("ArXiv"),
            Some(&"1706.03762".to_string())
        );
    }

    #[test]
    fn parses_reference_entry_from_cited_paper_payload() {
        let value = json!({
            "citedPaper": {
                "paperId": "s2ref",
                "externalIds": {
                    "DOI": "10.1000/ref1",
                    "ArXiv": "1706.03762"
                },
                "title": "Attention Is All You Need",
                "year": 2017,
                "authors": [{"name": "Ashish Vaswani"}]
            }
        });

        let parsed = S2Reference::from_json(&value);

        assert_eq!(parsed.paper_id.as_deref(), Some("s2ref"));
        assert_eq!(
            parsed.external_ids.get("DOI"),
            Some(&"10.1000/ref1".to_string())
        );
        assert_eq!(parsed.title.as_deref(), Some("Attention Is All You Need"));
        assert_eq!(parsed.year, Some(2017));
        assert_eq!(parsed.authors.len(), 1);
        assert!(!parsed.is_empty());
    }
}
