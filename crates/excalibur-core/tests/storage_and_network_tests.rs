use excalibur_core::client::{ClientOptions, HttpClient};
use excalibur_core::network::{HttpRequest, HttpResponse};
use excalibur_core::storage::{AuditFindingRecord, WorkspaceStorage};
use excalibur_core::tls::{Ja3Preset, PermissiveCertVerifier, TlsConfigBuilder};
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[test]
fn test_sqlite_batch_insert_performance() {
    let storage = WorkspaceStorage::in_memory().expect("Failed to initialize in-memory storage");

    let count = 1000;
    let start = Instant::now();

    for i in 0..count {
        let mut req = HttpRequest::new("POST", format!("https://example.com/api/item/{}", i));
        req.headers.append("Content-Type", "application/json");
        req.headers.append("X-Batch-Index", i.to_string());
        req.body = format!("{{\"index\": {}}}", i).into_bytes();

        storage.save_request(&req).expect("Failed to save request");

        let mut res = HttpResponse::new(&req.id, 200);
        res.headers.append("Server", "nginx/1.24");
        res.body = b"{\"status\":\"created\"}".to_vec();
        storage
            .save_response(&res)
            .expect("Failed to save response");
    }

    let elapsed = start.elapsed();
    println!("Inserted {} requests & responses in {:?}", count, elapsed);

    // Verify retrieval
    let list = storage
        .list_requests(10, 0)
        .expect("Failed to list requests");
    assert_eq!(list.len(), 10);
}

#[test]
fn test_fts5_full_text_search() {
    let storage = WorkspaceStorage::in_memory().expect("Failed to initialize storage");

    let mut req1 = HttpRequest::new(
        "GET",
        "https://app.target.com/oauth/callback?code=AUTH_CODE_SECRET",
    );
    req1.headers.append("Authorization", "Bearer TOKEN_XYZ_987");
    req1.body = b"param=injection_payload_123".to_vec();
    storage.save_request(&req1).expect("Failed to save req1");

    let mut req2 = HttpRequest::new("POST", "https://app.target.com/api/v2/payment");
    req2.headers.append("Content-Type", "application/json");
    req2.body = b"{\"card\": \"4111222233334444\"}".to_vec();
    storage.save_request(&req2).expect("Failed to save req2");

    // Search by URL fragment
    let search_code = storage
        .search_requests("AUTH_CODE_SECRET")
        .expect("Search failed");
    assert!(!search_code.is_empty());
    assert_eq!(search_code[0], req1.id);

    // Search by body content
    let search_card = storage
        .search_requests("4111222233334444")
        .expect("Search failed");
    assert!(!search_card.is_empty());
    assert_eq!(search_card[0], req2.id);
}

#[test]
fn test_audit_findings_storage() {
    let storage = WorkspaceStorage::in_memory().expect("Failed to initialize storage");

    let finding = AuditFindingRecord {
        id: "finding-001".to_string(),
        request_id: "req-001".to_string(),
        category: "Header Misconfiguration".to_string(),
        severity: "Medium".to_string(),
        title: "Missing Content-Security-Policy".to_string(),
        description: "The response does not specify a CSP header.".to_string(),
        matched_data: None,
        timestamp: 1600000000,
    };

    // Need request first due to foreign key
    let req = HttpRequest::new("GET", "https://example.com/");
    let mut finding_with_real_req = finding.clone();
    finding_with_real_req.request_id = req.id.clone();

    storage.save_request(&req).expect("Failed to save req");
    storage
        .save_finding(&finding_with_real_req)
        .expect("Failed to save finding");

    let findings = storage.list_findings().expect("Failed to list findings");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].id, "finding-001");
    assert_eq!(findings[0].severity, "Medium");
}

#[tokio::test]
async fn test_http_client_local_dispatch() {
    // Spin up local TCP test server
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind listener");
    let local_addr = listener.local_addr().expect("Failed to get local addr");

    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf).await;
            let response = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 15\r\n\r\nHello Excalibur";
            let _ = socket.write_all(response.as_bytes()).await;
        }
    });

    let client = HttpClient::new(ClientOptions::default()).expect("Failed to create client");
    let mut req = HttpRequest::new("GET", format!("http://{}/test", local_addr));
    req.headers.append("User-Agent", "Excalibur/0.1.0");

    let resp = client
        .dispatch(&req)
        .await
        .expect("Failed to dispatch request");
    assert_eq!(resp.status, 200);
    assert_eq!(resp.headers.get("content-type"), Some("text/plain"));
    assert_eq!(resp.body, b"Hello Excalibur");
    assert!(resp.elapsed_ms < 5000);
}

#[test]
fn test_tls_builder_options() {
    let config = TlsConfigBuilder::new()
        .bypass_verification(true)
        .ja3_preset(Ja3Preset::Chrome)
        .build();
    assert!(config.is_ok());

    let verifier = PermissiveCertVerifier::new(true);
    assert!(verifier.bypass_verification);
}

#[tokio::test]
async fn test_resilient_connect_and_dns_fallback() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let port = addr.port();

    tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            let mut buf = [0u8; 4];
            let _ = stream.read(&mut buf).await;
            let _ = stream.write_all(b"PONG").await;
        }
    });

    let mut stream =
        excalibur_core::connect_tcp_resilient("127.0.0.1", port, std::time::Duration::from_secs(3))
            .await
            .unwrap();
    stream.write_all(b"PING").await.unwrap();
    let mut resp = [0u8; 4];
    stream.read_exact(&mut resp).await.unwrap();
    assert_eq!(&resp, b"PONG");
}
