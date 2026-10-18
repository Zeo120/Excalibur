//! `excalibur-core`: Network client, TLS/JA3 engine, protocol parser, and SQLite persistence.

pub mod client;
pub mod error;
pub mod network;
pub mod session;
pub mod storage;
pub mod tls;

pub use client::{ClientOptions, HttpClient};
pub use error::{CoreError, Result};
pub use network::{HttpHeaders, HttpRequest, HttpResponse};
pub use session::SessionConfig;
pub use storage::{AuditFindingRecord, WorkspaceStorage};
pub use tls::{ensure_crypto_provider, Ja3Preset, PermissiveCertVerifier, TlsConfigBuilder};
