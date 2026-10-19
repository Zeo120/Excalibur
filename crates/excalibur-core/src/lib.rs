//! `excalibur-core`: Network client, TLS/JA3 engine, protocol parser, and SQLite persistence.

pub mod client;
pub mod content;
pub mod dns;
pub mod error;
pub mod network;
pub mod session;
pub mod storage;
pub mod tls;

pub use client::{ClientOptions, HttpClient};
pub use dns::{connect_tcp_resilient, resolve_host};
pub use error::{CoreError, Result};
pub use network::{HttpHeaders, HttpRequest, HttpResponse};
pub use session::SessionConfig;
pub use storage::{
    AuditFindingRecord, BrowserBookmarkEntry, BrowserHistoryEntry, OmniboxMatch, OmniboxMatchType,
    WorkspaceStorage,
};
pub use tls::{ensure_crypto_provider, Ja3Preset, PermissiveCertVerifier, TlsConfigBuilder};
