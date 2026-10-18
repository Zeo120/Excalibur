use excalibur_audit::{ResponseAuditor, SecretScanner};
use excalibur_core::network::HttpResponse;

#[test]
fn test_response_auditor_end_to_end() {
    let auditor = ResponseAuditor::new();

    let mut res = HttpResponse::new("req-audit-1", 200);
    // Add insecure CORS and leak an AWS key in response body
    res.headers.append("Access-Control-Allow-Origin", "*");
    res.headers
        .append("Access-Control-Allow-Credentials", "true");
    res.headers.append("Server", "Apache/2.4.41 (Ubuntu)");
    res.body = br#"{
        "status": "success",
        "api_key": "AKIAIOSFODNN7EXAMPLE",
        "access_token": "ghp_mockSecretTokenForIntegrationTesting123"
    }"#
    .to_vec();

    let report = auditor.audit_response(&res, true);

    assert!(!report.is_clean());
    assert!(report.total_issues() >= 5);

    // Verify header issues identified
    let header_rules: Vec<_> = report
        .header_findings
        .iter()
        .map(|f| f.rule_id.as_str())
        .collect();
    assert!(header_rules.contains(&"SEC-HSTS-MISSING"));
    assert!(header_rules.contains(&"SEC-CSP-MISSING"));
    assert!(header_rules.contains(&"SEC-CORS-WILDCARD-CREDS"));
    assert!(header_rules.contains(&"SEC-INFO-SERVER-VERSION"));

    // Verify secret findings
    let secret_rules: Vec<_> = report
        .secret_findings
        .iter()
        .map(|f| f.rule_id.as_str())
        .collect();
    assert!(secret_rules.contains(&"SEC-AWS-KEY-ID"));
    assert!(secret_rules.contains(&"SEC-GITHUB-TOKEN"));

    let aws_finding = report
        .secret_findings
        .iter()
        .find(|f| f.rule_id == "SEC-AWS-KEY-ID")
        .unwrap();
    assert_eq!(aws_finding.matched_text, "AKIAIOSFODNN7EXAMPLE");
    assert_eq!(aws_finding.redacted_text, "AKIA...MPLE");
}

#[test]
fn test_clean_response_audit() {
    let auditor = ResponseAuditor::new();

    let mut res = HttpResponse::new("req-clean", 200);
    res.headers.append(
        "Strict-Transport-Security",
        "max-age=63072000; includeSubDomains; preload",
    );
    res.headers
        .append("Content-Security-Policy", "default-src 'self'");
    res.headers.append("X-Frame-Options", "DENY");
    res.headers.append("X-Content-Type-Options", "nosniff");
    res.body = b"<html><body>Clean public response</body></html>".to_vec();

    let report = auditor.audit_response(&res, true);
    assert!(
        report.is_clean(),
        "Expected clean report, found: {:?}",
        report
    );
}

#[test]
fn test_shannon_entropy_calculation() {
    let zero = SecretScanner::shannon_entropy("");
    assert_eq!(zero, 0.0);

    let low = SecretScanner::shannon_entropy("aaaaaaaaaaaaaaaa");
    assert_eq!(low, 0.0);

    let high = SecretScanner::shannon_entropy("7f8b9c0d1e2f3a4b5c6d7e8f");
    assert!(high > 3.0);
}
