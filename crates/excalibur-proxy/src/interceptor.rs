use excalibur_core::network::{HttpRequest, HttpResponse};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterceptAction {
    Forward,
    Drop,
    ModifyRequest(HttpRequest),
    ModifyResponse(HttpResponse),
}

pub struct TrafficInterceptor {
    pub is_enabled: bool,
}

impl TrafficInterceptor {
    pub fn new() -> Self {
        Self { is_enabled: false }
    }
}

impl Default for TrafficInterceptor {
    fn default() -> Self {
        Self::new()
    }
}
