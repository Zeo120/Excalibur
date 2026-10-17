use crate::error::Result;
use crate::network::{HttpRequest, HttpResponse};
use rusqlite::{params, Connection};
use std::path::Path;

pub struct WorkspaceStorage {
    conn: Connection,
}

impl WorkspaceStorage {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        let storage = Self { conn };
        storage.init_schema()?;
        Ok(storage)
    }

    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let storage = Self { conn };
        storage.init_schema()?;
        Ok(storage)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS requests (
                id TEXT PRIMARY KEY,
                method TEXT NOT NULL,
                url TEXT NOT NULL,
                headers TEXT NOT NULL,
                body BLOB,
                timestamp INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS responses (
                request_id TEXT PRIMARY KEY,
                status INTEGER NOT NULL,
                version TEXT NOT NULL,
                headers TEXT NOT NULL,
                body BLOB,
                elapsed_ms INTEGER NOT NULL,
                timestamp INTEGER NOT NULL,
                FOREIGN KEY (request_id) REFERENCES requests(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_requests_url ON requests(url);
            CREATE INDEX IF NOT EXISTS idx_requests_timestamp ON requests(timestamp);
            "#,
        )?;
        Ok(())
    }

    pub fn save_request(&self, req: &HttpRequest) -> Result<()> {
        let headers_json = serde_json::to_string(&req.headers)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO requests (id, method, url, headers, body, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![req.id, req.method, req.url, headers_json, req.body, req.timestamp],
        )?;
        Ok(())
    }

    pub fn save_response(&self, res: &HttpResponse) -> Result<()> {
        let headers_json = serde_json::to_string(&res.headers)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO responses (request_id, status, version, headers, body, elapsed_ms, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![res.request_id, res.status, res.version, headers_json, res.body, res.elapsed_ms, res.timestamp],
        )?;
        Ok(())
    }
}
