use anyhow::Result;
use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, KeyPair, PKCS_ECDSA_P256_SHA256};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

fn clone_cert_pair(
    pair: &(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>),
) -> (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>) {
    (pair.0.clone(), pair.1.clone_key())
}

/// Dynamic Certificate Authority capable of forging leaf certificates for intercepted domains.
pub struct CertificateAuthority {
    pub ca_cert_pem: String,
    pub ca_cert_der: Vec<u8>,
    ca_key: Arc<KeyPair>,
    ca_params: CertificateParams,
    cert_cache: RwLock<HashMap<String, (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)>>,
}

impl CertificateAuthority {
    /// Generates a new ephemeral Root CA.
    pub fn new_ephemeral() -> Result<Self> {
        let mut ca_params = CertificateParams::default();
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca_params
            .distinguished_name
            .push(DnType::CommonName, "Excalibur MITM Proxy CA");
        ca_params
            .distinguished_name
            .push(DnType::OrganizationName, "Excalibur Security");

        let ca_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
        let ca_cert = ca_params.clone().self_signed(&ca_key)?;
        let ca_cert_pem = ca_cert.pem();
        let ca_cert_der = ca_cert.der().to_vec();

        Ok(Self {
            ca_cert_pem,
            ca_cert_der,
            ca_key: Arc::new(ca_key),
            ca_params,
            cert_cache: RwLock::new(HashMap::new()),
        })
    }

    /// Forges a TLS certificate for the specified domain, signed by this CA.
    pub fn get_or_create_leaf_cert(
        &self,
        domain: &str,
    ) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
        // Check cache first
        if let Ok(cache) = self.cert_cache.read() {
            if let Some(cached) = cache.get(domain) {
                return Ok(clone_cert_pair(cached));
            }
        }

        let mut leaf_params = CertificateParams::new(vec![domain.to_string()])?;
        leaf_params
            .distinguished_name
            .push(DnType::CommonName, domain);

        let leaf_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
        let ca_cert = self.ca_params.clone().self_signed(&self.ca_key)?;
        let leaf_cert = leaf_params.signed_by(&leaf_key, &ca_cert, &self.ca_key)?;

        let cert_der = CertificateDer::from(leaf_cert.der().to_vec());
        let key_der = PrivateKeyDer::try_from(leaf_key.serialize_der())
            .map_err(|e| anyhow::anyhow!("Failed to serialize private key: {:?}", e))?;

        let cert_chain = vec![cert_der];
        let pair = (cert_chain, key_der);

        // Cache the newly created certificate
        if let Ok(mut cache) = self.cert_cache.write() {
            cache.insert(domain.to_string(), clone_cert_pair(&pair));
        }

        Ok(pair)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_certificate_generation() {
        let ca = CertificateAuthority::new_ephemeral().expect("Failed to create CA");
        assert!(!ca.ca_cert_pem.is_empty());

        let (chain, _key) = ca
            .get_or_create_leaf_cert("example.com")
            .expect("Failed to generate leaf cert");
        assert_eq!(chain.len(), 1);

        // Second lookup should return cached certificate
        let (cached_chain, _) = ca
            .get_or_create_leaf_cert("example.com")
            .expect("Failed to get cached cert");
        assert_eq!(chain[0].as_ref(), cached_chain[0].as_ref());
    }
}
