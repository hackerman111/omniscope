#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crate::arxiv::parser::parse_atom_response;
use crate::arxiv::types::{ArxivMetadata, ArxivSearchQuery};
use crate::error::{Result, ScienceError};
use crate::http::{DiskCache, RateLimitedClient};
use crate::identifiers::arxiv::ArxivId;

const BASE_URL: &str = "http://export.arxiv.org/api/query";
const USER_AGENT: &str = "omniscope-science/0.1";
const CACHE_TTL_SECS: u64 = 7 * 24 * 60 * 60;
const DEFAULT_MAX_RESULTS: u32 = 25;

pub struct ArxivClient {
    http: RateLimitedClient,
    cache: DiskCache,
    base_url: String,
}

impl Default for ArxivClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ArxivClient {
    pub fn new() -> Self {
        Self::with_config(
            BASE_URL.to_string(),
            Duration::from_secs(3),
            Duration::from_secs(CACHE_TTL_SECS),
            "arxiv_api".to_string(),
        )
    }

    pub async fn fetch_metadata(&self, id: &ArxivId) -> Result<ArxivMetadata> {
        let cache_key = format!("metadata:{}", id.id);
        if let Some(cached) = self.cache.get::<ArxivMetadata>(&cache_key).await {
            return Ok(cached);
        }

        let url = format!("{}?id_list={}", self.base_url, id.id);
        let xml = self.http.get(&url).await?;
        let mut parsed = parse_atom_response(&xml)?;
        let metadata = parsed
            .pop()
            .ok_or_else(|| ScienceError::Parse(format!("arXiv entry not found: {}", id.id)))?;

        self.cache.set(&cache_key, &metadata).await;
        Ok(metadata)
    }

    pub async fn search(&self, query: &ArxivSearchQuery) -> Result<Vec<ArxivMetadata>> {
        let search_query = query.to_query_string();
        let max_results = query.max_results.unwrap_or(DEFAULT_MAX_RESULTS);
        let start = query.start.unwrap_or(0);
        let sort_by = query
            .sort_by
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("relevance");

        let cache_key =
            format!("search:{search_query}:start={start}:max={max_results}:sort={sort_by}");
        if let Some(cached) = self.cache.get::<Vec<ArxivMetadata>>(&cache_key).await {
            return Ok(cached);
        }

        let mut url = format!(
            "{}?search_query={search_query}&max_results={max_results}&start={start}&sortBy={sort_by}",
            self.base_url
        );

        if !query.id_list.is_empty() {
            let id_list = query.id_list.join(",");
            url.push_str(&format!("&id_list={id_list}"));
        }

        let xml = self.http.get(&url).await?;
        let parsed = parse_atom_response(&xml)?;
        self.cache.set(&cache_key, &parsed).await;
        Ok(parsed)
    }

    pub async fn check_for_updates(
        &self,
        id: &ArxivId,
        current_version: Option<u8>,
    ) -> Result<Option<ArxivMetadata>> {
        let latest = self.fetch_metadata(id).await?;
        let latest_version = latest.arxiv_id.version;

        let has_update = match (current_version, latest_version) {
            (Some(current), Some(found)) => found > current,
            (Some(_), None) => false,
            (None, Some(_)) => true,
            (None, None) => false,
        };

        if has_update {
            Ok(Some(latest))
        } else {
            Ok(None)
        }
    }

    fn with_config(
        base_url: String,
        min_interval: Duration,
        cache_ttl: Duration,
        cache_namespace: String,
    ) -> Self {
        Self {
            http: RateLimitedClient::new(min_interval, 3, USER_AGENT),
            cache: DiskCache::new(&cache_namespace, cache_ttl),
            base_url,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_tests(base_url: String) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let cache_namespace = format!(
            "arxiv_test_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        Self::with_config(
            base_url,
            Duration::from_millis(1),
            Duration::from_secs(60),
            cache_namespace,
        )
    }
}

#[cfg(test)]
mod tests {
    use mockito::{Matcher, Server};

    use super::*;

    const ATTENTION_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom"
      xmlns:arxiv="http://arxiv.org/schemas/atom">
  <entry>
    <id>http://arxiv.org/abs/1706.03762v7</id>
    <updated>2023-08-02T17:54:37Z</updated>
    <published>2017-06-12T17:57:40Z</published>
    <title>Attention Is All You Need</title>
    <summary>Transformer architecture.</summary>
    <author><name>Ashish Vaswani</name><arxiv:affiliation>Google Brain</arxiv:affiliation></author>
    <arxiv:doi>10.48550/arXiv.1706.03762</arxiv:doi>
    <link rel="related" type="application/pdf" href="http://arxiv.org/pdf/1706.03762v7" />
    <arxiv:primary_category term="cs.CL" />
    <category term="cs.CL" />
  </entry>
</feed>
"#;

    #[tokio::test]
    async fn fetch_metadata_parses_atom_response() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", "/api/query")
            .match_query(Matcher::UrlEncoded(
                "id_list".to_string(),
                "1706.03762".to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/atom+xml")
            .with_body(ATTENTION_XML)
            .expect(1)
            .create_async()
            .await;

        let client = ArxivClient::new_for_tests(format!("{}/api/query", server.url()));
        let arxiv_id = ArxivId::parse("1706.03762").unwrap();

        let metadata = client.fetch_metadata(&arxiv_id).await.unwrap();

        assert_eq!(metadata.arxiv_id.id, "1706.03762");
        assert_eq!(metadata.arxiv_id.version, Some(7));
        assert_eq!(metadata.title, "Attention Is All You Need");
        assert_eq!(metadata.primary_category, "cs.CL");
        assert_eq!(metadata.authors[0].name, "Ashish Vaswani");
        assert_eq!(
            metadata.doi.as_ref().map(|doi| doi.normalized.as_str()),
            Some("10.48550/arxiv.1706.03762")
        );

        mock.assert_async().await;
    }
}
