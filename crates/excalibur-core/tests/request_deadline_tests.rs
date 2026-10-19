//! Regression tests for whole-request deadlines and real cancellation (issue #15).

use excalibur_core::{ClientOptions, CoreError, HttpClient, HttpRequest};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// Accepts one connection, optionally writes `reply`, then reports when the
/// client closes the socket (read returns 0 / error).
async fn stalling_server(reply: &'static [u8]) -> (String, oneshot::Receiver<Instant>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (closed_tx, closed_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let _ = sock.read(&mut buf).await; // request head
        if !reply.is_empty() {
            let _ = sock.write_all(reply).await;
        }
        // Never finish the response; wait for the client to hang up.
        loop {
            match sock.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(_) => continue,
            }
        }
        let _ = closed_tx.send(Instant::now());
    });
    (format!("http://{}/", addr), closed_rx)
}

fn client(timeout_seconds: u64) -> HttpClient {
    HttpClient::new(ClientOptions {
        timeout_seconds,
        ..Default::default()
    })
    .unwrap()
}

#[tokio::test]
async fn test_unresponsive_server_hits_deadline_and_socket_is_closed() {
    let (url, closed) = stalling_server(b"").await;
    let start = Instant::now();
    let err = client(1)
        .dispatch(&HttpRequest::new("GET", url))
        .await
        .unwrap_err();
    let elapsed = start.elapsed();
    assert!(
        matches!(err, CoreError::Timeout(_)),
        "unexpected error: {err}"
    );
    assert!(
        elapsed >= Duration::from_millis(900),
        "too early: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(4),
        "deadline not enforced: {elapsed:?}"
    );

    // The connection must be torn down, not left running in a detached task.
    let closed_at = tokio::time::timeout(Duration::from_secs(3), closed)
        .await
        .expect("server never saw the client close the socket")
        .unwrap();
    assert!(closed_at.duration_since(start) < Duration::from_secs(4));
}

#[tokio::test]
async fn test_stalled_response_body_hits_deadline() {
    // Headers promise 100 bytes, only 5 arrive.
    let (url, closed) =
        stalling_server(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nhello").await;
    let start = Instant::now();
    let err = client(1)
        .dispatch(&HttpRequest::new("GET", url))
        .await
        .unwrap_err();
    assert!(
        matches!(err, CoreError::Timeout(_)),
        "unexpected error: {err}"
    );
    assert!(start.elapsed() < Duration::from_secs(4));
    tokio::time::timeout(Duration::from_secs(3), closed)
        .await
        .expect("socket left open after body timeout")
        .unwrap();
}

#[tokio::test]
async fn test_dispatch_single_is_also_bounded() {
    let (url, _closed) = stalling_server(b"").await;
    let start = Instant::now();
    let err = client(1)
        .dispatch_single(&HttpRequest::new("GET", url))
        .await
        .unwrap_err();
    assert!(matches!(err, CoreError::Timeout(_)));
    assert!(start.elapsed() < Duration::from_secs(4));
}

#[tokio::test]
async fn test_aborting_request_task_closes_connection() {
    // Long deadline: only cancellation can end this request.
    let (url, closed) = stalling_server(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nhel").await;
    let task =
        tokio::spawn(async move { client(120).dispatch(&HttpRequest::new("GET", url)).await });
    tokio::time::sleep(Duration::from_millis(300)).await;
    let aborted_at = Instant::now();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    let closed_at = tokio::time::timeout(Duration::from_secs(3), closed)
        .await
        .expect("aborted request kept its socket open")
        .unwrap();
    assert!(closed_at.duration_since(aborted_at) < Duration::from_secs(3));
}
