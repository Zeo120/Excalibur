//! `excalibur-proxy`: Inbound MITM proxy server, dynamic CA generation, and traffic interception.

pub mod cert;
pub mod interceptor;
pub mod server;

pub use cert::CertificateAuthority;
pub use interceptor::TrafficInterceptor;
pub use server::ProxyServer;
