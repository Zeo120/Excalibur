use excalibur_core::network::{HttpHeaders, HttpResponse};
use serde::{Deserialize, Serialize};

/// Vulnerability severity rating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AuditSeverity {
    Critical,
    High,
    Medium,
    Low,
    Informational,
}

impl AuditSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Critical => "CRITICAL",
            Self::High => "HIGH",
            Self::Medium => "MEDIUM",
            Self::Low => "LOW",
            Self::Informational => "INFORMATIONAL",
        }
    }
}

/// A structured finding produced by auditing response headers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeaderAuditFinding {
    pub rule_id: String,
    pub severity: AuditSeverity,
    pub title: String,
    pub description: String,
    pub recommendation: String,
    pub header_name: Option<String>,
    pub header_value: Option<String>,
}

pub struct HeaderAuditor;

impl HeaderAuditor {
    /// Audits an HTTP response's headers against best-practice security baselines.
    pub fn audit_response(res: &HttpResponse, is_https: bool) -> Vec<HeaderAuditFinding> {
        Self::audit_headers(&res.headers, is_https)
    }

    /// Audits a set of HTTP headers.
    pub fn audit_headers(headers: &HttpHeaders, is_https: bool) -> Vec<HeaderAuditFinding> {
        let mut findings = Vec::new();

        // 1. Strict-Transport-Security (HSTS)
        if is_https {
            if let Some(hsts) = headers.get("strict-transport-security") {
                let hsts_lower = hsts.to_ascii_lowercase();
                if !hsts_lower.contains("max-age") {
                    findings.push(HeaderAuditFinding {
                        rule_id: "SEC-HSTS-INVALID".into(),
                        severity: AuditSeverity::Medium,
                        title: "HSTS Header Missing max-age Directive".into(),
                        description: "The Strict-Transport-Security header is present but lacks a valid max-age directive.".into(),
                        recommendation: "Specify max-age=31536000; includeSubDomains; preload.".into(),
                        header_name: Some("Strict-Transport-Security".into()),
                        header_value: Some(hsts.into()),
                    });
                } else if !hsts_lower.contains("includesubdomains") {
                    findings.push(HeaderAuditFinding {
                        rule_id: "SEC-HSTS-NO-SUBDOMAINS".into(),
                        severity: AuditSeverity::Low,
                        title: "HSTS Header Missing includeSubDomains".into(),
                        description: "HSTS is active for the current host but does not extend protection to subdomains.".into(),
                        recommendation: "Add includeSubDomains to your Strict-Transport-Security header.".into(),
                        header_name: Some("Strict-Transport-Security".into()),
                        header_value: Some(hsts.into()),
                    });
                }
            } else {
                findings.push(HeaderAuditFinding {
                    rule_id: "SEC-HSTS-MISSING".into(),
                    severity: AuditSeverity::High,
                    title: "Missing Strict-Transport-Security (HSTS) Header".into(),
                    description: "HTTPS connection does not instruct the browser to enforce secure transport, leaving users vulnerable to SSL-stripping attacks.".into(),
                    recommendation: "Set 'Strict-Transport-Security: max-age=31536000; includeSubDomains' on all HTTPS responses.".into(),
                    header_name: Some("Strict-Transport-Security".into()),
                    header_value: None,
                });
            }
        }

        // 2. Content-Security-Policy (CSP)
        if let Some(csp) = headers.get("content-security-policy") {
            let csp_lower = csp.to_ascii_lowercase();
            if csp_lower.contains("'unsafe-inline'") {
                findings.push(HeaderAuditFinding {
                    rule_id: "SEC-CSP-UNSAFE-INLINE".into(),
                    severity: AuditSeverity::Medium,
                    title: "CSP Allows 'unsafe-inline' Script Execution".into(),
                    description: "Content Security Policy allows inline scripts, significantly diminishing defenses against Cross-Site Scripting (XSS).".into(),
                    recommendation: "Refactor inline scripts to external bundles or adopt cryptographic nonces/hashes.".into(),
                    header_name: Some("Content-Security-Policy".into()),
                    header_value: Some(csp.into()),
                });
            }
            if csp_lower.contains("'unsafe-eval'") {
                findings.push(HeaderAuditFinding {
                    rule_id: "SEC-CSP-UNSAFE-EVAL".into(),
                    severity: AuditSeverity::Medium,
                    title: "CSP Allows 'unsafe-eval' Code Generation".into(),
                    description: "Content Security Policy allows eval() and dynamic string execution.".into(),
                    recommendation: "Remove 'unsafe-eval' from your script-src directive.".into(),
                    header_name: Some("Content-Security-Policy".into()),
                    header_value: Some(csp.into()),
                });
            }
        } else {
            findings.push(HeaderAuditFinding {
                rule_id: "SEC-CSP-MISSING".into(),
                severity: AuditSeverity::Medium,
                title: "Missing Content-Security-Policy (CSP) Header".into(),
                description: "The response does not declare a Content-Security-Policy header, leaving the application without defense-in-depth against XSS and data exfiltration.".into(),
                recommendation: "Define a restrictive Content-Security-Policy (e.g., default-src 'self').".into(),
                header_name: Some("Content-Security-Policy".into()),
                header_value: None,
            });
        }

        // 3. X-Frame-Options (Clickjacking)
        let has_csp_frame_ancestors = headers
            .get("content-security-policy")
            .map(|c| c.to_ascii_lowercase().contains("frame-ancestors"))
            .unwrap_or(false);

        if !has_csp_frame_ancestors {
            if let Some(xfo) = headers.get("x-frame-options") {
                let xfo_upper = xfo.trim().to_ascii_uppercase();
                if xfo_upper != "DENY" && xfo_upper != "SAMEORIGIN" {
                    findings.push(HeaderAuditFinding {
                        rule_id: "SEC-XFO-WEAK".into(),
                        severity: AuditSeverity::Low,
                        title: "Unrecognized or Deprecated X-Frame-Options Directive".into(),
                        description: format!(
                            "X-Frame-Options is set to '{}', which is deprecated or invalid.",
                            xfo
                        ),
                        recommendation: "Use 'DENY' or 'SAMEORIGIN', or use CSP 'frame-ancestors'."
                            .into(),
                        header_name: Some("X-Frame-Options".into()),
                        header_value: Some(xfo.into()),
                    });
                }
            } else {
                findings.push(HeaderAuditFinding {
                    rule_id: "SEC-XFO-MISSING".into(),
                    severity: AuditSeverity::Medium,
                    title: "Missing X-Frame-Options / Clickjacking Protection".into(),
                    description: "Neither X-Frame-Options nor CSP frame-ancestors is configured, allowing the page to be framed inside external iframes for clickjacking attacks.".into(),
                    recommendation: "Send 'X-Frame-Options: DENY' or 'frame-ancestors 'none''.".into(),
                    header_name: Some("X-Frame-Options".into()),
                    header_value: None,
                });
            }
        }

        // 4. X-Content-Type-Options (MIME Sniffing)
        if let Some(xcto) = headers.get("x-content-type-options") {
            if !xcto.trim().eq_ignore_ascii_case("nosniff") {
                findings.push(HeaderAuditFinding {
                    rule_id: "SEC-XCTO-INVALID".into(),
                    severity: AuditSeverity::Low,
                    title: "Invalid X-Content-Type-Options Directive".into(),
                    description: format!("Expected 'nosniff' but found '{}'.", xcto),
                    recommendation: "Set 'X-Content-Type-Options: nosniff'.".into(),
                    header_name: Some("X-Content-Type-Options".into()),
                    header_value: Some(xcto.into()),
                });
            }
        } else {
            findings.push(HeaderAuditFinding {
                rule_id: "SEC-XCTO-MISSING".into(),
                severity: AuditSeverity::Low,
                title: "Missing X-Content-Type-Options Header".into(),
                description: "Without 'nosniff', browsers may inspect the response payload and execute non-script assets as executable scripts.".into(),
                recommendation: "Add 'X-Content-Type-Options: nosniff' to prevent MIME-sniffing.".into(),
                header_name: Some("X-Content-Type-Options".into()),
                header_value: None,
            });
        }

        // 5. Cross-Origin Resource Sharing (CORS) Misconfigurations
        if let Some(origin) = headers.get("access-control-allow-origin") {
            let creds = headers.get("access-control-allow-credentials");
            if origin.trim() == "*"
                && creds
                    .map(|c| c.trim().eq_ignore_ascii_case("true"))
                    .unwrap_or(false)
            {
                findings.push(HeaderAuditFinding {
                    rule_id: "SEC-CORS-WILDCARD-CREDS".into(),
                    severity: AuditSeverity::High,
                    title: "Insecure CORS: Wildcard Origin With Credentials".into(),
                    description: "Access-Control-Allow-Origin is set to '*' while Access-Control-Allow-Credentials is true, allowing unauthorized cross-origin data exposure.".into(),
                    recommendation: "Do not use wildcard origin when credentials are supported; whitelist explicit origins.".into(),
                    header_name: Some("Access-Control-Allow-Origin".into()),
                    header_value: Some(format!("Origin: *, Credentials: {:?}", creds)),
                });
            } else if origin.trim().eq_ignore_ascii_case("null") {
                findings.push(HeaderAuditFinding {
                    rule_id: "SEC-CORS-NULL-ORIGIN".into(),
                    severity: AuditSeverity::Medium,
                    title: "Insecure CORS: Null Origin Trusted".into(),
                    description: "Access-Control-Allow-Origin trusts 'null', which can be triggered by sandboxed iframes or file:/// URIs.".into(),
                    recommendation: "Avoid trusting the 'null' origin in Access-Control-Allow-Origin.".into(),
                    header_name: Some("Access-Control-Allow-Origin".into()),
                    header_value: Some(origin.into()),
                });
            }
        }

        // 6. Cookie Security (Set-Cookie headers)
        for cookie in headers.get_all("set-cookie") {
            let cookie_lower = cookie.to_ascii_lowercase();
            let is_session_cookie = cookie_lower.contains("session")
                || cookie_lower.contains("token")
                || cookie_lower.contains("auth")
                || cookie_lower.contains("jwt")
                || cookie_lower.contains("sid")
                || cookie_lower.contains("id=");

            if is_https && !cookie_lower.contains("secure") {
                findings.push(HeaderAuditFinding {
                    rule_id: "SEC-COOKIE-NO-SECURE".into(),
                    severity: AuditSeverity::Medium,
                    title: "Cookie Missing Secure Flag".into(),
                    description: format!("Set-Cookie header does not specify the Secure attribute on an HTTPS transport: {}", cookie),
                    recommendation: "Append '; Secure' to all cookies transmitted over HTTPS.".into(),
                    header_name: Some("Set-Cookie".into()),
                    header_value: Some(cookie.into()),
                });
            }

            if is_session_cookie && !cookie_lower.contains("httponly") {
                findings.push(HeaderAuditFinding {
                    rule_id: "SEC-COOKIE-NO-HTTPONLY".into(),
                    severity: AuditSeverity::High,
                    title: "Session Cookie Missing HttpOnly Flag".into(),
                    description: format!("Potential authentication cookie lacks HttpOnly flag and can be accessed via document.cookie by client-side scripts: {}", cookie),
                    recommendation: "Add '; HttpOnly' to prevent cookie theft via XSS.".into(),
                    header_name: Some("Set-Cookie".into()),
                    header_value: Some(cookie.into()),
                });
            }

            if !cookie_lower.contains("samesite") {
                findings.push(HeaderAuditFinding {
                    rule_id: "SEC-COOKIE-NO-SAMESITE".into(),
                    severity: AuditSeverity::Low,
                    title: "Cookie Missing SameSite Attribute".into(),
                    description: format!("Cookie does not specify SameSite attribute, which may increase CSRF susceptibility: {}", cookie),
                    recommendation: "Specify '; SameSite=Lax' or '; SameSite=Strict'.".into(),
                    header_name: Some("Set-Cookie".into()),
                    header_value: Some(cookie.into()),
                });
            }
        }

        // 7. Information Leakage Headers
        if let Some(srv) = headers.get("server") {
            if srv.chars().any(|c| c.is_ascii_digit()) {
                findings.push(HeaderAuditFinding {
                    rule_id: "SEC-INFO-SERVER-VERSION".into(),
                    severity: AuditSeverity::Informational,
                    title: "Server Banner Version Disclosure".into(),
                    description: format!(
                        "Server header reveals granular version information: '{}'.",
                        srv
                    ),
                    recommendation:
                        "Suppress detailed server version numbers in production headers.".into(),
                    header_name: Some("Server".into()),
                    header_value: Some(srv.into()),
                });
            }
        }
        if let Some(xpb) = headers.get("x-powered-by") {
            findings.push(HeaderAuditFinding {
                rule_id: "SEC-INFO-X-POWERED-BY".into(),
                severity: AuditSeverity::Informational,
                title: "Framework Disclosure via X-Powered-By".into(),
                description: format!(
                    "X-Powered-By header discloses underlying technology stack: '{}'.",
                    xpb
                ),
                recommendation: "Remove X-Powered-By headers from your application server.".into(),
                header_name: Some("X-Powered-By".into()),
                header_value: Some(xpb.into()),
            });
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_all_security_headers() {
        let headers = HttpHeaders::new();
        let findings = HeaderAuditor::audit_headers(&headers, true);

        let rule_ids: Vec<_> = findings.iter().map(|f| f.rule_id.as_str()).collect();
        assert!(rule_ids.contains(&"SEC-HSTS-MISSING"));
        assert!(rule_ids.contains(&"SEC-CSP-MISSING"));
        assert!(rule_ids.contains(&"SEC-XFO-MISSING"));
        assert!(rule_ids.contains(&"SEC-XCTO-MISSING"));
    }

    #[test]
    fn test_insecure_cors_and_cookies() {
        let mut headers = HttpHeaders::new();
        headers.append("Access-Control-Allow-Origin", "*");
        headers.append("Access-Control-Allow-Credentials", "true");
        headers.append("Set-Cookie", "session_id=abcdef123456; Path=/");

        let findings = HeaderAuditor::audit_headers(&headers, true);
        let rule_ids: Vec<_> = findings.iter().map(|f| f.rule_id.as_str()).collect();

        assert!(rule_ids.contains(&"SEC-CORS-WILDCARD-CREDS"));
        assert!(rule_ids.contains(&"SEC-COOKIE-NO-SECURE"));
        assert!(rule_ids.contains(&"SEC-COOKIE-NO-HTTPONLY"));
        assert!(rule_ids.contains(&"SEC-COOKIE-NO-SAMESITE"));
    }

    #[test]
    fn test_hardened_headers() {
        let mut headers = HttpHeaders::new();
        headers.append(
            "Strict-Transport-Security",
            "max-age=31536000; includeSubDomains; preload",
        );
        headers.append("Content-Security-Policy", "default-src 'self'");
        headers.append("X-Frame-Options", "DENY");
        headers.append("X-Content-Type-Options", "nosniff");
        headers.append(
            "Set-Cookie",
            "auth_token=xyz; Secure; HttpOnly; SameSite=Strict",
        );

        let findings = HeaderAuditor::audit_headers(&headers, true);
        assert!(
            findings.is_empty(),
            "Expected 0 findings for hardened headers, got: {:?}",
            findings
        );
    }
}
