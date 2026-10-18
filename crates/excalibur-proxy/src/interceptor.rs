use excalibur_core::network::{HttpRequest, HttpResponse};
use regex::Regex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;
use tokio::sync::{mpsc, oneshot};

/// Actions the user/UI can take on a paused intercepted packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterceptAction {
    Forward,
    Drop,
    ModifyRequest(HttpRequest),
    ModifyResponse(HttpResponse),
}

/// Filter rule matching against requests and responses.
#[derive(Debug, Clone)]
pub struct InterceptionFilter {
    pub url_regex: Option<Regex>,
    pub methods: Option<Vec<String>>,
    pub header_key: Option<String>,
}

impl InterceptionFilter {
    pub fn matches_request(&self, req: &HttpRequest) -> bool {
        if let Some(re) = &self.url_regex {
            if !re.is_match(&req.url) {
                return false;
            }
        }
        if let Some(methods) = &self.methods {
            if !methods.iter().any(|m| m.eq_ignore_ascii_case(&req.method)) {
                return false;
            }
        }
        if let Some(hdr) = &self.header_key {
            if !req.headers.contains_key(hdr) {
                return false;
            }
        }
        true
    }

    pub fn matches_response(&self, res: &HttpResponse) -> bool {
        if let Some(hdr) = &self.header_key {
            if !res.headers.contains_key(hdr) {
                return false;
            }
        }
        true
    }
}

/// Event sent to the UI when a packet is paused.
pub enum InterceptedEvent {
    Request {
        request: HttpRequest,
        responder: oneshot::Sender<InterceptAction>,
    },
    Response {
        response: HttpResponse,
        responder: oneshot::Sender<InterceptAction>,
    },
}

/// Real-time traffic interception engine.
pub struct TrafficInterceptor {
    pub enabled: AtomicBool,
    pub intercept_requests: AtomicBool,
    pub intercept_responses: AtomicBool,
    filters: RwLock<Vec<InterceptionFilter>>,
    event_tx: mpsc::Sender<InterceptedEvent>,
}

impl TrafficInterceptor {
    pub fn new(event_tx: mpsc::Sender<InterceptedEvent>) -> Self {
        Self {
            enabled: AtomicBool::new(false),
            intercept_requests: AtomicBool::new(true),
            intercept_responses: AtomicBool::new(true),
            filters: RwLock::new(Vec::new()),
            event_tx,
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    pub fn add_filter(&self, filter: InterceptionFilter) {
        if let Ok(mut filters) = self.filters.write() {
            filters.push(filter);
        }
    }

    pub fn clear_filters(&self) {
        if let Ok(mut filters) = self.filters.write() {
            filters.clear();
        }
    }

    pub fn should_intercept_request(&self, req: &HttpRequest) -> bool {
        if !self.is_enabled() || !self.intercept_requests.load(Ordering::SeqCst) {
            return false;
        }

        if let Ok(filters) = self.filters.read() {
            if filters.is_empty() {
                // Intercept all requests if enabled and no specific filters defined
                return true;
            }
            filters.iter().any(|f| f.matches_request(req))
        } else {
            false
        }
    }

    pub fn should_intercept_response(&self, res: &HttpResponse) -> bool {
        if !self.is_enabled() || !self.intercept_responses.load(Ordering::SeqCst) {
            return false;
        }

        if let Ok(filters) = self.filters.read() {
            if filters.is_empty() {
                return true;
            }
            filters.iter().any(|f| f.matches_response(res))
        } else {
            false
        }
    }

    /// Processes a request through the breakpoint engine, returning the final request (or None if dropped).
    pub async fn process_request(&self, req: HttpRequest) -> Option<HttpRequest> {
        if !self.should_intercept_request(&req) {
            return Some(req);
        }

        let (resp_tx, resp_rx) = oneshot::channel();
        let event = InterceptedEvent::Request {
            request: req.clone(),
            responder: resp_tx,
        };

        if self.event_tx.send(event).await.is_err() {
            // Receiver closed, fallback to forwarding
            return Some(req);
        }

        match resp_rx.await {
            Ok(InterceptAction::Forward) => Some(req),
            Ok(InterceptAction::ModifyRequest(modified)) => Some(modified),
            Ok(InterceptAction::Drop) => None,
            _ => Some(req),
        }
    }

    /// Processes a response through the breakpoint engine, returning the final response (or None if dropped).
    pub async fn process_response(&self, res: HttpResponse) -> Option<HttpResponse> {
        if !self.should_intercept_response(&res) {
            return Some(res);
        }

        let (resp_tx, resp_rx) = oneshot::channel();
        let event = InterceptedEvent::Response {
            response: res.clone(),
            responder: resp_tx,
        };

        if self.event_tx.send(event).await.is_err() {
            return Some(res);
        }

        match resp_rx.await {
            Ok(InterceptAction::Forward) => Some(res),
            Ok(InterceptAction::ModifyResponse(modified)) => Some(modified),
            Ok(InterceptAction::Drop) => None,
            _ => Some(res),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_interceptor_modify_request_flow() {
        let (tx, mut rx) = mpsc::channel(10);
        let interceptor = TrafficInterceptor::new(tx);
        interceptor.set_enabled(true);

        let req = HttpRequest::new("GET", "https://example.com/api/test");

        // Spawn mock UI handler responding with a modified request
        tokio::spawn(async move {
            if let Some(InterceptedEvent::Request {
                mut request,
                responder,
            }) = rx.recv().await
            {
                request.headers.append("X-Injected-Header", "AttackerValue");
                let _ = responder.send(InterceptAction::ModifyRequest(request));
            }
        });

        let final_req = interceptor.process_request(req).await;
        assert!(final_req.is_some());
        let final_req = final_req.unwrap();
        assert_eq!(
            final_req.headers.get("x-injected-header"),
            Some("AttackerValue")
        );
    }

    #[tokio::test]
    async fn test_interceptor_drop_flow() {
        let (tx, mut rx) = mpsc::channel(10);
        let interceptor = TrafficInterceptor::new(tx);
        interceptor.set_enabled(true);

        let req = HttpRequest::new("POST", "https://example.com/malicious");

        tokio::spawn(async move {
            if let Some(InterceptedEvent::Request { responder, .. }) = rx.recv().await {
                let _ = responder.send(InterceptAction::Drop);
            }
        });

        let final_req = interceptor.process_request(req).await;
        assert!(final_req.is_none());
    }
}
