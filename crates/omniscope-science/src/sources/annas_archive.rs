use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Url;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::{Result, ScienceError};
use crate::http::{DiskCache, RateLimitedClient};
use crate::identifiers::isbn::Isbn;
use crate::sources::{
    DownloadUrl as ExternalDownloadUrl, ExternalSource, Metadata, RateLimit, SearchResult,
    SourceStatus, SourceType,
};

const MIRRORS: &[&str] = &[
    "https://annas-archive.org",
    "https://annas-archive.se",
    "https://annas-archive.li",
    "https://annas-archive.gs",
];
const CACHE_TTL_SECS: u64 = 24 * 60 * 60;

static YEAR_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(?:19|20)\d{2}\b").expect("valid regex"));
static FORMAT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(pdf|epub|djvu|mobi|azw3|cbr|cbz)\b").expect("valid regex"));
static SIZE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(\d+(?:\.\d+)?)\s*(kb|mb|gb)\b").expect("valid regex"));
static LANG_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(en|ru|de|fr|es|it|pt|pl|uk|ja|ko|zh|english|russian|german|french|spanish|italian|portuguese|polish|ukrainian|japanese|korean|chinese)\b")
        .expect("valid regex")
});
static MD5_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b([a-f0-9]{32})\b").expect("valid regex"));
static ISBN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(?:isbn(?:-1[03])?:?\s*)?([0-9X][0-9X\-\s]{8,20}[0-9X])\b")
        .expect("valid regex")
});

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnnasQuery {
    pub q: String,
    pub ext: Vec<String>,
    pub lang: Option<String>,
    pub content: Option<String>,
}

impl AnnasQuery {
    pub fn from_text(query: &str) -> Self {
        Self {
            q: query.trim().to_string(),
            ext: Vec::new(),
            lang: None,
            content: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnnasResult {
    pub title: String,
    pub authors: Vec<String>,
    pub year: Option<i32>,
    pub format: String,
    pub size_mb: f64,
    pub language: String,
    pub md5: Option<String>,
    pub isbn: Option<String>,
    pub publisher: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DownloadLink {
    pub source: String,
    pub url: String,
    pub priority: u8,
}

pub struct AnnasArchiveSource {
    pub client: RateLimitedClient,
    pub active_mirror: Arc<RwLock<String>>,
    pub cache: DiskCache,
    mirrors: Vec<String>,
}

impl AnnasArchiveSource {
    pub fn new() -> Self {
        Self::with_config(
            MIRRORS.iter().map(|m| (*m).to_string()).collect(),
            Duration::from_secs(2),
            Duration::from_secs(CACHE_TTL_SECS),
            "annas_archive".to_string(),
        )
    }

    pub async fn search(&self, query: &AnnasQuery) -> Result<Vec<AnnasResult>> {
        if query.q.trim().is_empty() {
            return Ok(Vec::new());
        }

        let path = self.build_search_path(query)?;
        let cache_key = format!("search:{path}");
        if let Some(cached) = self.cache.get::<Vec<AnnasResult>>(&cache_key).await {
            return Ok(cached);
        }

        let html = self.fetch_with_mirror_rotation(&path).await?;
        let results = self.parse_search_html(&html)?;
        self.cache.set(&cache_key, &results).await;
        Ok(results)
    }

    pub async fn get_download_links(&self, md5: &str) -> Result<Vec<DownloadLink>> {
        let md5 = md5.trim().to_lowercase();
        if md5.is_empty() {
            return Ok(Vec::new());
        }

        let cache_key = format!("md5:{md5}");
        if let Some(cached) = self.cache.get::<Vec<DownloadLink>>(&cache_key).await {
            return Ok(cached);
        }

        let path = format!("/md5/{md5}");
        let html = self.fetch_with_mirror_rotation(&path).await?;
        let active_mirror = self.active_mirror.read().await.clone();
        let links = self.parse_download_links(&html, &active_mirror)?;
        self.cache.set(&cache_key, &links).await;
        Ok(links)
    }

    pub fn parse_search_html(&self, html: &str) -> Result<Vec<AnnasResult>> {
        let card_selector = parse_selector(
            "div.h-\\[125px\\], div[class*='h-[125px]'], div.search-result, article",
        )?;
        let title_selector = parse_selector("h3, a[href*='/md5/']")?;
        let meta_selector = parse_selector("div.text-sm, div.metadata, span.metadata")?;
        let link_selector = parse_selector("a[href]")?;

        let document = Html::parse_document(html);
        let mut seen = HashSet::new();
        let mut out = Vec::new();

        for card in document.select(&card_selector) {
            let title = card
                .select(&title_selector)
                .next()
                .map(|el| element_text(&el))
                .unwrap_or_default();
            if title.is_empty() {
                continue;
            }

            let link_href = card
                .select(&link_selector)
                .filter_map(|a| a.value().attr("href"))
                .find(|href| href.contains("/md5/"));
            let md5 = link_href.and_then(extract_md5_from_url);

            let mut meta_lines = card
                .select(&meta_selector)
                .map(|el| element_text(&el))
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>();
            if meta_lines.is_empty() {
                let fallback = element_text(&card);
                if !fallback.is_empty() {
                    meta_lines.push(fallback);
                }
            }

            let year = parse_year_from_meta(&meta_lines);
            let format = parse_format_from_meta(&meta_lines).unwrap_or_else(|| "unknown".into());
            let size_mb = parse_size_from_meta(&meta_lines).unwrap_or(0.0);
            let language = parse_lang_from_meta(&meta_lines).unwrap_or_else(|| "unknown".into());
            let authors = parse_authors_from_meta(&meta_lines);
            let isbn = parse_isbn_from_meta(&meta_lines);
            let publisher = parse_publisher_from_meta(&meta_lines);

            let dedup_key = md5.clone().unwrap_or_else(|| title.to_lowercase());
            if seen.insert(dedup_key) {
                out.push(AnnasResult {
                    title,
                    authors,
                    year,
                    format,
                    size_mb,
                    language,
                    md5,
                    isbn,
                    publisher,
                });
            }
        }

        Ok(out)
    }

    fn parse_download_links(&self, html: &str, active_mirror: &str) -> Result<Vec<DownloadLink>> {
        let link_selector = parse_selector("a[href]")?;
        let document = Html::parse_document(html);

        let mut seen = HashSet::new();
        let mut links = Vec::new();
        for anchor in document.select(&link_selector) {
            let Some(href) = anchor.value().attr("href") else {
                continue;
            };
            let Some(url) = normalize_url(href, active_mirror) else {
                continue;
            };

            let label = element_text(&anchor);
            if !is_download_candidate(&label, &url) {
                continue;
            }

            let source = infer_source_name(&label, &url);
            let priority = download_priority(&source, &url);
            if seen.insert(url.clone()) {
                links.push(DownloadLink {
                    source,
                    url,
                    priority,
                });
            }
        }

        links.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.source.cmp(&b.source)));
        Ok(links)
    }

    async fn fetch_with_mirror_rotation(&self, path_and_query: &str) -> Result<String> {
        let path = if path_and_query.starts_with('/') {
            path_and_query.to_string()
        } else {
            format!("/{path_and_query}")
        };

        let mut last_error: Option<ScienceError> = None;
        for mirror in self.mirror_order().await {
            let url = format!("{}{}", mirror.trim_end_matches('/'), path);
            match self.client.get(&url).await {
                Ok(body) => {
                    *self.active_mirror.write().await = mirror;
                    return Ok(body);
                }
                Err(err) => {
                    last_error = Some(err);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| ScienceError::NoMirror("annas_archive".to_string())))
    }

    fn build_search_path(&self, query: &AnnasQuery) -> Result<String> {
        let mut url = Url::parse("https://annas-archive.org/search")
            .map_err(|e| ScienceError::Parse(e.to_string()))?;
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("q", query.q.trim());
            if !query.ext.is_empty() {
                qp.append_pair("ext", &query.ext.join(","));
            }
            if let Some(lang) = query
                .lang
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                qp.append_pair("lang", lang);
            }
            if let Some(content) = query
                .content
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                qp.append_pair("content", content);
            }
        }

        Ok(match url.query() {
            Some(q) => format!("/search?{q}"),
            None => "/search".to_string(),
        })
    }

    async fn mirror_order(&self) -> Vec<String> {
        let active = self.active_mirror.read().await.clone();
        let mut order = Vec::new();
        if !active.is_empty() {
            order.push(active);
        }
        for mirror in &self.mirrors {
            if !order.contains(mirror) {
                order.push(mirror.clone());
            }
        }
        order
    }

    fn with_config(
        mirrors: Vec<String>,
        min_interval: Duration,
        cache_ttl: Duration,
        cache_namespace: String,
    ) -> Self {
        let mirrors = if mirrors.is_empty() {
            MIRRORS.iter().map(|m| (*m).to_string()).collect()
        } else {
            mirrors
        };
        let active = mirrors.first().cloned().unwrap_or_default();
        Self {
            client: RateLimitedClient::new(min_interval, 3, "omniscope-science/0.1"),
            active_mirror: Arc::new(RwLock::new(active)),
            cache: DiskCache::new(&cache_namespace, cache_ttl),
            mirrors,
        }
    }
}

impl Default for AnnasArchiveSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExternalSource for AnnasArchiveSource {
    fn name() -> &'static str {
        "annas_archive"
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
        let query = AnnasQuery::from_text(query);
        let results = self.search(&query).await?;
        Ok(results
            .into_iter()
            .map(|item| SearchResult {
                title: item.title,
                authors: item.authors,
                year: item.year,
                identifier: item.md5,
                source: Self::name().to_string(),
                relevance_score: 100.0,
            })
            .collect())
    }

    async fn fetch_metadata(&self, _id: &str) -> Result<Option<Metadata>> {
        Ok(None)
    }

    async fn find_download_url(&self, id: &str) -> Result<Option<ExternalDownloadUrl>> {
        let links = self.get_download_links(id).await?;
        Ok(links.first().map(|link| ExternalDownloadUrl {
            url: link.url.clone(),
            source_name: format!("annas_archive:{}", link.source),
            requires_redirect: true,
        }))
    }

    async fn health_check(&self) -> SourceStatus {
        let start = Instant::now();
        let available = self
            .fetch_with_mirror_rotation("/search?q=test")
            .await
            .is_ok();
        let mirror = Some(self.active_mirror.read().await.clone());
        SourceStatus {
            available,
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

fn element_text(element: &scraper::ElementRef<'_>) -> String {
    normalize_whitespace(&element.text().collect::<Vec<_>>().join(" "))
}

fn normalize_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_authors_from_meta(meta_lines: &[String]) -> Vec<String> {
    for line in meta_lines {
        let lower = line.to_lowercase();
        if lower.contains("pdf")
            || lower.contains("epub")
            || lower.contains("djvu")
            || lower.contains("mb")
            || YEAR_RE.is_match(line)
        {
            continue;
        }
        let authors = line
            .split(';')
            .map(str::trim)
            .filter(|part| !part.is_empty() && part.len() > 2)
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if !authors.is_empty() {
            return authors;
        }
    }
    Vec::new()
}

pub fn parse_year_from_meta(meta_lines: &[String]) -> Option<i32> {
    let combined = meta_lines.join(" ");
    YEAR_RE
        .find(&combined)
        .and_then(|m| m.as_str().parse::<i32>().ok())
}

pub fn parse_format_from_meta(meta_lines: &[String]) -> Option<String> {
    let combined = meta_lines.join(" ");
    FORMAT_RE
        .captures(&combined)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_lowercase())
}

pub fn parse_size_from_meta(meta_lines: &[String]) -> Option<f64> {
    let combined = meta_lines.join(" ");
    let caps = SIZE_RE.captures(&combined)?;
    let amount = caps.get(1)?.as_str().parse::<f64>().ok()?;
    let unit = caps.get(2)?.as_str().to_lowercase();
    Some(match unit.as_str() {
        "gb" => amount * 1024.0,
        "kb" => amount / 1024.0,
        _ => amount,
    })
}

pub fn parse_lang_from_meta(meta_lines: &[String]) -> Option<String> {
    let combined = meta_lines.join(" ");
    let token = LANG_RE
        .captures(&combined)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_lowercase())?;
    Some(match token.as_str() {
        "english" | "en" => "en".to_string(),
        "russian" | "ru" => "ru".to_string(),
        "german" | "de" => "de".to_string(),
        "french" | "fr" => "fr".to_string(),
        "spanish" | "es" => "es".to_string(),
        "italian" | "it" => "it".to_string(),
        "portuguese" | "pt" => "pt".to_string(),
        "polish" | "pl" => "pl".to_string(),
        "ukrainian" | "uk" => "uk".to_string(),
        "japanese" | "ja" => "ja".to_string(),
        "korean" | "ko" => "ko".to_string(),
        "chinese" | "zh" => "zh".to_string(),
        _ => token,
    })
}

fn parse_publisher_from_meta(meta_lines: &[String]) -> Option<String> {
    meta_lines
        .iter()
        .find_map(|line| {
            let lower = line.to_lowercase();
            if lower.contains("press")
                || lower.contains("publisher")
                || lower.contains("springer")
                || lower.contains("wiley")
                || lower.contains("elsevier")
            {
                Some(line.trim().to_string())
            } else {
                None
            }
        })
        .filter(|s| !s.is_empty())
}

fn parse_isbn_from_meta(meta_lines: &[String]) -> Option<String> {
    let combined = meta_lines.join(" ");
    for caps in ISBN_RE.captures_iter(&combined) {
        let Some(raw) = caps.get(1).map(|m| m.as_str()) else {
            continue;
        };
        if let Ok(isbn) = Isbn::parse(raw) {
            return Some(isbn.isbn13);
        }
    }
    None
}

fn extract_md5_from_url(url: &str) -> Option<String> {
    MD5_RE
        .captures(url)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_lowercase())
}

fn normalize_url(raw: &str, active_mirror: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.starts_with("javascript:") {
        return None;
    }
    if raw.starts_with("magnet:") {
        return Some(raw.to_string());
    }
    if raw.starts_with("//") {
        return Some(format!("https:{raw}"));
    }
    if raw.starts_with("http://") || raw.starts_with("https://") {
        return Some(raw.to_string());
    }
    if raw.starts_with('/') {
        return Some(format!("{}{}", active_mirror.trim_end_matches('/'), raw));
    }
    Some(format!(
        "{}/{}",
        active_mirror.trim_end_matches('/'),
        raw.trim_start_matches('/')
    ))
}

fn is_download_candidate(label: &str, url: &str) -> bool {
    let label = label.to_lowercase();
    let url = url.to_lowercase();
    url.contains("libgen")
        || url.contains("ipfs")
        || url.contains("download")
        || url.contains("z-lib")
        || url.starts_with("magnet:")
        || label.contains("download")
        || label.contains("mirror")
        || label.contains("libgen")
        || label.contains("ipfs")
}

fn infer_source_name(label: &str, url: &str) -> String {
    let text = label.trim();
    if !text.is_empty() {
        return text.to_string();
    }
    Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| "unknown".to_string())
}

fn download_priority(source: &str, url: &str) -> u8 {
    let source = source.to_lowercase();
    let url = url.to_lowercase();
    if source.contains("libgen.li") || url.contains("libgen.li") {
        100
    } else if source.contains("libgen") || url.contains("libgen") {
        90
    } else if source.contains("ipfs") || url.contains("ipfs") {
        70
    } else if source.contains("z-lib") || url.contains("z-lib") {
        60
    } else if source.contains("annas") || url.contains("annas-archive") {
        40
    } else {
        50
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_saved_search_fixture() {
        let fixture = include_str!("fixtures/annas_search_fixture.html");
        let source = AnnasArchiveSource::new();
        let results = source.parse_search_html(fixture).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Attention Is All You Need");
        assert_eq!(
            results[0].md5.as_deref(),
            Some("0123456789abcdef0123456789abcdef")
        );
        assert_eq!(results[0].format, "pdf");
        assert_eq!(results[0].year, Some(2017));
        assert_eq!(results[0].language, "en");
        assert!((results[0].size_mb - 1.2).abs() < 0.01);
    }

    #[test]
    fn parses_download_links() {
        let fixture = include_str!("fixtures/annas_md5_fixture.html");
        let source = AnnasArchiveSource::new();
        let links = source
            .parse_download_links(fixture, "https://annas-archive.org")
            .unwrap();

        assert_eq!(links.len(), 3);
        assert_eq!(links[0].source, "libgen.li");
        assert!(links[0].url.contains("libgen.li"));
    }

    #[test]
    fn parses_meta_helpers() {
        let meta = vec![
            "Vaswani, Ashish; Shazeer, Noam".to_string(),
            "English, PDF, 1.2 MB, 2017".to_string(),
        ];
        assert_eq!(parse_year_from_meta(&meta), Some(2017));
        assert_eq!(parse_format_from_meta(&meta).as_deref(), Some("pdf"));
        assert_eq!(parse_lang_from_meta(&meta).as_deref(), Some("en"));
        assert!(parse_size_from_meta(&meta).is_some());
    }
}
