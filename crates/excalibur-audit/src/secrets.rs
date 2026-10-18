use crate::headers::AuditSeverity;
use excalibur_core::network::HttpResponse;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Finding produced when a high-entropy secret, API key, or credential is identified.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SecretFinding {
    pub rule_id: String,
    pub rule_name: String,
    pub severity: AuditSeverity,
    pub matched_text: String,
    pub redacted_text: String,
    pub offset: usize,
    pub entropy: f64,
}

struct SecretRule {
    id: &'static str,
    name: &'static str,
    severity: AuditSeverity,
    pattern: Regex,
    min_entropy: f64,
}

/// Real-time scanner for credentials, tokens, and high-entropy secrets in HTTP traffic.
pub struct SecretScanner {
    rules: Vec<SecretRule>,
}

impl SecretScanner {
    pub fn new() -> Self {
        let rules = vec![
            SecretRule {
                id: "SEC-AWS-KEY-ID",
                name: "AWS Access Key ID",
                severity: AuditSeverity::High,
                pattern: Regex::new(r#"(AKIA[0-9A-Z]{16})"#).unwrap(),
                min_entropy: 2.5,
            },
            SecretRule {
                id: "SEC-AWS-SECRET-KEY",
                name: "AWS Secret Access Key",
                severity: AuditSeverity::Critical,
                pattern: Regex::new(r#"(?i)(?:aws_secret_access_key|aws_secret|aws_key)\s*[:=]\s*["']?([A-Za-z0-9/+=]{40})["']?"#).unwrap(),
                min_entropy: 3.5,
            },
            SecretRule {
                id: "SEC-GITHUB-TOKEN",
                name: "GitHub Personal Access Token",
                severity: AuditSeverity::Critical,
                pattern: Regex::new(r#"(ghp_[a-zA-Z0-9]{36}|github_pat_[a-zA-Z0-9]{22}_[a-zA-Z0-9]{59})"#).unwrap(),
                min_entropy: 3.0,
            },
            SecretRule {
                id: "SEC-GOOGLE-API-KEY",
                name: "Google API Key",
                severity: AuditSeverity::High,
                pattern: Regex::new(r#"(AIza[0-9A-Za-z\-_]{35})"#).unwrap(),
                min_entropy: 3.0,
            },
            SecretRule {
                id: "SEC-SLACK-TOKEN",
                name: "Slack API / Webhook Token",
                severity: AuditSeverity::High,
                pattern: Regex::new(r#"(xox[baprs]-[0-9a-zA-Z]{10,48})"#).unwrap(),
                min_entropy: 3.0,
            },
            SecretRule {
                id: "SEC-STRIPE-KEY",
                name: "Stripe Secret Key",
                severity: AuditSeverity::Critical,
                pattern: Regex::new(r#"(sk_live_[0-9a-zA-Z]{24,})"#).unwrap(),
                min_entropy: 3.0,
            },
            SecretRule {
                id: "SEC-JWT-TOKEN",
                name: "JSON Web Token (JWT)",
                severity: AuditSeverity::Medium,
                pattern: Regex::new(r#"(eyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,})"#).unwrap(),
                min_entropy: 3.5,
            },
            SecretRule {
                id: "SEC-PRIVATE-KEY",
                name: "Private Cryptographic Key",
                severity: AuditSeverity::Critical,
                pattern: Regex::new(r#"(-----BEGIN (?:RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----)"#).unwrap(),
                min_entropy: 0.0,
            },
            SecretRule {
                id: "SEC-GENERIC-API-KEY",
                name: "Generic High-Entropy API Key / Secret",
                severity: AuditSeverity::Medium,
                pattern: Regex::new(r#"(?i)(?:api_key|apikey|client_secret|auth_token)\s*[:=]\s*["']([a-zA-Z0-9_\-]{20,80})["']"#).unwrap(),
                min_entropy: 3.2,
            },
            SecretRule {
                id: "SEC-BEARER-TOKEN",
                name: "Bearer Authentication Token",
                severity: AuditSeverity::Medium,
                pattern: Regex::new(r#"(?i)bearer\s+([a-zA-Z0-9_\-\.]{25,})"#).unwrap(),
                min_entropy: 3.0,
            },
        ];

        Self { rules }
    }

    /// Calculates Shannon entropy of a string (bits per character).
    pub fn shannon_entropy(s: &str) -> f64 {
        if s.is_empty() {
            return 0.0;
        }

        let mut freq = HashMap::new();
        for ch in s.chars() {
            *freq.entry(ch).or_insert(0) += 1;
        }

        let len = s.len() as f64;
        let mut entropy = 0.0;
        for &count in freq.values() {
            let p = (count as f64) / len;
            entropy -= p * p.log2();
        }
        entropy
    }

    /// Redacts all but the first 4 and last 4 characters of a secret.
    pub fn redact_secret(s: &str) -> String {
        if s.len() <= 8 {
            return "*".repeat(s.len());
        }
        format!("{}...{}", &s[..4], &s[s.len().saturating_sub(4)..])
    }

    /// Scans text (source code, JSON, headers, JS bundles) for sensitive tokens.
    pub fn scan_text(&self, text: &str) -> Vec<SecretFinding> {
        let mut findings = Vec::new();

        for rule in &self.rules {
            for cap in rule.pattern.captures_iter(text) {
                if let Some(m) = cap.get(1) {
                    let matched_str = m.as_str();
                    let entropy = Self::shannon_entropy(matched_str);

                    if entropy >= rule.min_entropy {
                        findings.push(SecretFinding {
                            rule_id: rule.id.to_string(),
                            rule_name: rule.name.to_string(),
                            severity: rule.severity,
                            matched_text: matched_str.to_string(),
                            redacted_text: Self::redact_secret(matched_str),
                            offset: m.start(),
                            entropy,
                        });
                    }
                }
            }
        }

        findings
    }

    /// Scans an HTTP response (headers and payload body).
    pub fn scan_response(&self, res: &HttpResponse) -> Vec<SecretFinding> {
        let mut findings = Vec::new();

        // Scan headers
        for (k, v) in res.headers.iter() {
            let header_text = format!("{}: {}", k, v);
            findings.extend(self.scan_text(&header_text));
        }

        // Scan body if UTF-8
        if let Ok(body_str) = std::str::from_utf8(&res.body) {
            findings.extend(self.scan_text(body_str));
        }

        findings
    }
}

impl Default for SecretScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aws_key_and_github_pat_detection() {
        let scanner = SecretScanner::new();
        let payload = r#"
            const aws_config = {
                accessKeyId: "AKIAIOSFODNN7EXAMPLE",
                region: "us-west-2"
            };
            const github_token = "ghp_1234567890abcdefghijklmnopqrstuvwxyz";
        "#;

        let findings = scanner.scan_text(payload);
        assert_eq!(findings.len(), 2);

        let aws_finding = findings
            .iter()
            .find(|f| f.rule_id == "SEC-AWS-KEY-ID")
            .unwrap();
        assert_eq!(aws_finding.matched_text, "AKIAIOSFODNN7EXAMPLE");
        assert_eq!(aws_finding.redacted_text, "AKIA...MPLE");

        let gh_finding = findings
            .iter()
            .find(|f| f.rule_id == "SEC-GITHUB-TOKEN")
            .unwrap();
        assert_eq!(
            gh_finding.matched_text,
            "ghp_1234567890abcdefghijklmnopqrstuvwxyz"
        );
        assert_eq!(gh_finding.severity, AuditSeverity::Critical);
    }

    #[test]
    fn test_private_key_and_jwt_detection() {
        let scanner = SecretScanner::new();
        let payload = r#"
            -----BEGIN RSA PRIVATE KEY-----
            MIIEowIBAAKCAQEA0Y...
            -----END RSA PRIVATE KEY-----
            Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c
        "#;

        let findings = scanner.scan_text(payload);
        let rule_ids: Vec<_> = findings.iter().map(|f| f.rule_id.as_str()).collect();

        assert!(rule_ids.contains(&"SEC-PRIVATE-KEY"));
        assert!(rule_ids.contains(&"SEC-JWT-TOKEN"));
    }

    #[test]
    fn test_shannon_entropy() {
        let low_entropy = SecretScanner::shannon_entropy("aaaaaaaaaaaa");
        assert_eq!(low_entropy, 0.0);

        let high_entropy = SecretScanner::shannon_entropy("4b8d7f2a1e9c5b0d8f3a7e2c");
        assert!(high_entropy > 3.0);
    }
}
