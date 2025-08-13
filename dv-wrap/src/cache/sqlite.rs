use std::path::Path;
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::error::{Error, Result};

#[derive(Debug)]
pub struct SqliteCache {
    conn: Mutex<rusqlite::Connection>,
}

impl SqliteCache {
    pub fn new(db_path: impl AsRef<Path>) -> Self {
        let db_path = db_path.as_ref();
        info!("use sqlite path {}", db_path.display());
        let conn = rusqlite::Connection::open(db_path).expect("open sqlite connection");
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cache (
                device TEXT NOT NULL,
                key TEXT NOT NULL,
                version TEXT NOT NULL,
                latest TEXT NOT NULL,
                PRIMARY KEY (device, path)
            )",
            [],
        )
        .expect("create initial table");
        Self {
            conn: Mutex::new(conn),
        }
    }
    #[cfg(test)]
    pub fn memory() -> Self {
        let conn = rusqlite::Connection::open_in_memory().expect("open sqlite connection");
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cache (
                device TEXT NOT NULL,
                key TEXT NOT NULL,
                version TEXT NOT NULL,
                latest TEXT NOT NULL,
                PRIMARY KEY (device, path)
            )",
            [],
        )
        .expect("create initial table");
        Self {
            conn: Mutex::new(conn),
        }
    }
}

#[async_trait::async_trait]
impl super::Cache for SqliteCache {
    async fn get(&self, uid: &str, key: &str) -> Result<Option<(String, String)>> {
        let row = self.conn.lock().await.query_row(
            "SELECT version, latest FROM cache WHERE device = ? AND key = ?",
            [uid, key],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        );
        match row {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(Error::unknown(e)),
        }
    }
    async fn set(&self, uid: &str, key: &str, version: &str, latest: &str) -> Result<()> {
        debug!("cache set: {} {} {} {}", uid, key, version, latest);
        self.conn
            .lock()
            .await
            .execute(
                "INSERT OR REPLACE INTO cache (device, key, version, latest) VALUES (?, ?, ?, ?)",
                [uid, key, version, latest],
            )
            .map(|_| ())
            .map_err(Error::unknown)
    }
    async fn del(&self, uid: &str, key: &str) -> Result<()> {
        info!("cache del: {} {}", uid, key);
        let conn = self.conn.lock().await;
        if !key.is_empty() {
            conn.execute("DELETE FROM cache WHERE device = ? AND key = ?", [uid, key])
                .map(|_| ())
        } else {
            conn.execute("DELETE FROM cache WHERE device = ?", [uid])
                .map(|_| ())
        }
        .map_err(Error::unknown)
    }
}
