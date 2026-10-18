use crate::error::Result;
use crate::network::{HttpHeaders, HttpRequest, HttpResponse};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditFindingRecord {
    pub id: String,
    pub request_id: String,
    pub category: String,
    pub severity: String,
    pub title: String,
    pub description: String,
    pub matched_data: Option<String>,
    pub timestamp: i64,
}

pub struct WorkspaceStorage {
    conn: Mutex<Connection>,
}

impl WorkspaceStorage {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage.init_database()?;
        Ok(storage)
    }

    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage.init_database()?;
        Ok(storage)
    }

    fn init_database(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        // High-performance Pragmas
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;
            PRAGMA cache_size = -64000;

            CREATE TABLE IF NOT EXISTS requests (
                id TEXT PRIMARY KEY,
                method TEXT NOT NULL,
                url TEXT NOT NULL,
                version TEXT NOT NULL,
                headers TEXT NOT NULL,
                body BLOB,
                timestamp INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS responses (
                request_id TEXT PRIMARY KEY,
                status INTEGER NOT NULL,
                reason TEXT NOT NULL,
                version TEXT NOT NULL,
                headers TEXT NOT NULL,
                body BLOB,
                elapsed_ms INTEGER NOT NULL,
                timestamp INTEGER NOT NULL,
                FOREIGN KEY (request_id) REFERENCES requests(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS bookmarks (
                id TEXT PRIMARY KEY,
                request_id TEXT NOT NULL,
                title TEXT NOT NULL,
                notes TEXT,
                color TEXT,
                timestamp INTEGER NOT NULL,
                FOREIGN KEY (request_id) REFERENCES requests(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS audit_findings (
                id TEXT PRIMARY KEY,
                request_id TEXT NOT NULL,
                category TEXT NOT NULL,
                severity TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT NOT NULL,
                matched_data TEXT,
                timestamp INTEGER NOT NULL,
                FOREIGN KEY (request_id) REFERENCES requests(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_requests_url ON requests(url);
            CREATE INDEX IF NOT EXISTS idx_requests_timestamp ON requests(timestamp);
            CREATE INDEX IF NOT EXISTS idx_responses_status ON responses(status);
            CREATE INDEX IF NOT EXISTS idx_findings_severity ON audit_findings(severity);
            "#,
        )?;

        // Full-Text Search FTS5 Table
        let _ = conn.execute_batch(
            r#"
            CREATE VIRTUAL TABLE IF NOT EXISTS requests_fts USING fts5(
                id UNINDEXED,
                url,
                headers,
                body_text
            );
            "#,
        );

        Ok(())
    }

    pub fn save_request(&self, req: &HttpRequest) -> Result<()> {
        let headers_json = serde_json::to_string(&req.headers)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO requests (id, method, url, version, headers, body, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![req.id, req.method, req.url, req.version, headers_json, req.body, req.timestamp],
        )?;

        // Update FTS5 index
        let body_text = String::from_utf8_lossy(&req.body).to_string();
        let _ = conn.execute(
            "INSERT OR REPLACE INTO requests_fts (id, url, headers, body_text) VALUES (?1, ?2, ?3, ?4)",
            params![req.id, req.url, headers_json, body_text],
        );

        Ok(())
    }

    pub fn save_response(&self, res: &HttpResponse) -> Result<()> {
        let headers_json = serde_json::to_string(&res.headers)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO responses (request_id, status, reason, version, headers, body, elapsed_ms, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![res.request_id, res.status, res.reason, res.version, headers_json, res.body, res.elapsed_ms, res.timestamp],
        )?;
        Ok(())
    }

    pub fn get_request(&self, id: &str) -> Result<Option<HttpRequest>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, method, url, version, headers, body, timestamp FROM requests WHERE id = ?1",
        )?;

        let req = stmt
            .query_row(params![id], |row| {
                let id: String = row.get(0)?;
                let method: String = row.get(1)?;
                let url: String = row.get(2)?;
                let version: String = row.get(3)?;
                let headers_raw: String = row.get(4)?;
                let body: Vec<u8> = row.get(5)?;
                let timestamp: i64 = row.get(6)?;

                let headers: HttpHeaders = serde_json::from_str(&headers_raw).unwrap_or_default();

                Ok(HttpRequest {
                    id,
                    method,
                    url,
                    version,
                    headers,
                    body,
                    timestamp,
                })
            })
            .optional()?;

        Ok(req)
    }

    pub fn get_response(&self, request_id: &str) -> Result<Option<HttpResponse>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT request_id, status, reason, version, headers, body, elapsed_ms, timestamp FROM responses WHERE request_id = ?1",
        )?;

        let res = stmt
            .query_row(params![request_id], |row| {
                let request_id: String = row.get(0)?;
                let status: u16 = row.get(1)?;
                let reason: String = row.get(2)?;
                let version: String = row.get(3)?;
                let headers_raw: String = row.get(4)?;
                let body: Vec<u8> = row.get(5)?;
                let elapsed_ms: u64 = row.get(6)?;
                let timestamp: i64 = row.get(7)?;

                let headers: HttpHeaders = serde_json::from_str(&headers_raw).unwrap_or_default();

                Ok(HttpResponse {
                    request_id,
                    status,
                    reason,
                    version,
                    headers,
                    body,
                    elapsed_ms,
                    timestamp,
                })
            })
            .optional()?;

        Ok(res)
    }

    pub fn list_requests(&self, limit: usize, offset: usize) -> Result<Vec<HttpRequest>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, method, url, version, headers, body, timestamp FROM requests ORDER BY timestamp DESC LIMIT ?1 OFFSET ?2",
        )?;

        let rows = stmt.query_map(params![limit as i64, offset as i64], |row| {
            let id: String = row.get(0)?;
            let method: String = row.get(1)?;
            let url: String = row.get(2)?;
            let version: String = row.get(3)?;
            let headers_raw: String = row.get(4)?;
            let body: Vec<u8> = row.get(5)?;
            let timestamp: i64 = row.get(6)?;

            let headers: HttpHeaders = serde_json::from_str(&headers_raw).unwrap_or_default();

            Ok(HttpRequest {
                id,
                method,
                url,
                version,
                headers,
                body,
                timestamp,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Full-text search across URLs, headers, and request bodies.
    pub fn search_requests(&self, query: &str) -> Result<Vec<String>> {
        let escaped = format!("\"{}\"", query.replace('"', "\"\""));
        let conn = self.conn.lock().unwrap();
        let stmt = conn.prepare(
            "SELECT id FROM requests_fts WHERE requests_fts MATCH ?1 ORDER BY rank LIMIT 50",
        );

        if let Ok(mut s) = stmt {
            if let Ok(rows) = s.query_map(params![escaped], |row| row.get::<_, String>(0)) {
                let mut ids = Vec::new();
                for id in rows.flatten() {
                    ids.push(id);
                }
                if !ids.is_empty() {
                    return Ok(ids);
                }
            }
        }

        // Fallback to LIKE
        let pattern = format!("%{}%", query);
        let mut s = conn.prepare(
            "SELECT id FROM requests WHERE url LIKE ?1 OR headers LIKE ?1 ORDER BY timestamp DESC LIMIT 50",
        )?;
        let rows = s.query_map(params![pattern], |row| row.get::<_, String>(0))?;
        let mut ids = Vec::new();
        for r in rows {
            ids.push(r?);
        }
        Ok(ids)
    }

    pub fn save_finding(&self, finding: &AuditFindingRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO audit_findings (id, request_id, category, severity, title, description, matched_data, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                finding.id,
                finding.request_id,
                finding.category,
                finding.severity,
                finding.title,
                finding.description,
                finding.matched_data,
                finding.timestamp
            ],
        )?;
        Ok(())
    }

    pub fn list_findings(&self) -> Result<Vec<AuditFindingRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, request_id, category, severity, title, description, matched_data, timestamp FROM audit_findings ORDER BY timestamp DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(AuditFindingRecord {
                id: row.get(0)?,
                request_id: row.get(1)?,
                category: row.get(2)?,
                severity: row.get(3)?,
                title: row.get(4)?,
                description: row.get(5)?,
                matched_data: row.get(6)?,
                timestamp: row.get(7)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_crud_and_fts() {
        let storage = WorkspaceStorage::in_memory().expect("Failed to create in-memory storage");

        let mut req = HttpRequest::new("GET", "https://api.example.com/v1/users");
        req.headers
            .append("Authorization", "Bearer secret-token-12345");
        req.body = b"{\"username\":\"admin\"}".to_vec();

        storage.save_request(&req).expect("Failed to save request");

        let fetched = storage
            .get_request(&req.id)
            .expect("Failed to get request")
            .unwrap();
        assert_eq!(fetched.url, "https://api.example.com/v1/users");
        assert_eq!(
            fetched.headers.get("authorization"),
            Some("Bearer secret-token-12345")
        );

        let mut res = HttpResponse::new(&req.id, 200);
        res.headers.append("Content-Type", "application/json");
        res.body = b"{\"status\":\"ok\"}".to_vec();
        storage
            .save_response(&res)
            .expect("Failed to save response");

        let fetched_res = storage
            .get_response(&req.id)
            .expect("Failed to get response")
            .unwrap();
        assert_eq!(fetched_res.status, 200);
        assert_eq!(
            fetched_res.headers.get("content-type"),
            Some("application/json")
        );

        // Search test
        let results = storage
            .search_requests("secret-token")
            .expect("Search failed");
        assert!(!results.is_empty());
        assert_eq!(results[0], req.id);
    }
}
