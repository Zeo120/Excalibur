use crate::error::{CoreError, Result};
use crate::network::{HttpHeaders, HttpRequest, HttpResponse};
use crate::tls::{Ja3Preset, TlsConfigBuilder};
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper_util::rt::{TokioExecutor, TokioIo};
use rustls::pki_types::ServerName;
use std::sync::Arc;
use std::time::Instant;
use tokio_rustls::TlsConnector;
use url::Url;

enum HttpSender {
    Http1(hyper::client::conn::http1::SendRequest<Full<Bytes>>),
    Http2(hyper::client::conn::http2::SendRequest<Full<Bytes>>),
}

impl HttpSender {
    async fn send_request(
        &mut self,
        req: hyper::Request<Full<Bytes>>,
    ) -> std::result::Result<hyper::Response<hyper::body::Incoming>, hyper::Error> {
        match self {
            Self::Http1(s) => s.send_request(req).await,
            Self::Http2(s) => s.send_request(req).await,
        }
    }
}

/// Configuration options for the HTTP/HTTPS client.
#[derive(Debug, Clone)]
pub struct ClientOptions {
    pub bypass_tls_verification: bool,
    pub ja3_preset: Ja3Preset,
    pub upstream_proxy: Option<String>,
    pub timeout_seconds: u64,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            bypass_tls_verification: false,
            ja3_preset: Ja3Preset::ModernStandard,
            upstream_proxy: None,
            timeout_seconds: 30,
        }
    }
}

/// HTTP/HTTPS Client Dispatcher.
pub struct HttpClient {
    options: ClientOptions,
    tls_connector: TlsConnector,
}

impl HttpClient {
    pub fn new(options: ClientOptions) -> Result<Self> {
        let tls_config = TlsConfigBuilder::new()
            .bypass_verification(options.bypass_tls_verification)
            .ja3_preset(options.ja3_preset)
            .build()
            .map_err(|e| CoreError::Tls(e.to_string()))?;

        Ok(Self {
            options,
            tls_connector: TlsConnector::from(Arc::new(tls_config)),
        })
    }

    /// Dispatches an HttpRequest and follows HTTP redirects.
    pub async fn dispatch(&self, req: &HttpRequest) -> Result<HttpResponse> {
        let mut current_req = req.clone();
        let mut redirects_left = 5;

        loop {
            let res = self.dispatch_single(&current_req).await?;
            if (res.status == 301
                || res.status == 302
                || res.status == 303
                || res.status == 307
                || res.status == 308)
                && redirects_left > 0
                && (current_req.method == "GET" || current_req.method == "HEAD")
            {
                if let Some(location) = res.headers.get("location") {
                    let base_url = Url::parse(&current_req.url)?;
                    if let Ok(next_url) = base_url.join(location) {
                        current_req.url = next_url.to_string();
                        redirects_left -= 1;
                        continue;
                    }
                }
            }
            let mut final_res = res;
            final_res.headers.set("X-Final-URL", &current_req.url);
            return Ok(final_res);
        }
    }

    /// Dispatches a single HttpRequest without following redirects.
    pub async fn dispatch_single(&self, req: &HttpRequest) -> Result<HttpResponse> {
        let parsed_url = Url::parse(&req.url)?;
        let scheme = parsed_url.scheme();
        let host = parsed_url
            .host_str()
            .ok_or_else(|| CoreError::Http("URL is missing a valid host".to_string()))?;

        let port = parsed_url
            .port_or_known_default()
            .unwrap_or(if scheme == "https" { 443 } else { 80 });

        let start_time = Instant::now();

        let tcp_stream = match &self.options.upstream_proxy {
            Some(proxy_url_str) => {
                let proxy_url = Url::parse(proxy_url_str)?;
                let p_host = proxy_url
                    .host_str()
                    .ok_or_else(|| CoreError::Http("Invalid upstream proxy host".to_string()))?;
                let p_port = proxy_url.port().unwrap_or(8080);
                let mut p_stream = crate::dns::connect_tcp_resilient(
                    p_host,
                    p_port,
                    std::time::Duration::from_secs(self.options.timeout_seconds),
                )
                .await?;

                if scheme == "https" {
                    // Send HTTP CONNECT tunnel request to proxy
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let connect_req = format!(
                        "CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\nProxy-Connection: Keep-Alive\r\n\r\n",
                        host, port, host, port
                    );
                    p_stream.write_all(connect_req.as_bytes()).await?;

                    let mut resp_buf = [0u8; 1024];
                    let n = p_stream.read(&mut resp_buf).await?;
                    let resp_str = String::from_utf8_lossy(&resp_buf[..n]);
                    if !resp_str.starts_with("HTTP/1.1 200")
                        && !resp_str.starts_with("HTTP/1.0 200")
                    {
                        return Err(CoreError::Http(format!(
                            "Upstream proxy rejected CONNECT tunnel: {}",
                            resp_str.lines().next().unwrap_or("")
                        )));
                    }
                }
                p_stream
            }
            None => {
                crate::dns::connect_tcp_resilient(
                    host,
                    port,
                    std::time::Duration::from_secs(self.options.timeout_seconds),
                )
                .await?
            }
        };

        let path_and_query = match parsed_url.query() {
            Some(q) => format!("{}?{}", parsed_url.path(), q),
            None => parsed_url.path().to_string(),
        };

        let is_https = scheme == "https";

        let (mut sender, conn_handle, is_h2) = if is_https {
            let server_name = ServerName::try_from(host.to_string())
                .map_err(|e| CoreError::Tls(format!("Invalid server name: {}", e)))?;

            let tls_stream = self
                .tls_connector
                .connect(server_name, tcp_stream)
                .await
                .map_err(|e| CoreError::Tls(format!("TLS handshake failed: {}", e)))?;

            let is_h2 = tls_stream.get_ref().1.alpn_protocol() == Some(b"h2");
            let io = TokioIo::new(tls_stream);

            if is_h2 {
                let (sender, conn) =
                    hyper::client::conn::http2::handshake(TokioExecutor::new(), io)
                        .await
                        .map_err(|e| CoreError::Http(format!("HTTP/2 handshake failed: {}", e)))?;

                let handle = tokio::spawn(async move {
                    if let Err(err) = conn.await {
                        tracing::debug!("HTTP/2 connection closed: {:?}", err);
                    }
                });
                (HttpSender::Http2(sender), handle, true)
            } else {
                let (sender, conn) = hyper::client::conn::http1::handshake(io)
                    .await
                    .map_err(|e| CoreError::Http(format!("HTTP/1.1 handshake failed: {}", e)))?;

                let handle = tokio::spawn(async move {
                    if let Err(err) = conn.await {
                        tracing::debug!("HTTP/1.1 connection closed: {:?}", err);
                    }
                });
                (HttpSender::Http1(sender), handle, false)
            }
        } else {
            let io = TokioIo::new(tcp_stream);
            let (sender, conn) = hyper::client::conn::http1::handshake(io)
                .await
                .map_err(|e| CoreError::Http(format!("HTTP/1.1 handshake failed: {}", e)))?;

            let handle = tokio::spawn(async move {
                if let Err(err) = conn.await {
                    tracing::debug!("HTTP/1.1 connection closed: {:?}", err);
                }
            });
            (HttpSender::Http1(sender), handle, false)
        };

        // Build hyper request
        let mut builder = hyper::Request::builder().method(req.method.as_str());

        if is_h2 {
            // HTTP/2 requires absolute URI or pseudo-headers :scheme and :authority
            builder = builder.uri(parsed_url.as_str());
        } else {
            builder = builder.uri(path_and_query);
        }

        for (k, v) in req.headers.iter() {
            builder = builder.header(k, v);
        }

        // Add Host header if missing
        if !req.headers.contains_key("host") {
            let host_hdr = if (is_https && port == 443) || (!is_https && port == 80) {
                host.to_string()
            } else {
                format!("{}:{}", host, port)
            };
            builder = builder.header("Host", host_hdr);
        }

        if !req.headers.contains_key("x-request-id") {
            builder = builder.header("X-Request-ID", &req.id);
        }

        let body = Full::new(Bytes::from(req.body.clone()));
        let hyper_req = builder
            .body(body)
            .map_err(|e| CoreError::Http(e.to_string()))?;

        let response = sender
            .send_request(hyper_req)
            .await
            .map_err(|e| CoreError::Http(format!("Failed to send HTTP request: {}", e)))?;

        let elapsed_ms = start_time.elapsed().as_millis() as u64;

        let status = response.status().as_u16();
        let reason = response
            .status()
            .canonical_reason()
            .unwrap_or("")
            .to_string();
        let version = format!("{:?}", response.version());

        let mut resp_headers = HttpHeaders::new();
        for (name, value) in response.headers() {
            if let Ok(val_str) = value.to_str() {
                resp_headers.append(name.as_str(), val_str);
            }
        }

        let body_bytes = response
            .into_body()
            .collect()
            .await
            .map_err(|e| CoreError::Http(format!("Failed to read response body: {}", e)))?
            .to_bytes()
            .to_vec();

        drop(conn_handle);

        Ok(HttpResponse {
            request_id: req.id.clone(),
            status,
            reason,
            version,
            headers: resp_headers,
            body: body_bytes,
            elapsed_ms,
            timestamp: chrono::Utc::now().timestamp_millis(),
        })
    }
}
