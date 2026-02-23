use std::time::Duration;
use crate::error::Result;
use crate::http::{RateLimitedClient, DiskCache};
use crate::arxiv::types::{ArxivMetadata, ArxivSearchQuery};
use crate::identifiers::arxiv::ArxivId;
use crate::arxiv::parser::parse_atom_response;

pub struct ArxivClient {
    client: RateLimitedClient,
    cache: DiskCache,
    base_url: String,
}

impl ArxivClient {
    pub fn new() -> Self {
        Self::with_params("http://export.arxiv.org/api/query", Duration::from_secs(3))
    }

    pub fn with_base_url(base_url: &str) -> Self {
        Self::with_params(base_url, Duration::from_secs(3))
    }

    pub fn with_params(base_url: &str, min_interval: Duration) -> Self {
        Self {
            client: RateLimitedClient::new(min_interval, 3, "omniscope/0.1"),
            cache: DiskCache::new("arxiv", Duration::from_secs(7 * 24 * 3600)), // 7 days
            base_url: base_url.to_string(),
        }
    }

    pub async fn fetch_metadata(&self, id: &ArxivId) -> Result<ArxivMetadata> {
        let key = format!("metadata:{}", id.raw);
        if let Some(cached) = self.cache.get::<ArxivMetadata>(&key).await {
            return Ok(cached);
        }

        let url = if self.base_url.contains('?') {
            format!("{}&id_list={}", self.base_url, id.id)
        } else {
            format!("{}?id_list={}", self.base_url, id.id)
        };
        
        let xml = self.client.get(&url).await?;
        let results = parse_atom_response(&xml)?;
        
        let metadata = results.into_iter().next().ok_or_else(|| {
            crate::error::ScienceError::ApiError(url, "No results found for ID".to_string())
        })?;

        self.cache.set(&key, &metadata).await;
        Ok(metadata)
    }

    pub async fn search(&self, query: &ArxivSearchQuery) -> Result<Vec<ArxivMetadata>> {
        let query_str = query.to_query_string();
        let key = format!("search:{}", query_str);
        
        if let Some(cached) = self.cache.get::<Vec<ArxivMetadata>>(&key).await {
            return Ok(cached);
        }

        let url = if self.base_url.contains('?') {
            format!(
                "{}&search_query={}&max_results={}",
                self.base_url,
                query_str,
                query.max_results.unwrap_or(10)
            )
        } else {
            format!(
                "{}?search_query={}&max_results={}",
                self.base_url,
                query_str,
                query.max_results.unwrap_or(10)
            )
        };
        
        let xml = self.client.get(&url).await?;
        let results = parse_atom_response(&xml)?;

        self.cache.set(&key, &results).await;
        Ok(results)
    }
}

impl Default for ArxivClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ArxivClient {
    pub async fn check_for_updates(
        &self,
        id: &ArxivId,
        current_version: Option<u8>,
    ) -> Result<Option<ArxivMetadata>> {
        let metadata = self.fetch_metadata(id).await?;
        
        // If the local database has no concept of version, OR
        // the remote version is strictly greater than our local version,
        // we consider it an update.
        let has_update = match (metadata.arxiv_id.version, current_version) {
            (Some(remote), Some(local)) => remote > local,
            (Some(_remote), None) => true,
            _ => false,
        };

        if has_update {
            Ok(Some(metadata))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    #[tokio::test]
    async fn test_arxiv_client_fetch_metadata() {
        let mut server = Server::new_async().await;
        let base_url = server.url();

        let _m = server.mock("GET", "/query?id_list=1706.03762")
            .with_status(200)
            .with_header("content-type", "application/atom+xml")
            .with_body(r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <entry>
    <id>http://arxiv.org/abs/1706.03762v5</id>
    <updated>2023-08-02T03:09:44Z</updated>
    <published>2017-06-12T17:57:34Z</published>
    <title>Attention Is All You Need</title>
    <summary>Abstract</summary>
    <author><name>Ashish Vaswani</name></author>
    <arxiv:primary_category xmlns:arxiv="http://arxiv.org/schemas/atom" term="cs.CL"/>
    <category term="cs.CL"/>
  </entry>
</feed>"#)
            .create_async().await;

        let client = ArxivClient::with_params(
            &format!("{}/query", base_url),
            Duration::from_secs(0)
        );
        let id = ArxivId::parse("1706.03762").unwrap();
        let result = client.fetch_metadata(&id).await.unwrap();

        assert_eq!(result.title, "Attention Is All You Need");
    }
}
