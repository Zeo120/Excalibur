//! `excalibur-audit`: Real-time vulnerability rules, security header auditor, and secret hunter.

pub mod headers;
pub mod secrets;

pub use headers::{AuditSeverity, HeaderAuditFinding, HeaderAuditor};
pub use secrets::{SecretFinding, SecretScanner};

use excalibur_core::network::HttpResponse;
use serde::{Deserialize, Serialize};

/// Unified audit result aggregating header flaws and sensitive data disclosures.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FullAuditReport {
    pub header_findings: Vec<HeaderAuditFinding>,
    pub secret_findings: Vec<SecretFinding>,
}

impl FullAuditReport {
    pub fn is_clean(&self) -> bool {
        self.header_findings.is_empty() && self.secret_findings.is_empty()
    }

    pub fn total_issues(&self) -> usize {
        self.header_findings.len() + self.secret_findings.len()
    }
}

/// Orchestrates all security audit modules on HTTP traffic.
pub struct ResponseAuditor {
    secret_scanner: SecretScanner,
}

impl ResponseAuditor {
    pub fn new() -> Self {
        Self {
            secret_scanner: SecretScanner::new(),
        }
    }

    /// Performs an all-in-one security audit of an HTTP response.
    pub fn audit_response(&self, res: &HttpResponse, is_https: bool) -> FullAuditReport {
        let header_findings = HeaderAuditor::audit_response(res, is_https);
        let secret_findings = self.secret_scanner.scan_response(res);

        FullAuditReport {
            header_findings,
            secret_findings,
        }
    }
}

impl Default for ResponseAuditor {
    fn default() -> Self {
        Self::new()
    }
}
