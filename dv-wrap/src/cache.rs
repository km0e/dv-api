use std::path::Path;

use rusqlite::Result;
use tokio::sync::Mutex;
use tracing::info;

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
                path TEXT NOT NULL,
                version INTEGER NOT NULL,
                lastest INTEGER NOT NULL,
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
                path TEXT NOT NULL,
                version INTEGER NOT NULL,
                lastest INTEGER NOT NULL,
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

impl SqliteCache {
    pub async fn get(&self, uid: &str, path: &str) -> Result<Option<(i64, i64)>> {
        let row = self.conn.lock().await.query_row(
            "SELECT version, lastest FROM cache WHERE device = ? AND path = ?",
            [uid, path],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );
        match row {
            Ok(fs) => Ok(Some(fs)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
    pub async fn set(&self, uid: &str, path: &str, version: i64, latest: i64) -> Result<()> {
        info!("cache set: {} {} {} {}", uid, path, version, latest);
        self.conn
            .lock()
            .await
            .execute(
                "INSERT OR REPLACE INTO cache (device, path, version, lastest) VALUES (?, ?, ?, ?)",
                [uid, path, &version.to_string(), &latest.to_string()],
            )
            .map(|_| ())
    }
    pub async fn del(&self, uid: &str, path: &str) -> Result<()> {
        info!("cache del: {} {}", uid, path);
        let conn = self.conn.lock().await;
        if !path.is_empty() {
            conn.execute(
                "DELETE FROM cache WHERE device = ? AND path = ?",
                [uid, path],
            )
            .map(|_| ())
        } else {
            conn.execute("DELETE FROM cache WHERE device = ?", [uid])
                .map(|_| ())
        }
    }
}
