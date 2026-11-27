use std::{
    collections::HashSet,
    sync::atomic::{AtomicBool, Ordering},
};

const DEFAULT_BLOCKED_DOMAINS: &[&str] = &[
    "doubleclick.net",
    "googlesyndication.com",
    "googleadservices.com",
    "google-analytics.com",
    "googletagmanager.com",
    "googletagservices.com",
    "adservice.google.com",
    "pagead2.googlesyndication.com",
    "connect.facebook.net",
    "pixel.facebook.com",
    "an.facebook.com",
    "ads.twitter.com",
    "analytics.twitter.com",
    "static.ads-twitter.com",
    "ads.linkedin.com",
    "snap.licdn.com",
    "bat.bing.com",
    "adnxs.com",
    "rubiconproject.com",
    "pubmatic.com",
    "openx.net",
    "casalemedia.com",
    "criteo.com",
    "criteo.net",
    "taboola.com",
    "outbrain.com",
    "scorecardresearch.com",
    "quantserve.com",
    "moatads.com",
    "adsrvr.org",
    "mathtag.com",
    "adroll.com",
    "amazon-adsystem.com",
    "mediavine.com",
    "adthrive.com",
    "advertising.com",
    "ads.yahoo.com",
    "gemini.yahoo.com",
    "analytics.tiktok.com",
    "ads.tiktok.com",
    "hotjar.com",
    "mixpanel.com",
    "segment.io",
    "segment.com",
    "amplitude.com",
    "sentry.io",
    "bugsnag.com",
    "nr-data.net",
    "newrelic.com",
    "chartbeat.com",
    "omtrdc.net",
    "demdex.net",
    "krxd.net",
    "bluekai.com",
    "app-measurement.com",
    "branch.io",
    "adjust.com",
    "appsflyer.com",
    "telemetry.mozilla.org",
    "clarity.ms",
    "fullstory.com",
    "mouseflow.com",
];

/// Fast host/subdomain matcher for blocking ad and tracker subresources before network allocation.
pub struct DomainBlocker {
    enabled: AtomicBool,
    domains: HashSet<&'static str>,
}

impl DomainBlocker {
    /// Creates a blocker seeded with the built-in domain list.
    pub fn new(enabled: bool) -> Self {
        let domains = DEFAULT_BLOCKED_DOMAINS.iter().copied().collect();
        Self {
            enabled: AtomicBool::new(enabled),
            domains,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn rule_count(&self) -> usize {
        self.domains.len()
    }

    /// Returns the matched host if `url` should be blocked, or `None` if allowed.
    pub fn should_block_url(&self, url: &str) -> Option<String> {
        if !self.is_enabled() {
            return None;
        }
        let host = extract_host(url)?;
        if self.matches_host(&host) {
            Some(host)
        } else {
            None
        }
    }

    fn matches_host(&self, host: &str) -> bool {
        if self.domains.contains(host) {
            return true;
        }
        let mut rest = host;
        while let Some((_, tail)) = rest.split_once('.') {
            if self.domains.contains(tail) {
                return true;
            }
            rest = tail;
        }
        false
    }
}

fn extract_host(url: &str) -> Option<String> {
    let (_, after_scheme) = url.split_once("://")?;
    let authority = after_scheme.split(['/', '?', '#']).next()?;
    let host_port = authority.rsplit('@').next().unwrap_or(authority);
    let host = if host_port.starts_with('[') {
        host_port
            .trim_start_matches('[')
            .split(']')
            .next()
            .unwrap_or("")
    } else {
        host_port.split(':').next().unwrap_or("")
    };
    let normalized = host.trim_end_matches('.').to_ascii_lowercase();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_exact_and_subdomains_without_suffix_false_positives() {
        let blocker = DomainBlocker::new(true);
        assert_eq!(
            blocker.should_block_url("https://google-analytics.com/collect"),
            Some("google-analytics.com".to_owned())
        );
        assert_eq!(
            blocker.should_block_url("https://pixel.ads.doubleclick.net:443/img.gif"),
            Some("pixel.ads.doubleclick.net".to_owned())
        );
        assert_eq!(
            blocker.should_block_url("https://notdoubleclick.net/app.js"),
            None
        );
        assert_eq!(blocker.should_block_url("https://example.com/"), None);
    }

    #[test]
    fn toggle_disables_and_reenables_blocking() {
        let blocker = DomainBlocker::new(false);
        assert_eq!(
            blocker.should_block_url("https://www.googletagmanager.com/gtm.js"),
            None
        );
        blocker.set_enabled(true);
        assert_eq!(
            blocker.should_block_url("https://www.googletagmanager.com/gtm.js"),
            Some("www.googletagmanager.com".to_owned())
        );
    }
}
