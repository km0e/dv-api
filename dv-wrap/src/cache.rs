use crate::error::Result;

mod sqlite;
pub use sqlite::SqliteCache;

#[async_trait::async_trait]
pub trait Cache {
    async fn get(&self, uid: &str, path: &str) -> Result<Option<(i64, i64)>>;
    async fn set(&self, uid: &str, path: &str, version: i64, latest: i64) -> Result<()>;
    async fn del(&self, uid: &str, path: &str) -> Result<()>;
}

#[derive(Default)]
pub struct MultiCache {
    caches: Vec<Box<dyn Cache + Sync + Send>>,
}

impl MultiCache {
    pub fn add_cache<C: Cache + Sync + Send + 'static>(&mut self, cache: C) {
        self.caches.push(Box::new(cache));
    }
    pub fn add_sqlite(&mut self, db_path: impl AsRef<std::path::Path>) {
        self.caches.push(Box::new(SqliteCache::new(db_path)));
    }
    pub async fn get(&self, uid: &str, path: &str) -> Result<Option<(i64, i64)>> {
        for cache in &self.caches {
            if let Ok(result) = cache.get(uid, path).await {
                if result.is_some() {
                    return Ok(result);
                }
            }
        }
        Ok(None)
    }

    pub async fn set(&self, uid: &str, path: &str, version: i64, latest: i64) -> Result<()> {
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
}
