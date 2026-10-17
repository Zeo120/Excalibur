use anyhow::Result;
use std::net::SocketAddr;

pub struct ProxyServer {
    pub bind_addr: SocketAddr,
}

impl ProxyServer {
    pub fn new(bind_addr: SocketAddr) -> Self {
        Self { bind_addr }
    }

    pub async fn run(&self) -> Result<()> {
        tracing::info!("Starting Excalibur MITM Proxy on {}", self.bind_addr);
        Ok(())
    }
}
