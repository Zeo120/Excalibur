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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserHistoryEntry {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub visit_time: i64,
    pub visit_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserBookmarkEntry {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub folder: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OmniboxMatchType {
    Bookmark,
    History,
    SearchSuggestion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmniboxMatch {
    pub match_type: OmniboxMatchType,
    pub title: String,
    pub url: String,
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

            CREATE TABLE IF NOT EXISTS browser_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT NOT NULL,
                title TEXT NOT NULL,
                visit_time INTEGER NOT NULL,
                visit_count INTEGER NOT NULL DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS browser_bookmarks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL,
                folder TEXT NOT NULL DEFAULT 'Favorites',
                created_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS browser_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_requests_url ON requests(url);
            CREATE INDEX IF NOT EXISTS idx_requests_timestamp ON requests(timestamp);
            CREATE INDEX IF NOT EXISTS idx_responses_status ON responses(status);
            CREATE INDEX IF NOT EXISTS idx_findings_severity ON audit_findings(severity);
            CREATE INDEX IF NOT EXISTS idx_browser_history_url ON browser_history(url);
            CREATE INDEX IF NOT EXISTS idx_browser_history_time ON browser_history(visit_time);
            CREATE INDEX IF NOT EXISTS idx_browser_bookmarks_url ON browser_bookmarks(url);
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

        // Seed initial browser bookmarks if table is fresh
        let bookmark_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM browser_bookmarks", [], |r| r.get(0))
            .unwrap_or(0);

        if bookmark_count == 0 {
            let now = chrono::Utc::now().timestamp();
            let default_bookmarks = [
                ("OWASP", "https://owasp.org", "Favorites"),
                (
                    "PortSwigger",
                    "https://portswigger.net/web-security",
                    "Favorites",
                ),
                ("HackerNews", "https://news.ycombinator.com", "Favorites"),
                ("GitHub", "https://github.com", "Favorites"),
                ("YouTube", "https://www.youtube.com", "Favorites"),
                ("Example", "https://example.com", "Favorites"),
                (
                    "Cloudflare",
                    "https://cloudflare.com/cdn-cgi/trace",
                    "Favorites",
                ),
                ("HTTPBin", "https://httpbin.org/html", "Favorites"),
            ];
            for (title, url, folder) in default_bookmarks {
                let _ = conn.execute(
                    "INSERT OR IGNORE INTO browser_bookmarks (url, title, folder, created_at) VALUES (?1, ?2, ?3, ?4)",
                    params![url, title, folder, now],
                );
            }
        }

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

    /// Records or updates a browser history entry.
    pub fn record_browser_visit(&self, url: &str, title: &str) -> Result<()> {
        if url.starts_with("about:") || url.starts_with("excalibur:") || url.trim().is_empty() {
            return Ok(());
        }
        let now = chrono::Utc::now().timestamp();
        let conn = self.conn.lock().unwrap();
        let existing: Option<(i64, String, i64)> = conn
            .query_row(
                "SELECT id, title, visit_count FROM browser_history WHERE url = ?1 LIMIT 1",
                params![url],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        if let Some((id, old_title, visit_count)) = existing {
            let new_title = if !title.trim().is_empty() && title != url {
                title.trim()
            } else {
                &old_title
            };
            conn.execute(
                "UPDATE browser_history SET title = ?1, visit_time = ?2, visit_count = ?3 WHERE id = ?4",
                params![new_title, now, visit_count + 1, id],
            )?;
        } else {
            let entry_title = if title.trim().is_empty() {
                url
            } else {
                title.trim()
            };
            conn.execute(
                "INSERT INTO browser_history (url, title, visit_time, visit_count) VALUES (?1, ?2, ?3, 1)",
                params![url, entry_title, now],
            )?;
        }
        Ok(())
    }

    /// Lists browser history with optional search query and pagination.
    pub fn list_browser_history(
        &self,
        limit: usize,
        offset: usize,
        filter: Option<&str>,
    ) -> Result<Vec<BrowserHistoryEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut entries = Vec::new();

        if let Some(f) = filter.filter(|s| !s.trim().is_empty()) {
            let pattern = format!("%{}%", f.trim());
            let mut stmt = conn.prepare(
                "SELECT id, url, title, visit_time, visit_count FROM browser_history WHERE url LIKE ?1 OR title LIKE ?1 ORDER BY visit_time DESC LIMIT ?2 OFFSET ?3",
            )?;
            let rows = stmt.query_map(params![pattern, limit as i64, offset as i64], |row| {
                Ok(BrowserHistoryEntry {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    title: row.get(2)?,
                    visit_time: row.get(3)?,
                    visit_count: row.get(4)?,
                })
            })?;
            for r in rows {
                entries.push(r?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, url, title, visit_time, visit_count FROM browser_history ORDER BY visit_time DESC LIMIT ?1 OFFSET ?2",
            )?;
            let rows = stmt.query_map(params![limit as i64, offset as i64], |row| {
                Ok(BrowserHistoryEntry {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    title: row.get(2)?,
                    visit_time: row.get(3)?,
                    visit_count: row.get(4)?,
                })
            })?;
            for r in rows {
                entries.push(r?);
            }
        }
        Ok(entries)
    }

    /// Clears all entries from browser history.
    pub fn clear_browser_history(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM browser_history", [])?;
        Ok(())
    }

    /// Deletes a specific history entry by ID.
    pub fn delete_browser_history_entry(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM browser_history WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Adds or updates a browser bookmark.
    pub fn add_browser_bookmark(&self, url: &str, title: &str, folder: &str) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let clean_folder = if folder.trim().is_empty() {
            "Favorites"
        } else {
            folder.trim()
        };
        let clean_title = if title.trim().is_empty() {
            url
        } else {
            title.trim()
        };
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO browser_bookmarks (url, title, folder, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![url, clean_title, clean_folder, now],
        )?;
        Ok(())
    }

    /// Removes a bookmark by exact URL.
    pub fn remove_browser_bookmark(&self, url: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM browser_bookmarks WHERE url = ?1", params![url])?;
        Ok(())
    }

    /// Checks if a URL is currently bookmarked.
    pub fn is_browser_bookmarked(&self, url: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let exists: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM browser_bookmarks WHERE url = ?1 LIMIT 1",
                params![url],
                |row| row.get(0),
            )
            .optional()?;
        Ok(exists.is_some())
    }

    /// Lists all saved browser bookmarks.
    pub fn list_browser_bookmarks(&self) -> Result<Vec<BrowserBookmarkEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, url, title, folder, created_at FROM browser_bookmarks ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(BrowserBookmarkEntry {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                folder: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Retrieves a browser setting by key.
    pub fn get_browser_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let val: Option<String> = conn
            .query_row(
                "SELECT value FROM browser_settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(val)
    }

    /// Stores a browser setting key-value pair.
    pub fn set_browser_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO browser_settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Searches omnibox suggestions matching a query against bookmarks and history.
    pub fn search_omnibox(&self, query: &str, limit: usize) -> Result<Vec<OmniboxMatch>> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }
        let pattern = format!("%{}%", trimmed);
        let conn = self.conn.lock().unwrap();
        let mut matches = Vec::new();
        let mut seen_urls = std::collections::HashSet::new();

        // 1. Matches from bookmarks
        {
            let mut stmt = conn.prepare(
                "SELECT title, url FROM browser_bookmarks WHERE url LIKE ?1 OR title LIKE ?1 LIMIT 4",
            )?;
            let rows = stmt.query_map(params![pattern], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            for r in rows.flatten() {
                if seen_urls.insert(r.1.clone()) {
                    matches.push(OmniboxMatch {
                        match_type: OmniboxMatchType::Bookmark,
                        title: r.0,
                        url: r.1,
                    });
                }
            }
        }

        // 2. Matches from history
        {
            let mut stmt = conn.prepare(
                "SELECT title, url FROM browser_history WHERE url LIKE ?1 OR title LIKE ?1 ORDER BY visit_count DESC, visit_time DESC LIMIT 4",
            )?;
            let rows = stmt.query_map(params![pattern], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            for r in rows.flatten() {
                if seen_urls.insert(r.1.clone()) {
                    matches.push(OmniboxMatch {
                        match_type: OmniboxMatchType::History,
                        title: r.0,
                        url: r.1,
                    });
                }
            }
        }

        // 3. Search Engine suggestion
        let search_url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            trimmed.replace(' ', "+")
        );
        matches.push(OmniboxMatch {
            match_type: OmniboxMatchType::SearchSuggestion,
            title: format!("Search web: {}", trimmed),
            url: search_url,
        });

        if matches.len() > limit {
            matches.truncate(limit);
        }
        Ok(matches)
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

    #[test]
    fn test_browser_history_and_bookmarks() {
        let storage = WorkspaceStorage::in_memory().expect("Failed to create in-memory storage");

        // Initial default bookmarks should be pre-seeded
        let bookmarks = storage
            .list_browser_bookmarks()
            .expect("List bookmarks failed");
        assert!(!bookmarks.is_empty());
        assert!(bookmarks.iter().any(|b| b.title == "OWASP"));
        assert!(storage.is_browser_bookmarked("https://owasp.org").unwrap());

        // Add custom bookmark
        storage
            .add_browser_bookmark("https://crates.io", "Rust Crates", "Favorites")
            .expect("Add bookmark failed");
        assert!(storage.is_browser_bookmarked("https://crates.io").unwrap());

        // Remove bookmark
        storage
            .remove_browser_bookmark("https://crates.io")
            .expect("Remove bookmark failed");
        assert!(!storage.is_browser_bookmarked("https://crates.io").unwrap());

        // Record history
        storage
            .record_browser_visit("https://docs.rs", "Docs.rs Documentation")
            .expect("Record visit failed");
        storage
            .record_browser_visit("https://docs.rs", "Docs.rs Documentation")
            .expect("Record visit 2 failed");

        let history = storage
            .list_browser_history(10, 0, None)
            .expect("List history failed");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].url, "https://docs.rs");
        assert_eq!(history[0].visit_count, 2);

        // Filter history
        let filtered = storage
            .list_browser_history(10, 0, Some("Docs"))
            .expect("Filter failed");
        assert_eq!(filtered.len(), 1);

        // Omnibox search
        let matches = storage
            .search_omnibox("docs", 5)
            .expect("Omnibox search failed");
        assert!(matches.iter().any(|m| m.url == "https://docs.rs"));
        assert!(matches
            .iter()
            .any(|m| m.match_type == OmniboxMatchType::SearchSuggestion));

        // Settings
        storage
            .set_browser_setting("search_engine", "DuckDuckGo")
            .expect("Set setting failed");
        let val = storage
            .get_browser_setting("search_engine")
            .expect("Get setting failed");
        assert_eq!(val, Some("DuckDuckGo".to_string()));

        // Delete history entry
        storage
            .delete_browser_history_entry(history[0].id)
            .expect("Delete history entry failed");
        let remaining = storage
            .list_browser_history(10, 0, None)
            .expect("List history failed");
        assert!(remaining.is_empty());
    }
}
