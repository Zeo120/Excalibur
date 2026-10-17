use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditFinding {
    MissingHsts,
    MissingCsp,
    InsecureCors,
    MissingXContentTypeOptions,
    MissingXFrameOptions,
}

pub struct HeaderAuditor;

impl HeaderAuditor {
    pub fn audit_response_headers(headers: &HashMap<String, String>) -> Vec<AuditFinding> {
        let mut findings = Vec::new();

        if !headers.contains_key("strict-transport-security") {
            findings.push(AuditFinding::MissingHsts);
        }
        if !headers.contains_key("content-security-policy") {
            findings.push(AuditFinding::MissingCsp);
        }
        if let Some(cors) = headers.get("access-control-allow-origin") {
            if cors == "*" {
                findings.push(AuditFinding::InsecureCors);
            }
        }
        if !headers.contains_key("x-content-type-options") {
            findings.push(AuditFinding::MissingXContentTypeOptions);
        }
        if !headers.contains_key("x-frame-options") {
            findings.push(AuditFinding::MissingXFrameOptions);
        }

        findings
    }
}
