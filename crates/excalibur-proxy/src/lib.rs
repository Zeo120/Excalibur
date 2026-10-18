//! `excalibur-proxy`: Inbound MITM proxy server, dynamic CA generation, and traffic interception.

pub mod cert;
pub mod interceptor;
pub mod server;

pub use cert::CertificateAuthority;
pub use interceptor::{InterceptAction, InterceptedEvent, InterceptionFilter, TrafficInterceptor};
pub use server::ProxyServer;
