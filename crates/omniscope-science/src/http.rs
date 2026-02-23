use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::header::{HeaderMap, RETRY_AFTER};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;
use tokio::time::sleep;

use crate::error::{Result, ScienceError};

// ─── RateLimitedClient ────────────────────────────────────────────────────────

pub struct RateLimitedClient {
    client: reqwest::Client,
    min_interval: Duration,
    last_request: Arc<Mutex<Option<Instant>>>,
    max_retries: u32,
}

impl RateLimitedClient {
    pub fn new(min_interval: Duration, max_retries: u32, user_agent: &str) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .gzip(true)
            .build()
            .expect("failed to build reqwest client");
        Self {
            client,
            min_interval,
            last_request: Arc::new(Mutex::new(None)),
            max_retries,
        }
    }

    async fn wait_for_rate_limit(&self) {
        let mut last = self.last_request.lock().await;
        if let Some(t) = *last {
            let elapsed = t.elapsed();
            if elapsed < self.min_interval {
                sleep(self.min_interval - elapsed).await;
            }
        }
        *last = Some(Instant::now());
    }

    pub async fn get(&self, url: &str) -> Result<String> {
        self.get_with_headers(url, HeaderMap::new()).await
    }

    pub async fn get_with_headers(&self, url: &str, headers: HeaderMap) -> Result<String> {
        let mut attempt = 0u32;
        loop {
            self.wait_for_rate_limit().await;
            let resp = self.client.get(url).headers(headers.clone()).send().await;
            match resp {
                Ok(r) if r.status() == 429 => {
                    if attempt >= self.max_retries {
                        return Err(ScienceError::RateLimit("server".to_string(), 60));
                    }
                    let wait = r
                        .headers()
                        .get(RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(60);
                    sleep(Duration::from_secs(wait)).await;
                    attempt += 1;
                }
                Ok(r) if !r.status().is_success() => {
                    let status = r.status().as_u16();
                    let body = r.text().await.unwrap_or_default();
                    return Err(ScienceError::ApiError(
                        url.to_string(),
                        format!("HTTP {status}: {body}"),
                    ));
                }
                Ok(r) => return r.text().await.map_err(ScienceError::Http),
                Err(e) => {
                    if attempt >= self.max_retries {
                        return Err(ScienceError::Http(e));
                    }
                    let backoff = 2u64.pow(attempt);
                    sleep(Duration::from_secs(backoff)).await;
                    attempt += 1;
                }
            }
        }
    }

    pub async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let text = self.get(url).await?;
        serde_json::from_str(&text).map_err(|e| ScienceError::Parse(e.to_string()))
    }

    pub async fn post_json<B: Serialize, R: DeserializeOwned>(
        &self,
        url: &str,
        body: &B,
    ) -> Result<R> {
        self.post_json_with_headers(url, body, HeaderMap::new())
            .await
    }

    pub async fn post_json_with_headers<B: Serialize, R: DeserializeOwned>(
        &self,
        url: &str,
        body: &B,
        headers: HeaderMap,
    ) -> Result<R> {
        let mut attempt = 0u32;
        loop {
            self.wait_for_rate_limit().await;
            let resp = self
                .client
                .post(url)
                .headers(headers.clone())
                .json(body)
                .send()
                .await;

            match resp {
                Ok(r) if r.status() == 429 => {
                    if attempt >= self.max_retries {
                        return Err(ScienceError::RateLimit("server".to_string(), 60));
                    }
                    let wait = r
                        .headers()
                        .get(RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(60);
                    sleep(Duration::from_secs(wait)).await;
                    attempt += 1;
                }
                Ok(r) if !r.status().is_success() => {
                    let status = r.status().as_u16();
                    let msg = r.text().await.unwrap_or_default();
                    return Err(ScienceError::ApiError(
                        url.to_string(),
                        format!("HTTP {status}: {msg}"),
                    ));
                }
                Ok(r) => {
                    let text = r.text().await.map_err(ScienceError::Http)?;
                    return serde_json::from_str(&text)
                        .map_err(|e| ScienceError::Parse(e.to_string()));
                }
                Err(e) => {
                    if attempt >= self.max_retries {
                        return Err(ScienceError::Http(e));
                    }
                    let backoff = 2u64.pow(attempt);
                    sleep(Duration::from_secs(backoff)).await;
                    attempt += 1;
                }
            }
        }
    }
}

// ─── DiskCache ────────────────────────────────────────────────────────────────

pub struct DiskCache {
    dir: PathBuf,
    ttl: Duration,
}

fn cache_key_to_path(dir: &Path, key: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    let hash = hasher.finish();
    dir.join(format!("{hash:016x}.json"))
}

#[derive(Serialize, serde::Deserialize)]
struct CacheEntry<T> {
    stored_at: u64, // Unix timestamp secs
    value: T,
}

impl DiskCache {
    pub fn new(namespace: &str, ttl: Duration) -> Self {
        let dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("omniscope")
            .join("cache")
            .join(namespace);
        let _ = std::fs::create_dir_all(&dir);
        Self { dir, ttl }
    }

    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let path = cache_key_to_path(&self.dir, key);
        let data = tokio::fs::read(&path).await.ok()?;
        let entry: CacheEntry<T> = serde_json::from_slice(&data).ok()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if now.saturating_sub(entry.stored_at) > self.ttl.as_secs() {
            let _ = tokio::fs::remove_file(&path).await;
            return None;
        }
        Some(entry.value)
    }

    pub async fn set<T: Serialize>(&self, key: &str, value: &T) {
        let path = cache_key_to_path(&self.dir, key);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let entry = CacheEntry {
            stored_at: now,
            value,
        };
        if let Ok(data) = serde_json::to_vec(&entry) {
            let _ = tokio::fs::write(&path, data).await;
        }
    }

    pub async fn invalidate(&self, key: &str) {
        let path = cache_key_to_path(&self.dir, key);
        let _ = tokio::fs::remove_file(&path).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cache_set_get_roundtrip() {
        let ttl = Duration::from_secs(60);
        let cache = DiskCache::new("test_roundtrip", ttl);
        cache.set("key1", &"hello world").await;
        let val: Option<String> = cache.get("key1").await;
        assert_eq!(val, Some("hello world".to_string()));
    }

    #[tokio::test]
    async fn cache_expired_returns_none() {
        let ttl = Duration::from_secs(0); // immediate expiry
        let cache = DiskCache::new("test_expired", ttl);
        cache.set("key_exp", &42u32).await;
        // Sleep 1s to ensure TTL passes
        sleep(Duration::from_millis(1100)).await;
        let val: Option<u32> = cache.get("key_exp").await;
        assert_eq!(val, None);
    }
}
