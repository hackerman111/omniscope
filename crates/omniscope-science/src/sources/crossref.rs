use std::time::Duration;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::error::{Result, ScienceError};
use crate::http::{RateLimitedClient, DiskCache};
use crate::identifiers::doi::Doi;
use crate::types::DocumentType;
use crate::sources::{ExternalSource, SourceType, RateLimit, SearchResult, Metadata, DownloadUrl, SourceStatus};
use async_trait::async_trait;
use futures::StreamExt;

pub struct CrossRefSource {
    client: RateLimitedClient,
    cache: DiskCache,
    base_url: String,
}

impl CrossRefSource {
    pub fn new(polite_email: Option<String>) -> Self {
        Self::with_params(
            "https://api.crossref.org",
            Duration::from_millis(100),
            polite_email,
        )
    }

    pub fn with_params(
        base_url: &str,
        min_interval: Duration,
        polite_email: Option<String>,
    ) -> Self {
        let user_agent = match &polite_email {
            Some(email) => format!("omniscope/0.1 (mailto:{})", email),
            None => "omniscope/0.1".to_string(),
        };

        let client = RateLimitedClient::new(min_interval, 3, &user_agent);
        let cache = DiskCache::new("crossref", Duration::from_secs(7 * 24 * 3600));

        Self {
            client,
            cache,
            base_url: base_url.to_string(),
        }
    }

    pub async fn fetch_by_doi(&self, doi: &Doi) -> Result<CrossRefWork> {
        let key = format!("doi:{}", doi.normalized);
        if let Some(cached) = self.cache.get::<CrossRefWork>(&key).await {
            return Ok(cached);
        }

        let url = format!("{}/works/{}", self.base_url, doi.normalized);
        let val: Value = self.client.get_json(&url).await?;

        let work = CrossRefWork::from_json(&val["message"])?;
        self.cache.set(&key, &work).await;

        Ok(work)
    }

    pub async fn query_by_text(&self, reference: &str) -> Result<Option<(Doi, f32)>> {
        let url = format!(
            "{}/works?query.bibliographic={}&rows=1",
            self.base_url,
            urlencoding::encode(reference)
        );
        let val: Value = self.client.get_json(&url).await?;

        if let Some(item) = val["message"]["items"].as_array().and_then(|items| items.first()) {
            let score = item["score"].as_f64().unwrap_or(0.0) as f32;
            if score >= 80.0
                && let Some(doi) = item["DOI"].as_str().and_then(|doi_str| Doi::parse(doi_str).ok())
            {
                return Ok(Some((doi, score)));
            }
        }

        Ok(None)
    }

    pub async fn fetch_batch(&self, dois: &[Doi]) -> Result<Vec<CrossRefWork>> {
        let mut stream = futures::stream::iter(dois)
            .map(|doi| self.fetch_by_doi(doi))
            .buffer_unordered(5);

        let mut results = Vec::new();
        while let Some(res) = stream.next().await {
            results.push(res?);
        }
        Ok(results)
    }
}

#[async_trait]
impl ExternalSource for CrossRefSource {
    fn name() -> &'static str { "CrossRef" }
    fn source_type() -> SourceType { SourceType::AcademicMetadata }
    fn requires_auth() -> bool { false }
    fn rate_limit() -> RateLimit { RateLimit { requests_per_second: 10.0 } }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let url = format!(
            "{}/works?query={}&rows=10",
            self.base_url,
            urlencoding::encode(query)
        );
        let val: Value = self.client.get_json(&url).await?;
        
        let mut results = Vec::new();
        if let Some(items) = val["message"]["items"].as_array() {
            for item in items {
                let title = item["title"][0].as_str().unwrap_or("Unknown Title").to_string();
                let authors = parse_authors(item);
                let year = parse_year(item);
                let identifier = item["DOI"].as_str().map(|s| s.to_string());
                let score = item["score"].as_f64().unwrap_or(0.0) as f32;
                
                results.push(SearchResult {
                    title,
                    authors,
                    year,
                    identifier,
                    source: "CrossRef".to_string(),
                    relevance_score: score,
                });
            }
        }
        
        Ok(results)
    }

    async fn fetch_metadata(&self, id: &str) -> Result<Option<Metadata>> {
        if let Ok(doi) = Doi::parse(id) {
            let work = self.fetch_by_doi(&doi).await?;
            Ok(Some(work.into_metadata()))
        } else {
            Ok(None)
        }
    }

    async fn find_download_url(&self, _id: &str) -> Result<Option<DownloadUrl>> {
        // CrossRef usually doesn't provide direct PDF downloads, mostly landing pages.
        Ok(None)
    }

    async fn health_check(&self) -> SourceStatus {
        // Simple health check could be fetching a known DOI
        SourceStatus {
            available: true,
            latency_ms: None,
            last_checked: Some(chrono::Utc::now()),
            mirror: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossRefWork {
    pub doi: String,
    pub title: Vec<String>,
    pub author: Vec<CrossRefAuthor>,
    pub published_year: Option<i32>,
    pub work_type: DocumentType,
    pub container_title: Vec<String>,
    pub publisher: Option<String>,
    pub issn: Vec<String>,
    pub isbn: Vec<String>,
    pub abstract_text: Option<String>,
    pub reference_count: Option<u32>,
    pub citation_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossRefAuthor {
    pub given: Option<String>,
    pub family: Option<String>,
    pub name: Option<String>,
    pub affiliation: Vec<String>,
}

impl CrossRefWork {
    pub fn from_json(v: &Value) -> Result<Self> {
        let doi = v["DOI"].as_str()
            .ok_or_else(|| ScienceError::Parse("Missing DOI in CrossRef response".to_string()))?
            .to_string();
            
        let title = v["title"].as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect())
            .unwrap_or_default();
            
        let author = v["author"].as_array()
            .map(|a| a.iter().map(CrossRefAuthor::from_json).collect())
            .unwrap_or_default();
            
        let published_year = parse_year(v);
            
        let work_type = v["type"].as_str()
            .map(DocumentType::from_crossref_type)
            .unwrap_or(DocumentType::Other("unknown".to_string()));
            
        let container_title = v["container-title"].as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect())
            .unwrap_or_default();
            
        let publisher = v["publisher"].as_str().map(|s| s.to_string());
        
        let issn = v["ISSN"].as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect())
            .unwrap_or_default();
            
        let isbn = v["ISBN"].as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect())
            .unwrap_or_default();
            
        let abstract_text = v["abstract"].as_str().map(|s| s.to_string());
        let reference_count = v["reference-count"].as_u64().map(|n| n as u32);
        let citation_count = v["is-referenced-by-count"].as_u64().map(|n| n as u32);
        
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

    pub fn into_metadata(self) -> Metadata {
        Metadata {
            title: self.title.first().cloned().unwrap_or_default(),
            authors: self.author.iter().map(|a| {
                match (&a.family, &a.given) {
                    (Some(f), Some(g)) => format!("{}, {}", f, g),
                    (Some(f), None) => f.clone(),
                    (None, Some(g)) => g.clone(),
                    (None, None) => a.name.clone().unwrap_or_default(),
                }
            }).collect(),
            year: self.published_year,
            abstract_text: self.abstract_text,
            doi: Some(self.doi),
            isbn: self.isbn.first().cloned(),
            publisher: self.publisher,
            journal: self.container_title.first().cloned(),
            volume: None, // CrossRef has volume, but it's not in Metadata struct yet
            issue: None,
            pages: None,
        }
    }
}

impl CrossRefAuthor {
    fn from_json(v: &Value) -> Self {
        let given = v["given"].as_str().map(|s| s.to_string());
        let family = v["family"].as_str().map(|s| s.to_string());
        let name = v["name"].as_str().map(|s| s.to_string());
        let affiliation = v["affiliation"].as_array()
            .map(|a| a.iter().filter_map(|v| v["name"].as_str()).map(|s| s.to_string()).collect())
            .unwrap_or_default();
            
        Self { given, family, name, affiliation }
    }
}

fn parse_year(v: &Value) -> Option<i32> {
    // CrossRef date parts: "published-print": {"date-parts": [[2017, 6, 12]]}
    v["published-print"]["date-parts"][0][0].as_i64()
        .or_else(|| v["published-online"]["date-parts"][0][0].as_i64())
        .or_else(|| v["issued"]["date-parts"][0][0].as_i64())
        .or_else(|| v["created"]["date-parts"][0][0].as_i64())
        .map(|n| n as i32)
}

fn parse_authors(item: &Value) -> Vec<String> {
    item["author"].as_array()
        .map(|a| a.iter().map(|v| {
            let family = v["family"].as_str();
            let given = v["given"].as_str();
            match (family, given) {
                (Some(f), Some(g)) => format!("{}, {}", f, g),
                (Some(f), None) => f.to_string(),
                (None, Some(g)) => g.to_string(),
                (None, None) => v["name"].as_str().unwrap_or("Unknown").to_string(),
            }
        }).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    #[tokio::test]
    async fn test_crossref_fetch_by_doi() {
        let mut server = Server::new_async().await;
        let base_url = server.url();

        let _m = server.mock("GET", "/works/10.1038/nature14539")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "status": "ok",
                "message": {
                    "DOI": "10.1038/nature14539",
                    "title": ["Human-level control through deep reinforcement learning"],
                    "author": [
                        {"given": "Volodymyr", "family": "Mnih"},
                        {"given": "Koray", "family": "Kavukcuoglu"}
                    ],
                    "published-print": {"date-parts": [[2015, 2, 26]]},
                    "type": "journal-article",
                    "container-title": ["Nature"],
                    "publisher": "Springer Science and Business Media LLC"
                }
            }"#)
            .create_async().await;

        let source = CrossRefSource::with_params(
            &base_url,
            Duration::from_secs(0),
            None,
        );
        let doi = Doi::parse("10.1038/nature14539").unwrap();
        let result = source.fetch_by_doi(&doi).await.unwrap();

        assert_eq!(result.title[0], "Human-level control through deep reinforcement learning");
        assert_eq!(result.author.len(), 2);
        assert_eq!(result.published_year, Some(2015));
        assert_eq!(result.work_type, DocumentType::JournalArticle);
    }

    #[tokio::test]
    async fn test_crossref_query_by_text() {
        let mut server = Server::new_async().await;
        let base_url = server.url();

        let _m = server.mock("GET", "/works?query.bibliographic=Mnih%202015&rows=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "status": "ok",
                "message": {
                    "items": [
                        {
                            "DOI": "10.1038/nature14539",
                            "score": 95.5
                        }
                    ]
                }
            }"#)
            .create_async().await;

        let source = CrossRefSource::with_params(
            &base_url,
            Duration::from_secs(0),
            None,
        );
        let result = source.query_by_text("Mnih 2015").await.unwrap();

        assert!(result.is_some());
        let (doi, score) = result.unwrap();
        assert_eq!(doi.normalized, "10.1038/nature14539");
        assert!(score > 80.0);
    }
}
