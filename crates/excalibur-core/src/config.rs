/// Runtime and memory-budget configuration for Excalibur.
#[derive(Debug, Clone)]
pub struct BrowserConfig {
    /// Initial URL loaded in the first tab when no URL argument is supplied.
    pub home_url: String,
    /// Search URL template containing `{query}` placeholder.
    pub search_template: String,
    /// Maximum Chromium renderer processes allowed (`--renderer-process-limit`).
    pub renderer_process_limit: u32,
    /// Idle duration in seconds before a background tab is eligible to sleep.
    pub tab_sleep_timeout_secs: u64,
    /// Whether the built-in ad and tracker domain blocker is enabled.
    pub ad_block_enabled: bool,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            home_url: "https://example.com".to_owned(),
            search_template: "https://duckduckgo.com/?q={query}".to_owned(),
            renderer_process_limit: 4,
            tab_sleep_timeout_secs: 600,
            ad_block_enabled: true,
        }
    }
}
