use super::dev::{self, *};

mod sqlite;
pub use sqlite::Sqlite;
use tracing::warn;

#[async_trait::async_trait]
pub trait DB {
    async fn get(&self, uid: &str, path: &str) -> Result<Option<(String, String)>>;
    async fn set(&self, uid: &str, path: &str, version: &str, latest: &str) -> Result<()>;
    async fn del(&self, uid: &str, path: &str) -> Result<()>;
}

#[derive(Default)]
pub struct MultiDB {
    dbs: Vec<Box<dyn DB + Sync + Send>>,
    dir: Option<std::path::PathBuf>,
}

impl MultiDB {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_db<C: DB + Sync + Send + 'static>(&mut self, db: C) {
        self.dbs.push(Box::new(db));
    }

    pub fn add_sqlite(&mut self, db_path: impl AsRef<std::path::Path>) {
        self.dbs.push(Box::new(Sqlite::new(db_path)));
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
            anyhow::bail!(
                "Failed to parse version or latest as {} for uid: {}, path: {}",
                std::any::type_name::<T>(),
                uid,
                path
            )
        };
        Ok(Some((version, latest)))
    }
    pub async fn get(&self, uid: &str, path: &str) -> Result<Option<(String, String)>> {
        for db in &self.dbs {
            match db.get(uid, path).await {
                Ok(Some(result)) => return Ok(Some(result)),
                Ok(None) => continue,
                Err(e) => {
                    warn!("Error getting db for uid: {}, path: {}: {}", uid, path, e);
                }
            }
        }
        Ok(None)
    }

    pub async fn set(&self, uid: &str, path: &str, version: &str, latest: &str) -> Result<()> {
        for db in &self.dbs {
            db.set(uid, path, version, latest).await?;
        }
        Ok(())
    }

    pub async fn del(&self, uid: &str, path: &str) -> Result<()> {
        for db in &self.dbs {
            db.del(uid, path).await?;
        }
        Ok(())
    }
}
