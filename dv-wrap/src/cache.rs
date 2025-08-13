use crate::error::Result;

mod sqlite;
use dv_api::whatever;
pub use sqlite::SqliteCache;
use tracing::{debug, info, warn};

#[async_trait::async_trait]
pub trait Cache {
    async fn get(&self, uid: &str, path: &str) -> Result<Option<(String, String)>>;
    async fn set(&self, uid: &str, path: &str, version: &str, latest: &str) -> Result<()>;
    async fn del(&self, uid: &str, path: &str) -> Result<()>;
}

#[derive(Default)]
pub struct MultiCache {
    caches: Vec<Box<dyn Cache + Sync + Send>>,
    dir: Option<std::path::PathBuf>,
}

impl MultiCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_cache<C: Cache + Sync + Send + 'static>(&mut self, cache: C) {
        self.caches.push(Box::new(cache));
    }

    pub fn add_sqlite(&mut self, db_path: impl AsRef<std::path::Path>) {
        self.caches.push(Box::new(SqliteCache::new(db_path)));
    }

    pub fn set_dir(&mut self, dir: std::path::PathBuf) {
        self.dir = Some(dir);
    }
    pub async fn get_as<T: std::str::FromStr>(
        &self,
        uid: &str,
        path: &str,
    ) -> Result<Option<(T, T)>> {
        let result = self.get(uid, path).await?;
        let Some(result) = result else {
            return Ok(None);
        };
        let (Ok(version), Ok(latest)) = (result.0.parse::<T>(), result.1.parse::<T>()) else {
            dv_api::whatever!(
                "Failed to parse version or latest as {} for uid: {}, path: {}",
                std::any::type_name::<T>(),
                uid,
                path
            )
        };
        Ok(Some((version, latest)))
    }
    pub async fn get(&self, uid: &str, path: &str) -> Result<Option<(String, String)>> {
        for cache in &self.caches {
            match cache.get(uid, path).await {
                Ok(Some(result)) => return Ok(Some(result)),
                Ok(None) => continue,
                Err(e) => {
                    warn!(
                        "Error getting cache for uid: {}, path: {}: {}",
                        uid, path, e
                    );
                }
            }
        }
        Ok(None)
    }

    pub async fn set(&self, uid: &str, path: &str, version: &str, latest: &str) -> Result<()> {
        for cache in &self.caches {
            cache.set(uid, path, version, latest).await?;
        }
        Ok(())
    }

    pub async fn del(&self, uid: &str, path: &str) -> Result<()> {
        for cache in &self.caches {
            cache.del(uid, path).await?;
        }
        Ok(())
    }

    pub async fn cache_url(&self, url: &str) -> Result<String> {
        use base64::Engine;
        let name = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(url);
        let vl = self.get(&name, "").await?;
        let Ok(url2) = url.parse::<reqwest::Url>() else {
            whatever!("invalid url: {}", url)
        };
        let mut req = reqwest::Request::new(reqwest::Method::GET, url2);
        if let Some((version, _)) = &vl
            && !version.is_empty()
        {
            debug!(
                "Using cached version for url: {}, version: {}",
                url, version
            );
            req.headers_mut().insert(
                reqwest::header::IF_NONE_MATCH,
                reqwest::header::HeaderValue::from_str(version).unwrap(),
            );
        }
        let client = reqwest::Client::new();
        let Ok(resp) = client.execute(req).await else {
            whatever!("failed to fetch url: {}", url)
        };
        if resp.status().is_success() {
            let etag = resp
                .headers()
                .get(reqwest::header::ETAG)
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default()
                .to_string();
            let Ok(text) = resp.text().await else {
                whatever!("failed to read response text from url: {}", url)
            };
            if text.len() < 1024 * 1024 {
                self.set(&name, "", &etag, &text).await?;
            } else if let Some(dir) = self.dir.as_ref() {
                let path = dir.join(&name);
                tokio::fs::create_dir_all(&path.parent().unwrap()).await?;
                tokio::fs::write(&path, text.as_bytes()).await?;
                self.set(&name, "", &etag, "").await?;
            }
            Ok(text)
        } else if resp.status().as_u16() == 304 {
            info!("Cached response for url: {}", url);
            let content = vl.unwrap().1;
            if content.is_empty() {
                let Some(dir) = self.dir.as_ref() else {
                    whatever!("no dir set for caching, but got 304 for url: {}", url)
                };
                let path = dir.join(&name);
                Ok(tokio::fs::read_to_string(&path).await?)
            } else {
                Ok(content)
            }
        } else {
            whatever!("failed to fetch url: {}, status: {}", url, resp.status())
        }
    }
}
