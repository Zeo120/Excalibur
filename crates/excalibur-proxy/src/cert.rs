use anyhow::Result;

pub struct CertificateAuthority {
    pub ca_cert_pem: String,
}

impl CertificateAuthority {
    pub fn new_ephemeral() -> Result<Self> {
        Ok(Self {
            ca_cert_pem: String::new(),
        })
    }
}
