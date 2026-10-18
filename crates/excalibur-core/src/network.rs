use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Ordered HTTP headers preserving original key casing and duplicate entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HttpHeaders {
    entries: Vec<(String, String)>,
}

impl HttpHeaders {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn from_entries(entries: Vec<(String, String)>) -> Self {
        Self { entries }
    }

    /// Case-insensitive header lookup, returning the first matching value.
    pub fn get(&self, key: &str) -> Option<&str> {
        let key_lower = key.to_ascii_lowercase();
        self.entries
            .iter()
            .find(|(k, _)| k.to_ascii_lowercase() == key_lower)
            .map(|(_, v)| v.as_str())
    }

    /// Returns all values for a given header key (case-insensitive).
    pub fn get_all(&self, key: &str) -> Vec<&str> {
        let key_lower = key.to_ascii_lowercase();
        self.entries
            .iter()
            .filter(|(k, _)| k.to_ascii_lowercase() == key_lower)
            .map(|(_, v)| v.as_str())
            .collect()
    }

    /// Returns true if a header with the given key exists (case-insensitive).
    pub fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Appends a new header, preserving duplicate keys.
    pub fn append(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.entries.push((key.into(), value.into()));
    }

    /// Inserts or replaces all existing instances of the given header key.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let k_str = key.into();
        let v_str = value.into();
        let k_lower = k_str.to_ascii_lowercase();
        self.entries
            .retain(|(k, _)| k.to_ascii_lowercase() != k_lower);
        self.entries.push((k_str, v_str));
    }

    /// Removes all occurrences of a header key.
    pub fn remove(&mut self, key: &str) {
        let key_lower = key.to_ascii_lowercase();
        self.entries
            .retain(|(k, _)| k.to_ascii_lowercase() != key_lower);
    }

    /// Returns an iterator over all `(key, value)` header tuples.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Returns the number of headers.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the headers list is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Formats headers as raw HTTP wire representation (`Key: Value\r\n`).
    pub fn to_raw_string(&self) -> String {
        let mut out = String::new();
        for (k, v) in &self.entries {
            out.push_str(k);
            out.push_str(": ");
            out.push_str(v);
            out.push_str("\r\n");
        }
        out
    }

    /// Converts headers to a lowercase `HashMap<String, String>` for convenience.
    pub fn to_map(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for (k, v) in &self.entries {
            map.insert(k.to_ascii_lowercase(), v.clone());
        }
        map
    }
}

impl From<HashMap<String, String>> for HttpHeaders {
    fn from(map: HashMap<String, String>) -> Self {
        let mut entries = Vec::with_capacity(map.len());
        for (k, v) in map {
            entries.push((k, v));
        }
        Self { entries }
    }
}

impl From<Vec<(String, String)>> for HttpHeaders {
    fn from(entries: Vec<(String, String)>) -> Self {
        Self { entries }
    }
}

/// Structured HTTP Request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequest {
    pub id: String,
    pub method: String,
    pub url: String,
    pub version: String,
    pub headers: HttpHeaders,
    pub body: Vec<u8>,
    pub timestamp: i64,
}

impl HttpRequest {
    pub fn new(method: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            method: method.into(),
            url: url.into(),
            version: "HTTP/1.1".to_string(),
            headers: HttpHeaders::new(),
            body: Vec::new(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// Structured HTTP Response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpResponse {
    pub request_id: String,
    pub status: u16,
    pub reason: String,
    pub version: String,
    pub headers: HttpHeaders,
    pub body: Vec<u8>,
    pub elapsed_ms: u64,
    pub timestamp: i64,
}

impl HttpResponse {
    pub fn new(request_id: impl Into<String>, status: u16) -> Self {
        Self {
            request_id: request_id.into(),
            status,
            reason: String::new(),
            version: "HTTP/1.1".to_string(),
            headers: HttpHeaders::new(),
            body: Vec::new(),
            elapsed_ms: 0,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headers_case_insensitivity_and_preservation() {
        let mut headers = HttpHeaders::new();
        headers.append("Content-Type", "application/json");
        headers.append("X-Custom-Header", "Value1");
        headers.append("Set-Cookie", "a=1");
        headers.append("Set-Cookie", "b=2");

        // Case-insensitive lookup
        assert_eq!(headers.get("content-type"), Some("application/json"));
        assert_eq!(headers.get("CONTENT-TYPE"), Some("application/json"));
        assert_eq!(headers.get("x-custom-header"), Some("Value1"));

        // Duplicate header collection
        let cookies = headers.get_all("set-cookie");
        assert_eq!(cookies, vec!["a=1", "b=2"]);

        // Preservation of original casing in raw output
        let raw = headers.to_raw_string();
        assert!(raw.contains("Content-Type: application/json\r\n"));
        assert!(raw.contains("X-Custom-Header: Value1\r\n"));
    }
}
