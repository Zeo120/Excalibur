use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, Error as RustlsError, SignatureScheme};
use std::sync::Arc;

/// TLS Configuration presets for fingerprinting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ja3Preset {
    #[default]
    ModernStandard,
    Chrome,
    Firefox,
    Safari,
}

/// Custom Server Certificate Verifier that can bypass verification for pentesting.
#[derive(Debug)]
pub struct PermissiveCertVerifier {
    pub bypass_verification: bool,
    default_verifier: Arc<dyn ServerCertVerifier>,
}

pub fn ensure_crypto_provider() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
}

impl PermissiveCertVerifier {
    pub fn new(bypass_verification: bool) -> Self {
        ensure_crypto_provider();
        let mut root_store = rustls::RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

        let default_verifier = rustls::client::WebPkiServerVerifier::builder_with_provider(
            Arc::new(root_store),
            Arc::new(rustls::crypto::aws_lc_rs::default_provider()),
        )
        .build()
        .expect("Failed to build WebPkiServerVerifier");

        Self {
            bypass_verification,
            default_verifier,
        }
    }
}

impl ServerCertVerifier for PermissiveCertVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, RustlsError> {
        if self.bypass_verification {
            tracing::warn!(
                "TLS certificate verification bypassed for server: {:?}",
                server_name
            );
            Ok(ServerCertVerified::assertion())
        } else {
            self.default_verifier.verify_server_cert(
                end_entity,
                intermediates,
                server_name,
                ocsp_response,
                now,
            )
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        if self.bypass_verification {
            Ok(HandshakeSignatureValid::assertion())
        } else {
            self.default_verifier
                .verify_tls12_signature(message, cert, dss)
        }
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        if self.bypass_verification {
            Ok(HandshakeSignatureValid::assertion())
        } else {
            self.default_verifier
                .verify_tls13_signature(message, cert, dss)
        }
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.default_verifier.supported_verify_schemes()
    }
}

/// TLS Client Builder supporting custom ciphers, ALPN, and JA3 presets.
pub struct TlsConfigBuilder {
    bypass_verification: bool,
    ja3_preset: Ja3Preset,
    alpn_protocols: Vec<Vec<u8>>,
}

impl TlsConfigBuilder {
    pub fn new() -> Self {
        Self {
            bypass_verification: false,
            ja3_preset: Ja3Preset::ModernStandard,
            alpn_protocols: vec![b"http/1.1".to_vec()],
        }
    }

    pub fn bypass_verification(mut self, bypass: bool) -> Self {
        self.bypass_verification = bypass;
        self
    }

    pub fn ja3_preset(mut self, preset: Ja3Preset) -> Self {
        self.ja3_preset = preset;
        self
    }

    pub fn alpn_protocols(mut self, protocols: Vec<Vec<u8>>) -> Self {
        self.alpn_protocols = protocols;
        self
    }

    pub fn build(self) -> Result<ClientConfig, RustlsError> {
        let verifier = Arc::new(PermissiveCertVerifier::new(self.bypass_verification));

        let mut config = ClientConfig::builder_with_provider(
            rustls::crypto::aws_lc_rs::default_provider().into(),
        )
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();

        config.alpn_protocols = self.alpn_protocols;
        Ok(config)
    }
}

impl Default for TlsConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tls_config_builder() {
        let builder = TlsConfigBuilder::new()
            .bypass_verification(true)
            .ja3_preset(Ja3Preset::Chrome);

        let config = builder.build();
        assert!(config.is_ok());
        let config = config.unwrap();
        assert_eq!(config.alpn_protocols, vec![b"http/1.1".to_vec()]);
    }
}
