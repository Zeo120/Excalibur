use std::sync::{mpsc::Sender, Arc, Mutex};

use cef::{rc::Rc, *};
use excalibur_core::DomainBlocker;

/// Events emitted from CEF callbacks to the host UI.
#[derive(Debug, Clone)]
pub enum CefEvent {
    ContextInitialized,
    BrowserCreated {
        tab_id: u64,
    },
    TitleChanged {
        tab_id: u64,
        title: String,
    },
    AddressChanged {
        tab_id: u64,
        url: String,
    },
    LoadingStateChanged {
        tab_id: u64,
        is_loading: bool,
        can_go_back: bool,
        can_go_forward: bool,
    },
    RequestBlocked {
        tab_id: u64,
        host: String,
    },
    BrowserClosed {
        tab_id: u64,
    },
}

/// Shared handle to a single tab's underlying CEF browser.
#[derive(Clone)]
pub struct ExcaliburClientHandle {
    tab_id: u64,
    browser: Arc<Mutex<Option<Browser>>>,
}

impl ExcaliburClientHandle {
    pub fn new(tab_id: u64) -> Self {
        Self {
            tab_id,
            browser: Arc::new(Mutex::new(None)),
        }
    }

    pub fn tab_id(&self) -> u64 {
        self.tab_id
    }

    pub fn browser(&self) -> Option<Browser> {
        self.browser.lock().ok().and_then(|b| b.clone())
    }

    fn set_browser(&self, browser: Option<Browser>) {
        if let Ok(mut guard) = self.browser.lock() {
            *guard = browser;
        }
    }
}

struct ClientContext {
    tab_id: u64,
    handle: ExcaliburClientHandle,
    blocker: Arc<DomainBlocker>,
    event_tx: Sender<CefEvent>,
}

pub fn build_client(
    tab_id: u64,
    handle: ExcaliburClientHandle,
    blocker: Arc<DomainBlocker>,
    event_tx: Sender<CefEvent>,
) -> Client {
    let ctx = Arc::new(ClientContext {
        tab_id,
        handle,
        blocker,
        event_tx,
    });
    ExcaliburClient::new(ctx)
}

wrap_client! {
    struct ExcaliburClient {
        ctx: Arc<ClientContext>,
    }

    impl Client {
        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(ExcaliburDisplayHandler::new(self.ctx.clone()))
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(ExcaliburLifeSpanHandler::new(self.ctx.clone()))
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(ExcaliburLoadHandler::new(self.ctx.clone()))
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(ExcaliburRequestHandler::new(self.ctx.clone()))
        }
    }
}

wrap_display_handler! {
    struct ExcaliburDisplayHandler {
        ctx: Arc<ClientContext>,
    }

    impl DisplayHandler {
        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            let title = title.map(CefString::to_string).unwrap_or_default();
            let _ = self.ctx.event_tx.send(CefEvent::TitleChanged {
                tab_id: self.ctx.tab_id,
                title,
            });
        }

        fn on_address_change(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            url: Option<&CefString>,
        ) {
            let is_main = frame.is_some_and(|f| cef::ImplFrame::is_main(f) != 0);
            if !is_main {
                return;
            }
            let url = url.map(CefString::to_string).unwrap_or_default();
            let _ = self.ctx.event_tx.send(CefEvent::AddressChanged {
                tab_id: self.ctx.tab_id,
                url,
            });
        }
    }
}

wrap_life_span_handler! {
    struct ExcaliburLifeSpanHandler {
        ctx: Arc<ClientContext>,
    }

    impl LifeSpanHandler {
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            self.ctx.handle.set_browser(browser.cloned());
            let _ = self.ctx.event_tx.send(CefEvent::BrowserCreated {
                tab_id: self.ctx.tab_id,
            });
        }

        fn on_before_close(&self, _browser: Option<&mut Browser>) {
            self.ctx.handle.set_browser(None);
            let _ = self.ctx.event_tx.send(CefEvent::BrowserClosed {
                tab_id: self.ctx.tab_id,
            });
        }
    }
}

wrap_load_handler! {
    struct ExcaliburLoadHandler {
        ctx: Arc<ClientContext>,
    }

    impl LoadHandler {
        fn on_loading_state_change(
            &self,
            _browser: Option<&mut Browser>,
            is_loading: ::std::os::raw::c_int,
            can_go_back: ::std::os::raw::c_int,
            can_go_forward: ::std::os::raw::c_int,
        ) {
            let _ = self.ctx.event_tx.send(CefEvent::LoadingStateChanged {
                tab_id: self.ctx.tab_id,
                is_loading: is_loading != 0,
                can_go_back: can_go_back != 0,
                can_go_forward: can_go_forward != 0,
            });
        }
    }
}

wrap_request_handler! {
    struct ExcaliburRequestHandler {
        ctx: Arc<ClientContext>,
    }

    impl RequestHandler {
        fn resource_request_handler(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _request: Option<&mut Request>,
            is_navigation: ::std::os::raw::c_int,
            _is_download: ::std::os::raw::c_int,
            _request_initiator: Option<&CefString>,
            _disable_default_handling: Option<&mut ::std::os::raw::c_int>,
        ) -> Option<ResourceRequestHandler> {
            let is_main_nav =
                is_navigation != 0 && frame.is_some_and(|f| cef::ImplFrame::is_main(f) != 0);
            if is_main_nav {
                return None;
            }
            Some(ExcaliburResourceRequestHandler::new(self.ctx.clone()))
        }
    }
}

wrap_resource_request_handler! {
    struct ExcaliburResourceRequestHandler {
        ctx: Arc<ClientContext>,
    }

    impl ResourceRequestHandler {
        fn on_before_resource_load(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _callback: Option<&mut Callback>,
        ) -> ReturnValue {
            let Some(request) = request else {
                return ReturnValue::CONTINUE;
            };
            let url = CefString::from(&request.url()).to_string();
            if let Some(host) = self.ctx.blocker.should_block_url(&url) {
                let _ = self.ctx.event_tx.send(CefEvent::RequestBlocked {
                    tab_id: self.ctx.tab_id,
                    host,
                });
                return ReturnValue::CANCEL;
            }
            ReturnValue::CONTINUE
        }
    }
}
