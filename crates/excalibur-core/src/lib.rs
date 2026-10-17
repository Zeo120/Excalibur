//! `excalibur-core`: Network client, TLS/JA3 engine, protocol parser, and SQLite persistence.

pub mod error;
pub mod network;
pub mod session;
pub mod storage;

pub use error::{CoreError, Result};
