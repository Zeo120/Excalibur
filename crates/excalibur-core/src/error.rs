use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Network I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TLS configuration error: {0}")]
    Tls(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("Request timed out after {0:?}")]
    Timeout(std::time::Duration),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CoreError>;
