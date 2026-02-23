use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::warn;

use crate::error::{Result, ScienceError};
use crate::http::RateLimitedClient;
use crate::identifiers::doi::Doi;
use crate::sources::{
    DownloadUrl, ExternalSource, Metadata, RateLimit, SearchResult, SourceStatus, SourceType,
};

const KNOWN_MIRRORS: &[&str] = &[
    "https://sci-hub.se",
    "https://sci-hub.st",
    "https://sci-hub.ru",
    "https://sci-hub.ren",
    "https://sci-hub.mksa.top",
];

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SciHubResult {
    pub pdf_url: Option<String>,
    pub title: Option<String>,
}

pub struct SciHubSource {
    pub client: RateLimitedClient,
    pub working_mirror: Arc<RwLock<Option<String>>>,
    mirrors: Vec<String>,
}

impl SciHubSource {
    pub fn new() -> Self {
        Self::with_config(
            KNOWN_MIRRORS.iter().map(|m| (*m).to_string()).collect(),
            Duration::from_secs(2),
        )
    }

    pub async fn init(&self) -> Result<()> {
        match self.find_working_mirror().await {
            Ok(mirror) => {
                *self.working_mirror.write().await = Some(mirror);
            }
            Err(_) => {
                warn!("scihub: no working mirror found during init");
            }
        }
        Ok(())
    }

    pub async fn fetch_by_doi(&self, doi: &Doi) -> Result<SciHubResult> {
        let (html, mirror) = self.fetch_with_mirror_rotation(&doi.normalized).await?;
        self.parse_scihub_page(&html, &mirror)
    }

    pub async fn download_pdf(
        &self,
        doi: &Doi,
        output_dir: &Path,
        filename: Option<&str>,
    ) -> Result<PathBuf> {
        let result = self.fetch_by_doi(doi).await?;
        let pdf_url = result
            .pdf_url
            .ok_or_else(|| ScienceError::Parse("Sci-Hub page has no PDF URL".to_string()))?;

        let bytes = self.client.get_bytes(&pdf_url).await?;
        tokio::fs::create_dir_all(output_dir)
            .await
            .map_err(|e| ScienceError::PdfExtraction(e.to_string()))?;

        let final_name = filename
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| default_filename_for_doi(doi));
        let path = output_dir.join(final_name);
        tokio::fs::write(&path, bytes)
            .await
            .map_err(|e| ScienceError::PdfExtraction(e.to_string()))?;
        Ok(path)
    }

    pub fn parse_scihub_page(&self, html: &str, mirror: &str) -> Result<SciHubResult> {
        let iframe_selector = parse_selector("iframe#pdf, iframe[src*='.pdf']")?;
        let embed_selector = parse_selector("embed[type='application/pdf'], embed[src*='.pdf']")?;
        let citation_selector = parse_selector("#citation, #title, h1, title")?;

        let document = Html::parse_document(html);

        let pdf_url = document
            .select(&iframe_selector)
            .next()
            .and_then(|el| el.value().attr("src"))
            .or_else(|| {
                document
                    .select(&embed_selector)
                    .next()
                    .and_then(|el| el.value().attr("src"))
            })
            .map(|src| normalize_scihub_url(src, mirror));

        let title = document
            .select(&citation_selector)
            .next()
            .map(|el| normalize_whitespace(&el.text().collect::<Vec<_>>().join(" ")))
            .filter(|s| !s.is_empty());

        Ok(SciHubResult { pdf_url, title })
    }

    async fn find_working_mirror(&self) -> Result<String> {
        for mirror in self.mirror_order().await {
            if self.check_mirror(&mirror).await {
                return Ok(mirror);
            }
        }
        Err(ScienceError::NoMirror("scihub".to_string()))
    }

    async fn check_mirror(&self, mirror: &str) -> bool {
        self.client.get(mirror).await.is_ok()
    }

    async fn mirror_order(&self) -> Vec<String> {
        let active = self.working_mirror.read().await.clone();
        let mut order = Vec::new();
        if let Some(active) = active {
            order.push(active);
        }
        for mirror in &self.mirrors {
            if !order.contains(mirror) {
                order.push(mirror.clone());
            }
        }
        order
    }

    async fn fetch_with_mirror_rotation(&self, doi_path: &str) -> Result<(String, String)> {
        let mut last_error: Option<ScienceError> = None;
        for mirror in self.mirror_order().await {
            let url = format!(
                "{}/{}",
                mirror.trim_end_matches('/'),
                doi_path.trim_start_matches('/')
            );
            match self.client.get(&url).await {
                Ok(body) => {
                    *self.working_mirror.write().await = Some(mirror.clone());
                    return Ok((body, mirror));
                }
                Err(err) => {
                    last_error = Some(err);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| ScienceError::NoMirror("scihub".to_string())))
    }

    fn with_config(mirrors: Vec<String>, min_interval: Duration) -> Self {
        let mirrors = if mirrors.is_empty() {
            KNOWN_MIRRORS.iter().map(|m| (*m).to_string()).collect()
        } else {
            mirrors
        };
        Self {
            client: RateLimitedClient::new(min_interval, 3, "omniscope-science/0.1"),
            working_mirror: Arc::new(RwLock::new(None)),
            mirrors,
        }
    }
}

impl Default for SciHubSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExternalSource for SciHubSource {
    fn name() -> &'static str {
        "scihub"
    }

    fn source_type() -> SourceType {
        SourceType::Search
    }

    fn requires_auth() -> bool {
        false
    }

    fn rate_limit() -> RateLimit {
        RateLimit {
            requests_per_second: 0.5,
        }
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let doi = match Doi::parse(query) {
            Ok(doi) => doi,
            Err(_) => return Ok(Vec::new()),
        };

        let item = self.fetch_by_doi(&doi).await?;
        let mut title = item.title.unwrap_or_else(|| doi.normalized.clone());
        if title.is_empty() {
            title = doi.normalized.clone();
        }

        Ok(vec![SearchResult {
            title,
            authors: Vec::new(),
            year: None,
            identifier: Some(doi.normalized),
            source: Self::name().to_string(),
            relevance_score: if item.pdf_url.is_some() { 100.0 } else { 50.0 },
        }])
    }

    async fn fetch_metadata(&self, id: &str) -> Result<Option<Metadata>> {
        let doi = match Doi::parse(id) {
            Ok(doi) => doi,
            Err(_) => return Ok(None),
        };
        let item = self.fetch_by_doi(&doi).await?;
        Ok(Some(Metadata {
            title: item.title.unwrap_or_else(|| doi.normalized.clone()),
            authors: Vec::new(),
            year: None,
            abstract_text: None,
            doi: Some(doi.normalized),
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
        let item = self.fetch_by_doi(&doi).await?;
        Ok(item.pdf_url.map(|url| DownloadUrl {
            url,
            source_name: Self::name().to_string(),
            requires_redirect: false,
        }))
    }

    async fn health_check(&self) -> SourceStatus {
        let start = Instant::now();
        let mirror = self.find_working_mirror().await.ok();
        if let Some(found) = &mirror {
            *self.working_mirror.write().await = Some(found.clone());
        }

        SourceStatus {
            available: mirror.is_some(),
            latency_ms: Some(start.elapsed().as_millis() as u64),
            last_checked: Some(Utc::now()),
            mirror,
        }
    }
}

fn parse_selector(input: &str) -> Result<Selector> {
    Selector::parse(input)
        .map_err(|e| ScienceError::Parse(format!("invalid selector {input}: {e}")))
}

fn normalize_scihub_url(src: &str, mirror: &str) -> String {
    if src.starts_with("//") {
        return format!("https:{src}");
    }
    if src.starts_with("http://") || src.starts_with("https://") {
        return src.to_string();
    }
    if src.starts_with('/') {
        return format!("{}{}", mirror.trim_end_matches('/'), src);
    }
    format!("{}/{}", mirror.trim_end_matches('/'), src)
}

fn normalize_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn default_filename_for_doi(doi: &Doi) -> String {
    let safe = doi
        .normalized
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>();
    format!("{safe}.pdf")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_saved_scihub_fixture() {
        let fixture = include_str!("fixtures/scihub_page_fixture.html");
        let source = SciHubSource::new();
        let parsed = source
            .parse_scihub_page(fixture, "https://sci-hub.se")
            .unwrap();

        assert_eq!(parsed.title.as_deref(), Some("Attention Is All You Need"));
        assert_eq!(
            parsed.pdf_url.as_deref(),
            Some("https://sci-hub.se/downloads/2017/attention.pdf")
        );
    }
}
