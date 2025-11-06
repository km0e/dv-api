use super::dev::*;
use std::path::Path;
use tokio::sync::Mutex;
use tracing::{debug, info};

#[derive(Debug)]
pub struct Sqlite {
    conn: Mutex<rusqlite::Connection>,
}

impl Sqlite {
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref();
        info!("use sqlite path {}", db_path.display());
        let conn = match rusqlite::Connection::open(db_path) {
            Ok(c) => c,
            Err(e) => {
                if let Some(rusqlite::ErrorCode::CannotOpen) = e.sqlite_error_code() {
                    let parent = db_path
                        .parent()
                        .ok_or_else(|| anyhow::anyhow!("sqlite db path should have parent"))?;
                    std::fs::create_dir_all(parent)?;
                }
                rusqlite::Connection::open(db_path)?
            }
        };
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cache (
                device TEXT NOT NULL,
                key TEXT NOT NULL,
                version TEXT NOT NULL,
                latest TEXT NOT NULL,
                PRIMARY KEY (device, key)
            )",
            [],
        )?;
        info!("sqlite db initialized");
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
    #[cfg(test)]
    pub fn memory() -> Result<Self> {
        let conn = rusqlite::Connection::open_in_memory()?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cache (
                device TEXT NOT NULL,
                key TEXT NOT NULL,
                version TEXT NOT NULL,
                latest TEXT NOT NULL,
                PRIMARY KEY (device, key)
            )",
            [],
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

#[async_trait::async_trait]
impl crate::db::DB for Sqlite {
    async fn get(&self, uid: &str, key: &str) -> Result<Option<(String, String)>> {
        let row = self.conn.lock().await.query_row(
            "SELECT version, latest FROM cache WHERE device = ? AND key = ?",
            [uid, key],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        );
        match row {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => anyhow::bail!(e),
        }
    }
    async fn set(&self, uid: &str, key: &str, version: &str, latest: &str) -> Result<()> {
        debug!("cache set: {} {} {} {}", uid, key, version, latest);
        Ok(self
            .conn
            .lock()
            .await
            .execute(
                "INSERT OR REPLACE INTO cache (device, key, version, latest) VALUES (?, ?, ?, ?)",
                [uid, key, version, latest],
            )
            .map(|_| ())?)
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
        }?;
        Ok(())
    }
}
