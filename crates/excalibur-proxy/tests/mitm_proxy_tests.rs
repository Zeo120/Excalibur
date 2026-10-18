use excalibur_core::client::{ClientOptions, HttpClient};
use excalibur_core::network::HttpRequest;
use excalibur_core::storage::WorkspaceStorage;
use excalibur_proxy::cert::CertificateAuthority;
use excalibur_proxy::interceptor::{InterceptAction, InterceptedEvent, TrafficInterceptor};
use excalibur_proxy::server::ProxyServer;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_plain_http_proxy_flow() {
    // 1. Upstream Mock Server
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream_listener.local_addr().unwrap();

    tokio::spawn(async move {
        if let Ok((mut socket, _)) = upstream_listener.accept().await {
            let mut buf = [0u8; 2048];
            let _ = socket.read(&mut buf).await;
            let resp = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 14\r\n\r\nUpstream Hello";
            let _ = socket.write_all(resp.as_bytes()).await;
        }
    });

    // 2. Excalibur MITM Proxy Server
    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr: SocketAddr = proxy_listener.local_addr().unwrap();
    drop(proxy_listener); // Release so ProxyServer can bind

    let ca = Arc::new(CertificateAuthority::new_ephemeral().unwrap());
    let (event_tx, _event_rx) = mpsc::channel(100);
    let interceptor = Arc::new(TrafficInterceptor::new(event_tx));
    let storage = Arc::new(WorkspaceStorage::in_memory().unwrap());

    let server = ProxyServer::new(
        proxy_addr,
        ca,
        Arc::clone(&interceptor),
        Some(Arc::clone(&storage)),
        None,
    )
    .unwrap();

    tokio::spawn(async move {
        let _ = server.run().await;
    });

    // Give proxy a moment to bind
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // 3. Client routing through Proxy
    let client_opts = ClientOptions {
        upstream_proxy: Some(format!("http://{}", proxy_addr)),
        ..Default::default()
    };
    let client = HttpClient::new(client_opts).unwrap();

    let req = HttpRequest::new("GET", format!("http://{}/api/data", upstream_addr));
    let resp = client.dispatch(&req).await.expect("Proxy dispatch failed");

    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, b"Upstream Hello");

    // 4. Verify storage recorded the proxied transaction
    let stored_req = storage.get_request(&req.id).unwrap();
    assert!(stored_req.is_some());
}

#[tokio::test]
async fn test_proxy_interception_tampering_and_dropping() {
    // 1. Upstream Mock Server checking for tampered header
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream_listener.local_addr().unwrap();

    tokio::spawn(async move {
        while let Ok((mut socket, _)) = upstream_listener.accept().await {
            let mut buf = [0u8; 2048];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let req_str = String::from_utf8_lossy(&buf[..n]);

            if req_str.to_ascii_lowercase().contains("x-tampered: active") {
                let resp = "HTTP/1.1 200 OK\r\nContent-Length: 16\r\n\r\nTampered Success";
                let _ = socket.write_all(resp.as_bytes()).await;
            } else {
                let resp = "HTTP/1.1 400 Bad Request\r\nContent-Length: 14\r\n\r\nMissing Header";
                let _ = socket.write_all(resp.as_bytes()).await;
            }
        }
    });

    // 2. Proxy Server with Interception Enabled
    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    drop(proxy_listener);

    let ca = Arc::new(CertificateAuthority::new_ephemeral().unwrap());
    let (event_tx, mut event_rx) = mpsc::channel(100);
    let interceptor = Arc::new(TrafficInterceptor::new(event_tx));
    interceptor.set_enabled(true);

    let server = ProxyServer::new(proxy_addr, ca, Arc::clone(&interceptor), None, None).unwrap();

    tokio::spawn(async move {
        let _ = server.run().await;
    });

    // Mock UI Handler: Intercepts request and injects X-Tampered header
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                InterceptedEvent::Request {
                    mut request,
                    responder,
                } => {
                    if request.url.contains("/drop-me") {
                        let _ = responder.send(InterceptAction::Drop);
                    } else {
                        request.headers.append("X-Tampered", "Active");
                        let _ = responder.send(InterceptAction::ModifyRequest(request));
                    }
                }
                InterceptedEvent::Response {
                    response,
                    responder,
                } => {
                    let _ = responder.send(InterceptAction::ModifyResponse(response));
                }
            }
        }
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let client_opts = ClientOptions {
        upstream_proxy: Some(format!("http://{}", proxy_addr)),
        ..Default::default()
    };
    let client = HttpClient::new(client_opts).unwrap();

    // Test 1: Tampered request succeeds
    let req1 = HttpRequest::new("GET", format!("http://{}/modify-me", upstream_addr));
    let resp1 = client.dispatch(&req1).await.expect("Dispatch 1 failed");
    assert_eq!(resp1.status, 200);
    assert_eq!(resp1.body, b"Tampered Success");

    // Test 2: Dropped request returns 403
    let req2 = HttpRequest::new("GET", format!("http://{}/drop-me", upstream_addr));
    let resp2 = client.dispatch(&req2).await.expect("Dispatch 2 failed");
    assert_eq!(resp2.status, 403);
    assert!(String::from_utf8_lossy(&resp2.body).contains("dropped"));
}
