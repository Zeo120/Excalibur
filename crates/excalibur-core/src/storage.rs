use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection};

use crate::BrowserConfig;

/// Saved bookmark record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookmarkEntry {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub created_at: i64,
}

/// Recorded history visit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub visited_at: i64,
}

/// Lightweight SQLite WAL store for bookmarks, browsing history, and settings.
pub struct Storage {
    conn: Connection,
}

impl Storage {
    /// Opens or creates the SQLite database at the default user data directory.
    pub fn open_default() -> Result<Self, String> {
        let path = default_db_path();
        Self::open(&path)
    }

    /// Opens or creates a SQLite database at `path` with WAL mode enabled.
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create storage directory: {e}"))?;
        }
        let conn =
            Connection::open(path).map_err(|e| format!("Failed to open SQLite database: {e}"))?;
        let storage = Self { conn };
        storage.init_schema(true)?;
        Ok(storage)
    }

    /// Opens an ephemeral in-memory SQLite database (used in tests).
    pub fn open_in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory()
            .map_err(|e| format!("Failed to open in-memory SQLite: {e}"))?;
        let storage = Self { conn };
        storage.init_schema(false)?;
        Ok(storage)
    }

    fn init_schema(&self, enable_wal: bool) -> Result<(), String> {
        if enable_wal {
            self.conn
                .execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")
                .map_err(|e| format!("Failed to enable WAL mode: {e}"))?;
        }
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS bookmarks (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    url TEXT NOT NULL UNIQUE,
                    title TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS history (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    url TEXT NOT NULL,
                    title TEXT NOT NULL,
                    visited_at INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_history_visited_at ON history(visited_at DESC);
                CREATE TABLE IF NOT EXISTS settings (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );",
            )
            .map_err(|e| format!("Failed to initialize SQLite schema: {e}"))?;
        Ok(())
    }

    /// Loads persisted settings overlaid on `defaults`.
    pub fn load_settings(&self, defaults: &BrowserConfig) -> Result<BrowserConfig, String> {
        let mut cfg = defaults.clone();
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM settings")
            .map_err(|e| format!("Failed to query settings: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| format!("Failed to map settings: {e}"))?;

        for item in rows {
            let (key, value) = item.map_err(|e| format!("Failed to read setting row: {e}"))?;
            match key.as_str() {
                "home_url" if !value.trim().is_empty() => cfg.home_url = value,
                "search_template" if value.contains("{query}") => cfg.search_template = value,
                "renderer_process_limit" => {
                    if let Ok(limit) = value.parse::<u32>() {
                        cfg.renderer_process_limit = limit.clamp(1, 32);
                    }
                }
                "tab_sleep_timeout_secs" => {
                    if let Ok(secs) = value.parse::<u64>() {
                        cfg.tab_sleep_timeout_secs = secs.max(30);
                    }
                }
                _ => {}
            }
        }
        Ok(cfg)
    }

    /// Persists `config` fields into the settings table.
    pub fn save_settings(&self, config: &BrowserConfig) -> Result<(), String> {
        let pairs = [
            ("home_url", config.home_url.as_str()),
            ("search_template", config.search_template.as_str()),
            (
                "renderer_process_limit",
                &config.renderer_process_limit.to_string(),
            ),
            (
                "tab_sleep_timeout_secs",
                &config.tab_sleep_timeout_secs.to_string(),
            ),
        ];
        for (key, value) in pairs {
            self.conn
                .execute(
                    "INSERT INTO settings (key, value) VALUES (?1, ?2)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    params![key, value],
                )
                .map_err(|e| format!("Failed to save setting {key}: {e}"))?;
        }
        Ok(())
    }

    /// Adds or updates a bookmark for `url`.
    pub fn add_bookmark(&self, url: &str, title: &str) -> Result<(), String> {
        let clean_title = if title.trim().is_empty() { url } else { title };
        self.conn
            .execute(
                "INSERT INTO bookmarks (url, title, created_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(url) DO UPDATE SET title = excluded.title",
                params![url, clean_title, unix_now()],
            )
            .map_err(|e| format!("Failed to save bookmark: {e}"))?;
        Ok(())
    }

    /// Removes a bookmark matching `url`. Returns `true` if a row was deleted.
    pub fn remove_bookmark_by_url(&self, url: &str) -> Result<bool, String> {
        let count = self
            .conn
            .execute("DELETE FROM bookmarks WHERE url = ?1", params![url])
            .map_err(|e| format!("Failed to delete bookmark: {e}"))?;
        Ok(count > 0)
    }

    /// Checks whether `url` is currently bookmarked.
    pub fn is_bookmarked(&self, url: &str) -> Result<bool, String> {
        let exists: i64 = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM bookmarks WHERE url = ?1)",
                params![url],
                |row| row.get(0),
            )
            .map_err(|e| format!("Failed to check bookmark: {e}"))?;
        Ok(exists != 0)
    }

    /// Lists all bookmarks ordered by creation descending.
    pub fn list_bookmarks(&self) -> Result<Vec<BookmarkEntry>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, url, title, created_at FROM bookmarks ORDER BY id DESC")
            .map_err(|e| format!("Failed to prepare bookmark list: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(BookmarkEntry {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    title: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })
            .map_err(|e| format!("Failed to read bookmarks: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to collect bookmarks: {e}"))
    }

    /// Records a page visit in history (skips `about:blank`).
    pub fn record_history(&self, url: &str, title: &str) -> Result<(), String> {
        let trimmed = url.trim();
        if trimmed.is_empty() || trimmed == "about:blank" {
            return Ok(());
        }
        let clean_title = if title.trim().is_empty() {
            trimmed
        } else {
            title
        };
        self.conn
            .execute(
                "INSERT INTO history (url, title, visited_at) VALUES (?1, ?2, ?3)",
                params![trimmed, clean_title, unix_now()],
            )
            .map_err(|e| format!("Failed to record history: {e}"))?;
        Ok(())
    }

    /// Returns up to `limit` history entries matching `query` (or most recent when empty).
    pub fn search_history(&self, query: &str, limit: usize) -> Result<Vec<HistoryEntry>, String> {
        let limit_i64 = i64::try_from(limit).unwrap_or(100);
        let q = query.trim();
        let (sql, pattern) = if q.is_empty() {
            (
                "SELECT id, url, title, visited_at FROM history ORDER BY visited_at DESC, id DESC LIMIT ?1",
                None,
            )
        } else {
            (
                "SELECT id, url, title, visited_at FROM history
                 WHERE url LIKE ?1 OR title LIKE ?1
                 ORDER BY visited_at DESC, id DESC LIMIT ?2",
                Some(format!("%{q}%")),
            )
        };

        let mut stmt = self
            .conn
            .prepare(sql)
            .map_err(|e| format!("Failed to query history: {e}"))?;

        let map_row = |row: &rusqlite::Row<'_>| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                visited_at: row.get(3)?,
            })
        };

        let entries = if let Some(pat) = pattern {
            stmt.query_map(params![pat, limit_i64], map_row)
                .map_err(|e| format!("Failed to search history: {e}"))?
                .collect::<Result<Vec<_>, _>>()
        } else {
            stmt.query_map(params![limit_i64], map_row)
                .map_err(|e| format!("Failed to list history: {e}"))?
                .collect::<Result<Vec<_>, _>>()
        };

        entries.map_err(|e| format!("Failed to collect history: {e}"))
    }

    /// Clears all browsing history.
    pub fn clear_history(&self) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM history", [])
            .map_err(|e| format!("Failed to clear history: {e}"))?;
        Ok(())
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn default_db_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(local_app_data)
            .join("Excalibur")
            .join("excalibur.db");
    }

    #[cfg(target_os = "macos")]
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("Excalibur")
            .join("excalibur.db");
    }

    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(xdg).join("excalibur").join("excalibur.db");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("excalibur")
            .join("excalibur.db");
    }

    std::env::temp_dir().join("excalibur.db")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_clamping() {
        let store = Storage::open_in_memory().expect("in-memory db");
        let cfg = BrowserConfig {
            home_url: "https://rust-lang.org".to_owned(),
            search_template: "https://kagi.com/search?q={query}".to_owned(),
            renderer_process_limit: 6,
            tab_sleep_timeout_secs: 300,
        };
        store.save_settings(&cfg).expect("save settings");

        let loaded = store
            .load_settings(&BrowserConfig::default())
            .expect("load settings");
        assert_eq!(loaded.home_url, "https://rust-lang.org");
        assert_eq!(loaded.search_template, "https://kagi.com/search?q={query}");
        assert_eq!(loaded.renderer_process_limit, 6);
        assert_eq!(loaded.tab_sleep_timeout_secs, 300);
    }

    #[test]
    fn bookmarks_crud_works() {
        let store = Storage::open_in_memory().expect("in-memory db");
        assert!(!store.is_bookmarked("https://example.com").unwrap());

        store
            .add_bookmark("https://example.com", "Example")
            .expect("add bookmark");
        assert!(store.is_bookmarked("https://example.com").unwrap());
        assert_eq!(store.list_bookmarks().unwrap().len(), 1);

        assert!(store.remove_bookmark_by_url("https://example.com").unwrap());
        assert!(!store.is_bookmarked("https://example.com").unwrap());
    }

    #[test]
    fn history_record_search_and_clear() {
        let store = Storage::open_in_memory().expect("in-memory db");
        store.record_history("about:blank", "Blank").unwrap();
        store
            .record_history("https://example.com", "Example Domain")
            .unwrap();
        store
            .record_history("https://doc.rust-lang.org", "Rust Docs")
            .unwrap();

        assert_eq!(store.search_history("", 50).unwrap().len(), 2);
        let hits = store.search_history("rust", 50).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].url, "https://doc.rust-lang.org");

        store.clear_history().unwrap();
        assert!(store.search_history("", 50).unwrap().is_empty());
    }
}
