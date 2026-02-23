use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Result, ScienceError};
use crate::http::{DiskCache, RateLimitedClient};
use crate::sources::{
    DownloadUrl, ExternalSource, Metadata, RateLimit, SearchResult, SourceStatus, SourceType,
};

const BASE_URL: &str = "https://api.openalex.org";
const CACHE_TTL_SECS: u64 = 7 * 24 * 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct OpenAlexId(String);

impl OpenAlexId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for OpenAlexId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for OpenAlexId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpenAlexIds {
    pub openalex: Option<String>,
    pub doi: Option<String>,
    pub pmid: Option<String>,
    pub pmcid: Option<String>,
    pub mag: Option<String>,
    pub extra: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpenAlexOpenAccess {
    pub is_oa: bool,
    pub oa_status: Option<String>,
    pub oa_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpenAlexAuthor {
    pub id: Option<String>,
    pub display_name: String,
    pub orcid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpenAlexAuthorship {
    pub author: OpenAlexAuthor,
    pub institutions: Vec<String>,
    pub author_position: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpenAlexWork {
    pub id: String,
    pub doi: Option<String>,
    pub title: String,
    pub publication_year: Option<i32>,
    pub ids: OpenAlexIds,
    pub open_access: OpenAlexOpenAccess,
    pub authorships: Vec<OpenAlexAuthorship>,
    pub cited_by_count: u32,
    pub referenced_works: Vec<String>,
    pub abstract_inverted_index: Option<HashMap<String, Vec<u32>>>,
}

impl OpenAlexWork {
    pub fn from_json(v: &Value) -> Result<Self> {
        let id = v
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        let doi = v.get("doi").and_then(Value::as_str).map(ToOwned::to_owned);

        let title = v
            .get("title")
            .or_else(|| v.get("display_name"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        let publication_year = v
            .get("publication_year")
            .and_then(Value::as_i64)
            .and_then(|n| i32::try_from(n).ok());

        let ids = parse_ids(v.get("ids"));

        let open_access = parse_open_access(v.get("open_access"));

        let authorships = v
            .get("authorships")
            .and_then(Value::as_array)
            .map(|arr| arr.iter().map(parse_authorship).collect::<Vec<_>>())
            .unwrap_or_default();

        let cited_by_count = v
            .get("cited_by_count")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0);

        let referenced_works = v
            .get("referenced_works")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let abstract_inverted_index = v
            .get("abstract_inverted_index")
            .and_then(Value::as_object)
            .map(|obj| {
                obj.iter()
                    .filter_map(|(token, positions)| {
                        let values = positions
                            .as_array()?
                            .iter()
                            .filter_map(Value::as_u64)
                            .filter_map(|n| u32::try_from(n).ok())
                            .collect::<Vec<_>>();
                        if values.is_empty() {
                            None
                        } else {
                            Some((token.clone(), values))
                        }
                    })
                    .collect::<HashMap<_, _>>()
            });

        Ok(Self {
            id,
            doi,
            title,
            publication_year,
            ids,
            open_access,
            authorships,
            cited_by_count,
            referenced_works,
            abstract_inverted_index,
        })
    }

    pub fn reconstruct_abstract(&self) -> Option<String> {
        let index = self.abstract_inverted_index.as_ref()?;
        let max_position = index
            .values()
            .flat_map(|positions| positions.iter())
            .max()
            .copied()? as usize;

        let mut slots = vec![String::new(); max_position + 1];
        for (word, positions) in index {
            for &pos in positions {
                let idx = pos as usize;
                if idx < slots.len() {
                    slots[idx] = word.clone();
                }
            }
        }

        let abstract_text = slots
            .into_iter()
            .filter(|token| !token.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        if abstract_text.is_empty() {
            None
        } else {
            Some(abstract_text)
        }
    }
}

pub struct OpenAlexSource {
    pub client: RateLimitedClient,
    pub cache: DiskCache,
    base_url: String,
}

impl OpenAlexSource {
    pub fn new() -> Self {
        Self::with_config(
            BASE_URL.to_string(),
            Duration::from_millis(100),
            Duration::from_secs(CACHE_TTL_SECS),
            "openalex".to_string(),
        )
    }

    pub async fn fetch_work(&self, id: &OpenAlexId) -> Result<OpenAlexWork> {
        let cache_key = format!("work:{}", id.as_str());
        if let Some(cached) = self.cache.get::<OpenAlexWork>(&cache_key).await {
            return Ok(cached);
        }

        let mut url = parse_base_url(&self.base_url)?;
        {
            let mut segs = url
                .path_segments_mut()
                .map_err(|_| ScienceError::Parse("invalid OpenAlex base URL".to_string()))?;
            segs.push("works");
            segs.push(id.as_str());
        }

        let body = self.client.get(url.as_str()).await?;
        let json: Value =
            serde_json::from_str(&body).map_err(|e| ScienceError::Parse(e.to_string()))?;
        let work = OpenAlexWork::from_json(&json)?;

        self.cache.set(&cache_key, &work).await;
        Ok(work)
    }

    pub async fn search(&self, query: &str, limit: u32) -> Result<Vec<OpenAlexWork>> {
        let cap_limit = limit.clamp(1, 200);
        let cache_key = format!("search:{}:{}", query.trim().to_lowercase(), cap_limit);
        if let Some(cached) = self.cache.get::<Vec<OpenAlexWork>>(&cache_key).await {
            return Ok(cached);
        }

        let mut url = parse_base_url(&self.base_url)?;
        {
            let mut segs = url
                .path_segments_mut()
                .map_err(|_| ScienceError::Parse("invalid OpenAlex base URL".to_string()))?;
            segs.push("works");
        }
        url.query_pairs_mut()
            .append_pair("search", query)
            .append_pair("per-page", &cap_limit.to_string());

        let body = self.client.get(url.as_str()).await?;
        let json: Value =
            serde_json::from_str(&body).map_err(|e| ScienceError::Parse(e.to_string()))?;
        let works = json
            .get("results")
            .and_then(Value::as_array)
            .map(|results| {
                results
                    .iter()
                    .map(OpenAlexWork::from_json)
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
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
}

impl Default for OpenAlexSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExternalSource for OpenAlexSource {
    fn name() -> &'static str {
        "openalex"
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
        let works = self.search(query, 10).await?;
        Ok(works
            .into_iter()
            .map(|work| {
                let OpenAlexWork {
                    id,
                    title,
                    publication_year,
                    authorships,
                    cited_by_count,
                    ..
                } = work;
                SearchResult {
                    title,
                    authors: authorships
                        .into_iter()
                        .map(|a| a.author.display_name)
                        .collect(),
                    year: publication_year,
                    identifier: Some(id),
                    source: Self::name().to_string(),
                    relevance_score: cited_by_count as f32,
                }
            })
            .collect())
    }

    async fn fetch_metadata(&self, id: &str) -> Result<Option<Metadata>> {
        let work = self.fetch_work(&OpenAlexId::from(id)).await?;
        let authors = work
            .authorships
            .iter()
            .map(|a| a.author.display_name.clone())
            .collect::<Vec<_>>();
        let abstract_text = work.reconstruct_abstract();
        Ok(Some(Metadata {
            title: work.title,
            authors,
            year: work.publication_year,
            abstract_text,
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
        let work = self.fetch_work(&OpenAlexId::from(id)).await?;
        Ok(work.open_access.oa_url.map(|url| DownloadUrl {
            url,
            source_name: Self::name().to_string(),
            requires_redirect: false,
        }))
    }

    async fn health_check(&self) -> SourceStatus {
        let start = Instant::now();
        let available = self.search("attention", 1).await.is_ok();
        SourceStatus {
            available,
            latency_ms: Some(start.elapsed().as_millis() as u64),
            last_checked: Some(Utc::now()),
            mirror: None,
        }
    }
}

fn parse_ids(value: Option<&Value>) -> OpenAlexIds {
    let Some(obj) = value.and_then(Value::as_object) else {
        return OpenAlexIds::default();
    };

    let mut extra = HashMap::new();
    for (key, val) in obj {
        if let Some(text) = val.as_str()
            && !matches!(key.as_str(), "openalex" | "doi" | "pmid" | "pmcid" | "mag")
        {
            extra.insert(key.clone(), text.to_string());
        }
    }

    OpenAlexIds {
        openalex: obj
            .get("openalex")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        doi: obj
            .get("doi")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        pmid: obj
            .get("pmid")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        pmcid: obj
            .get("pmcid")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        mag: obj
            .get("mag")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        extra,
    }
}

fn parse_open_access(value: Option<&Value>) -> OpenAlexOpenAccess {
    let Some(obj) = value.and_then(Value::as_object) else {
        return OpenAlexOpenAccess::default();
    };

    OpenAlexOpenAccess {
        is_oa: obj.get("is_oa").and_then(Value::as_bool).unwrap_or(false),
        oa_status: obj
            .get("oa_status")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        oa_url: obj
            .get("oa_url")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    }
}

fn parse_authorship(value: &Value) -> OpenAlexAuthorship {
    let author = value
        .get("author")
        .and_then(Value::as_object)
        .map(|obj| OpenAlexAuthor {
            id: obj.get("id").and_then(Value::as_str).map(ToOwned::to_owned),
            display_name: obj
                .get("display_name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            orcid: obj
                .get("orcid")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        })
        .unwrap_or_default();

    let institutions = value
        .get("institutions")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|inst| inst.get("display_name").and_then(Value::as_str))
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    OpenAlexAuthorship {
        author,
        institutions,
        author_position: value
            .get("author_position")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    }
}

fn parse_base_url(base_url: &str) -> Result<Url> {
    Url::parse(base_url).map_err(|e| ScienceError::Parse(format!("invalid URL {base_url}: {e}")))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn reconstruct_abstract_from_inverted_index() {
        let mut index = HashMap::new();
        index.insert("attention".to_string(), vec![0]);
        index.insert("is".to_string(), vec![1]);
        index.insert("all".to_string(), vec![2]);
        index.insert("you".to_string(), vec![3]);
        index.insert("need".to_string(), vec![4]);

        let work = OpenAlexWork {
            abstract_inverted_index: Some(index),
            ..Default::default()
        };

        assert_eq!(
            work.reconstruct_abstract().as_deref(),
            Some("attention is all you need")
        );
    }
}
