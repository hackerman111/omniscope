use std::time::Duration;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use crate::error::{Result, ScienceError};
use crate::http::{RateLimitedClient, DiskCache};
use crate::sources::{ExternalSource, SourceType, RateLimit, SearchResult, Metadata, DownloadUrl, SourceStatus};
use async_trait::async_trait;
use std::collections::HashMap;

pub struct SemanticScholarSource {
    client: RateLimitedClient,
    cache: DiskCache,
    api_key: Option<String>,
    base_url: String,
}

impl SemanticScholarSource {
    pub fn new(api_key: Option<String>) -> Self {
        let min_interval = if api_key.is_some() {
            Duration::from_millis(100) // 10 req/sec with key
        } else {
            Duration::from_millis(1100) // ~1 req/sec without key
        };
        
        Self::with_params(
            "https://api.semanticscholar.org/graph/v1",
            min_interval,
            api_key,
        )
    }

    pub fn with_params(base_url: &str, min_interval: Duration, api_key: Option<String>) -> Self {
        let client = RateLimitedClient::new(min_interval, 3, "omniscope/0.1");
        let cache = DiskCache::new("semantic_scholar", Duration::from_secs(7 * 24 * 3600));
        
        Self {
            client,
            cache,
            api_key,
            base_url: base_url.to_string(),
        }
    }

    fn get_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(key) = &self.api_key
            && let Ok(val) = reqwest::header::HeaderValue::from_str(key)
        {
            headers.insert("x-api-key", val);
        }
        headers
    }

    pub async fn fetch_paper(&self, id: &str) -> Result<S2Paper> {
        let key = format!("paper:{}", id);
        if let Some(cached) = self.cache.get::<S2Paper>(&key).await {
            return Ok(cached);
        }

        let fields = "title,authors,year,abstract,externalIds,citationCount,referenceCount,influentialCitationCount,fieldsOfStudy,isOpenAccess,openAccessPdf,tldr";
        let url = format!("{}/paper/{}?fields={}", self.base_url, id, fields);
        
        let text = self.client.get_with_headers(&url, self.get_headers()).await?;
        let paper: S2Paper = serde_json::from_str(&text).map_err(|e| ScienceError::Parse(e.to_string()))?;
        
        self.cache.set(&key, &paper).await;
        Ok(paper)
    }

    pub async fn fetch_batch(&self, ids: &[String]) -> Result<Vec<S2Paper>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        
        // Semantic Scholar batch API: POST /paper/batch
        let url = format!("{}/paper/batch?fields=title,authors,year,externalIds,citationCount", self.base_url);
        let body = json!({ "ids": ids });
        
        let papers: Vec<Option<S2Paper>> = self.client.post_json(&url, &body).await?;
        Ok(papers.into_iter().flatten().collect())
    }

    pub async fn get_recommendations(&self, paper_id: &str) -> Result<Vec<S2Paper>> {
        let url = format!(
            "https://api.semanticscholar.org/recommendations/v1/papers/forpaper/{}?limit=10",
            paper_id
        );
        let val: Value = self.client.get_json(&url).await?;
        
        let recommendations = val["recommendedPapers"].as_array()
            .map(|a| a.iter().filter_map(|v| serde_json::from_value(v.clone()).ok()).collect())
            .unwrap_or_default();
            
        Ok(recommendations)
    }
}

#[async_trait]
impl ExternalSource for SemanticScholarSource {
    fn name() -> &'static str { "SemanticScholar" }
    fn source_type() -> SourceType { SourceType::AcademicMetadata }
    fn requires_auth() -> bool { false } // Optional API key
    fn rate_limit() -> RateLimit { RateLimit { requests_per_second: 1.0 } }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let url = format!(
            "{}/paper/search?query={}&limit=10&fields=title,authors,year,externalIds",
            self.base_url,
            urlencoding::encode(query)
        );
        
        let val: Value = self.client.get_with_headers(&url, self.get_headers())
            .await
            .and_then(|text| serde_json::from_str::<Value>(&text).map_err(|e| ScienceError::Parse(e.to_string())))?;
            
        let mut results = Vec::new();
        if let Some(data) = val["data"].as_array() {
            for item in data {
                let title = item["title"].as_str().unwrap_or("Unknown").to_string();
                let authors = item["authors"].as_array()
                    .map(|a| a.iter().filter_map(|v| v["name"].as_str()).map(|s| s.to_string()).collect())
                    .unwrap_or_default();
                let year = item["year"].as_i64().map(|n| n as i32);
                let identifier = item["externalIds"]["DOI"].as_str()
                    .or_else(|| item["paperId"].as_str())
                    .map(|s| s.to_string());
                
                results.push(SearchResult {
                    title,
                    authors,
                    year,
                    identifier,
                    source: "SemanticScholar".to_string(),
                    relevance_score: 0.0, // S2 search result doesn't explicitly return score in simple search
                });
            }
        }
        
        Ok(results)
    }

    async fn fetch_metadata(&self, id: &str) -> Result<Option<Metadata>> {
        match self.fetch_paper(id).await {
            Ok(paper) => Ok(Some(paper.into_metadata())),
            Err(ScienceError::ApiError(_, _)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn find_download_url(&self, id: &str) -> Result<Option<DownloadUrl>> {
        let paper = self.fetch_paper(id).await?;
        if let Some(pdf) = paper.open_access_pdf {
            Ok(Some(DownloadUrl {
                url: pdf.url,
                source_name: "SemanticScholar (OA)".to_string(),
                requires_redirect: false,
            }))
        } else {
            Ok(None)
        }
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
#[serde(rename_all = "camelCase")]
pub struct S2Paper {
    pub paper_id: String,
    #[serde(default)]
    pub external_ids: HashMap<String, String>,
    pub title: String,
    pub abstract_text: Option<String>,
    pub year: Option<i32>,
    #[serde(default)]
    pub authors: Vec<S2Author>,
    pub citation_count: Option<u32>,
    pub reference_count: Option<u32>,
    pub influential_citation_count: Option<u32>,
    #[serde(default)]
    pub fields_of_study: Vec<String>,
    #[serde(default)]
    pub is_open_access: bool,
    pub open_access_pdf: Option<S2OpenAccessPdf>,
    pub tldr: Option<S2Tldr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S2Author {
    pub author_id: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S2OpenAccessPdf {
    pub url: String,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S2Tldr {
    pub model: Option<String>,
    pub text: String,
}

impl S2Paper {
    pub fn into_metadata(self) -> Metadata {
        Metadata {
            title: self.title,
            authors: self.authors.into_iter().map(|a| a.name).collect(),
            year: self.year,
            abstract_text: self.abstract_text.or_else(|| self.tldr.map(|t| t.text)),
            doi: self.external_ids.get("DOI").cloned(),
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
    async fn test_s2_fetch_paper() {
        let mut server = Server::new_async().await;
        let base_url = server.url();

        let _m = server.mock("GET", "/paper/DOI:10.1038/nature14539?fields=title,authors,year,abstract,externalIds,citationCount,referenceCount,influentialCitationCount,fieldsOfStudy,isOpenAccess,openAccessPdf,tldr")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "paperId": "204e3073870fae3d05bcbc2f6a8e263d9b72e776",
                "externalIds": {"DOI": "10.1038/nature14539", "ArXiv": "1706.03762"},
                "title": "Human-level control through deep reinforcement learning",
                "year": 2015,
                "authors": [
                    {"authorId": "1", "name": "Volodymyr Mnih"},
                    {"authorId": "2", "name": "Koray Kavukcuoglu"}
                ],
                "citationCount": 1000,
                "isOpenAccess": true,
                "tldr": {"text": "TLDR of the paper"}
            }"#)
            .create_async().await;

        let source = SemanticScholarSource::with_params(
            &base_url,
            Duration::from_secs(0),
            None,
        );
        let result = source.fetch_paper("DOI:10.1038/nature14539").await.unwrap();

        assert_eq!(result.title, "Human-level control through deep reinforcement learning");
        assert_eq!(result.external_ids.get("ArXiv").unwrap(), "1706.03762");
        assert_eq!(result.citation_count, Some(1000));
        assert!(result.is_open_access);
    }
}
