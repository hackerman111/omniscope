use std::time::Duration;
use serde::{Deserialize, Serialize};
use crate::error::{Result, ScienceError};
use crate::http::{RateLimitedClient, DiskCache};
use crate::sources::{ExternalSource, SourceType, RateLimit, SearchResult, Metadata, DownloadUrl, SourceStatus};
use async_trait::async_trait;
use std::collections::HashMap;

pub struct OpenAlexSource {
    client: RateLimitedClient,
    cache: DiskCache,
    base_url: String,
}

impl OpenAlexSource {
    pub fn new() -> Self {
        Self::with_params("https://api.openalex.org", Duration::from_millis(100))
    }

    pub fn with_params(base_url: &str, min_interval: Duration) -> Self {
        let client = RateLimitedClient::new(min_interval, 3, "omniscope/0.1");
        let cache = DiskCache::new("openalex", Duration::from_secs(7 * 24 * 3600));
        
        Self {
            client,
            cache,
            base_url: base_url.to_string(),
        }
    }

    pub async fn fetch_work(&self, id: &str) -> Result<OpenAlexWork> {
        let key = format!("work:{}", id);
        if let Some(cached) = self.cache.get::<OpenAlexWork>(&key).await {
            return Ok(cached);
        }

        let url = format!("{}/works/{}", self.base_url, id);
        let work: OpenAlexWork = self.client.get_json(&url).await?;
        
        self.cache.set(&key, &work).await;
        Ok(work)
    }

    pub async fn search_works(&self, query: &str, limit: u32) -> Result<Vec<OpenAlexWork>> {
        let url = format!(
            "{}/works?search={}&per_page={}",
            self.base_url,
            urlencoding::encode(query),
            limit
        );
        let res: OpenAlexResponse = self.client.get_json(&url).await?;
        Ok(res.results)
    }
}

impl Default for OpenAlexSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExternalSource for OpenAlexSource {
    fn name() -> &'static str { "OpenAlex" }
    fn source_type() -> SourceType { SourceType::AcademicMetadata }
    fn requires_auth() -> bool { false }
    fn rate_limit() -> RateLimit { RateLimit { requests_per_second: 10.0 } }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let works = self.search_works(query, 10).await?;
        Ok(works.into_iter().map(|w| SearchResult {
            title: w.title.clone().unwrap_or_else(|| w.display_name.clone()),
            authors: w.authorships.iter().map(|a| a.author.display_name.clone()).collect(),
            year: w.publication_year,
            identifier: w.doi.clone().or(Some(w.id.clone())),
            source: "OpenAlex".to_string(),
            relevance_score: 0.0,
        }).collect())
    }

    async fn fetch_metadata(&self, id: &str) -> Result<Option<Metadata>> {
        match self.fetch_work(id).await {
            Ok(work) => Ok(Some(work.into_metadata())),
            Err(ScienceError::ApiError(_, _)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn find_download_url(&self, id: &str) -> Result<Option<DownloadUrl>> {
        let work = self.fetch_work(id).await?;
        if let Some(oa) = work.open_access
            && oa.is_oa
            && let Some(url) = oa.oa_url
        {
            return Ok(Some(DownloadUrl {
                url,
                source_name: "OpenAlex (OA)".to_string(),
                requires_redirect: false,
            }));
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
pub struct OpenAlexResponse {
    pub results: Vec<OpenAlexWork>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAlexWork {
    pub id: String,
    pub doi: Option<String>,
    pub title: Option<String>,
    pub display_name: String,
    pub publication_year: Option<i32>,
    pub ids: OpenAlexIds,
    pub open_access: Option<OpenAlexOA>,
    pub authorships: Vec<Authorship>,
    pub cited_by_count: u32,
    pub referenced_works: Vec<String>,
    pub abstract_inverted_index: Option<HashMap<String, Vec<u32>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAlexIds {
    pub openalex: String,
    pub doi: Option<String>,
    pub pmid: Option<String>,
    pub pmcid: Option<String>,
    pub mag: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAlexOA {
    pub is_oa: bool,
    pub oa_status: String,
    pub oa_url: Option<String>,
    pub any_repository_has_fulltext: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Authorship {
    pub author: Author,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub id: String,
    pub display_name: String,
}

impl OpenAlexWork {
    pub fn reconstruct_abstract(&self) -> Option<String> {
        let index = self.abstract_inverted_index.as_ref()?;
        
        let mut words_at_pos = Vec::new();
        for (word, positions) in index {
            for &pos in positions {
                words_at_pos.push((pos, word));
            }
        }
        
        words_at_pos.sort_by_key(|&(pos, _)| pos);
        
        let mut result = String::new();
        for (_, word) in words_at_pos {
            if !result.is_empty() {
                result.push(' ');
            }
            result.push_str(word);
        }
        
        Some(result)
    }

    pub fn into_metadata(self) -> Metadata {
        let abs = self.reconstruct_abstract();
        Metadata {
            title: self.title.unwrap_or(self.display_name),
            authors: self.authorships.into_iter().map(|a| a.author.display_name).collect(),
            year: self.publication_year,
            abstract_text: abs,
            doi: self.doi,
            isbn: None,
            publisher: None, // OpenAlex has this deeper in locations
            journal: None,   // OpenAlex has this in primary_location
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
    async fn test_openalex_reconstruct_abstract() {
        let mut index = HashMap::new();
        index.insert("The".to_string(), vec![0]);
        index.insert("Transformer".to_string(), vec![1]);
        index.insert("is".to_string(), vec![2]);
        index.insert("great".to_string(), vec![3]);

        let work = OpenAlexWork {
            id: "W1".to_string(),
            doi: None,
            title: None,
            display_name: "Test".to_string(),
            publication_year: None,
            ids: OpenAlexIds {
                openalex: "W1".to_string(),
                doi: None,
                pmid: None,
                pmcid: None,
                mag: None,
            },
            open_access: None,
            authorships: vec![],
            cited_by_count: 0,
            referenced_works: vec![],
            abstract_inverted_index: Some(index),
        };

        assert_eq!(work.reconstruct_abstract().unwrap(), "The Transformer is great");
    }

    #[tokio::test]
    async fn test_openalex_fetch_work() {
        let mut server = Server::new_async().await;
        let base_url = server.url();

        let _m = server.mock("GET", "/works/W2963403868")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "id": "https://openalex.org/W2963403868",
                "doi": "https://doi.org/10.48550/arxiv.1706.03762",
                "display_name": "Attention Is All You Need",
                "title": "Attention Is All You Need",
                "publication_year": 2017,
                "ids": {
                    "openalex": "https://openalex.org/W2963403868",
                    "doi": "https://doi.org/10.48550/arxiv.1706.03762"
                },
                "authorships": [
                    {"author": {"id": "A1", "display_name": "Ashish Vaswani"}}
                ],
                "cited_by_count": 87654,
                "referenced_works": []
            }"#)
            .create_async().await;

        let source = OpenAlexSource::with_params(
            &base_url,
            Duration::from_secs(0),
        );
        let result = source.fetch_work("W2963403868").await.unwrap();

        assert_eq!(result.display_name, "Attention Is All You Need");
        assert_eq!(result.publication_year, Some(2017));
        assert_eq!(result.authorships[0].author.display_name, "Ashish Vaswani");
    }
}
