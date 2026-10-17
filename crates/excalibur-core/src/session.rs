use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub project_name: String,
    pub is_in_memory: bool,
    pub upstream_proxy: Option<String>,
    pub bypass_tls_validation: bool,
    pub ja3_fingerprint: Option<String>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            project_name: "default".to_string(),
            is_in_memory: false,
            upstream_proxy: None,
            bypass_tls_validation: false,
            ja3_fingerprint: None,
        }
    }
}
