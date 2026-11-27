pub mod config;
pub mod tab;
pub mod url;

pub use config::BrowserConfig;
pub use tab::{TabInfo, TabManager};
pub use url::normalize_url_or_search;
