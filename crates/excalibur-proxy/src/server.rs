use crate::cert::CertificateAuthority;
use crate::interceptor::TrafficInterceptor;
use anyhow::{anyhow, Result};
use bytes::Bytes;
use excalibur_core::client::{ClientOptions, HttpClient};
use excalibur_core::network::{HttpHeaders, HttpRequest};
use excalibur_core::storage::WorkspaceStorage;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use rustls::ServerConfig;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;

/// MITM Proxy Server coordinating inbound connections, TLS forging, interception, and storage.
pub struct ProxyServer {
    pub bind_addr: SocketAddr,
    ca: Arc<CertificateAuthority>,
    interceptor: Arc<TrafficInterceptor>,
    storage: Option<Arc<WorkspaceStorage>>,
    http_client: Arc<HttpClient>,
}

impl ProxyServer {
    pub fn new(
        bind_addr: SocketAddr,
        ca: Arc<CertificateAuthority>,
        interceptor: Arc<TrafficInterceptor>,
        storage: Option<Arc<WorkspaceStorage>>,
        client_options: Option<ClientOptions>,
    ) -> Result<Self> {
        let http_client = Arc::new(HttpClient::new(client_options.unwrap_or_default())?);

        Ok(Self {
            bind_addr,
            ca,
            interceptor,
            storage,
            http_client,
        })
    }

    pub async fn run(&self) -> Result<()> {
        let listener = TcpListener::bind(self.bind_addr).await?;
        tracing::info!("[*] Excalibur MITM Proxy listening on {}", self.bind_addr);

        loop {
            let (stream, client_addr) = listener.accept().await?;
            let ca = Arc::clone(&self.ca);
            let interceptor = Arc::clone(&self.interceptor);
            let storage = self.storage.as_ref().map(Arc::clone);
            let http_client = Arc::clone(&self.http_client);

            tokio::spawn(async move {
                if let Err(err) =
                    Self::handle_client(stream, client_addr, ca, interceptor, storage, http_client)
                        .await
                {
                    tracing::debug!("Proxy connection error from {}: {:?}", client_addr, err);
                }
            });
        }
    }

    async fn handle_client(
        stream: TcpStream,
        client_addr: SocketAddr,
        ca: Arc<CertificateAuthority>,
        interceptor: Arc<TrafficInterceptor>,
        storage: Option<Arc<WorkspaceStorage>>,
        http_client: Arc<HttpClient>,
    ) -> Result<()> {
        // Peek at initial bytes to determine if this is an HTTPS CONNECT or plain HTTP request
        let mut peek_buf = [0u8; 4096];
        let n = stream.peek(&mut peek_buf).await?;
        if n == 0 {
            return Ok(());
        }

        let peek_str = String::from_utf8_lossy(&peek_buf[..n]);

        if peek_str.starts_with("CONNECT ") {
            Self::handle_https_connect(stream, &peek_str, ca, interceptor, storage, http_client)
                .await
        } else {
            Self::handle_plain_http(stream, interceptor, storage, http_client, client_addr).await
        }
    }

    async fn handle_https_connect(
        mut stream: TcpStream,
        peek_str: &str,
        ca: Arc<CertificateAuthority>,
        interceptor: Arc<TrafficInterceptor>,
        storage: Option<Arc<WorkspaceStorage>>,
        http_client: Arc<HttpClient>,
    ) -> Result<()> {
        // Parse the target host and port from CONNECT target:port HTTP/1.1
        let first_line = peek_str.lines().next().unwrap_or("");
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() < 2 {
            return Err(anyhow!("Malformed CONNECT request"));
        }

        let target_authority = parts[1];
        let target_host = target_authority
            .split(':')
            .next()
            .unwrap_or(target_authority);

        // Read and consume the CONNECT request headers
        let mut header_buf = Vec::new();
        let mut byte = [0u8; 1];
        while !header_buf.ends_with(b"\r\n\r\n") {
            let read_n = stream.read(&mut byte).await?;
            if read_n == 0 {
                return Err(anyhow!("Client disconnected before completing CONNECT"));
            }
            header_buf.push(byte[0]);
        }

        // Send 200 Connection Established to client
        stream
            .write_all(
                b"HTTP/1.1 200 Connection Established\r\nProxy-Agent: Excalibur/0.1.0\r\n\r\n",
            )
            .await?;

        // Forge leaf certificate for target host
        let (cert_chain, private_key) = ca.get_or_create_leaf_cert(target_host)?;

        let server_config = ServerConfig::builder_with_provider(
            rustls::crypto::aws_lc_rs::default_provider().into(),
        )
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(cert_chain, private_key)?;

        let acceptor = TlsAcceptor::from(Arc::new(server_config));
        let tls_stream = acceptor.accept(stream).await?;
        let io = TokioIo::new(tls_stream);

        let target_host_string = target_host.to_string();
        let service = service_fn(move |hyper_req: Request<Incoming>| {
            let interceptor = Arc::clone(&interceptor);
            let storage = storage.as_ref().map(Arc::clone);
            let http_client = Arc::clone(&http_client);
            let target_host = target_host_string.clone();

            async move {
                Self::process_proxy_request(
                    hyper_req,
                    "https",
                    &target_host,
                    interceptor,
                    storage,
                    http_client,
                )
                .await
            }
        });

        hyper::server::conn::http1::Builder::new()
            .serve_connection(io, service)
            .await
            .map_err(|e| anyhow!("HTTPS proxy connection error: {:?}", e))?;

        Ok(())
    }

    async fn handle_plain_http(
        stream: TcpStream,
        interceptor: Arc<TrafficInterceptor>,
        storage: Option<Arc<WorkspaceStorage>>,
        http_client: Arc<HttpClient>,
        _client_addr: SocketAddr,
    ) -> Result<()> {
        let io = TokioIo::new(stream);
        let service = service_fn(move |hyper_req: Request<Incoming>| {
            let interceptor = Arc::clone(&interceptor);
            let storage = storage.as_ref().map(Arc::clone);
            let http_client = Arc::clone(&http_client);

            async move {
                let host = hyper_req
                    .headers()
                    .get("host")
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("localhost")
                    .to_string();

                Self::process_proxy_request(
                    hyper_req,
                    "http",
                    &host,
                    interceptor,
                    storage,
                    http_client,
                )
                .await
            }
        });

        hyper::server::conn::http1::Builder::new()
            .serve_connection(io, service)
            .await
            .map_err(|e| anyhow!("Plain HTTP proxy connection error: {:?}", e))?;

        Ok(())
    }

    async fn process_proxy_request(
        hyper_req: Request<Incoming>,
        scheme: &str,
        default_host: &str,
        interceptor: Arc<TrafficInterceptor>,
        storage: Option<Arc<WorkspaceStorage>>,
        http_client: Arc<HttpClient>,
    ) -> Result<Response<Full<Bytes>>, hyper::Error> {
        let method = hyper_req.method().as_str().to_string();
        let path_and_query = hyper_req
            .uri()
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/");

        let full_url = if hyper_req.uri().scheme().is_some() {
            hyper_req.uri().to_string()
        } else {
            format!("{}://{}{}", scheme, default_host, path_and_query)
        };

        let mut req_headers = HttpHeaders::new();
        for (k, v) in hyper_req.headers() {
            if let Ok(v_str) = v.to_str() {
                req_headers.append(k.as_str(), v_str);
            }
        }

        let body_bytes = match hyper_req.into_body().collect().await {
            Ok(collected) => collected.to_bytes().to_vec(),
            Err(_) => Vec::new(),
        };

        let mut app_req = HttpRequest::new(method, full_url);
        if let Some(req_id) = req_headers.get("x-request-id") {
            app_req.id = req_id.to_string();
        }
        app_req.headers = req_headers;
        app_req.body = body_bytes;

        // Process through Interceptor
        let final_req = match interceptor.process_request(app_req).await {
            Some(r) => r,
            None => {
                // Interceptor dropped the request
                return Ok(Response::builder()
                    .status(403)
                    .body(Full::new(Bytes::from(
                        "Request dropped by Excalibur Interceptor\r\n",
                    )))
                    .unwrap());
            }
        };

        // Dispatch upstream
        let upstream_res = match http_client.dispatch(&final_req).await {
            Ok(res) => res,
            Err(err) => {
                let err_msg = format!("Excalibur Gateway Error: {}\r\n", err);
                return Ok(Response::builder()
                    .status(502)
                    .body(Full::new(Bytes::from(err_msg)))
                    .unwrap());
            }
        };

        // Process response through Interceptor
        let final_res = match interceptor.process_response(upstream_res).await {
            Some(r) => r,
            None => {
                return Ok(Response::builder()
                    .status(403)
                    .body(Full::new(Bytes::from(
                        "Response dropped by Excalibur Interceptor\r\n",
                    )))
                    .unwrap());
            }
        };

        // Persist to storage if enabled
        if let Some(store) = storage {
            let _ = store.save_request(&final_req);
            let _ = store.save_response(&final_res);
        }

        // Build response to downstream client
        let mut builder = Response::builder().status(final_res.status);
        for (k, v) in final_res.headers.iter() {
            builder = builder.header(k, v);
        }

        let resp_body = Full::new(Bytes::from(final_res.body));
        Ok(builder.body(resp_body).unwrap_or_else(|_| {
            Response::builder()
                .status(500)
                .body(Full::new(Bytes::from("Internal Proxy Response Error")))
                .unwrap()
        }))
    }
}
