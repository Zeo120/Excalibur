use std::sync::{mpsc::Sender, Arc, Mutex};

use cef::{rc::Rc, *};
use excalibur_core::DomainBlocker;

/// Browser-level keyboard shortcuts intercepted before the webview consumes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserShortcut {
    NewTab,
    CloseTab,
    FocusOmnibox,
    ToggleBookmark,
    ToggleBookmarksDrawer,
    ToggleHistoryDrawer,
    Reload,
    NextTab,
    PrevTab,
}

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
    OpenPopup {
        url: String,
    },
    Shortcut(BrowserShortcut),
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

        fn keyboard_handler(&self) -> Option<KeyboardHandler> {
            Some(ExcaliburKeyboardHandler::new(self.ctx.clone()))
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
        fn on_before_popup(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _popup_id: ::std::os::raw::c_int,
            target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: ::std::os::raw::c_int,
            _popup_features: Option<&PopupFeatures>,
            _window_info: Option<&mut WindowInfo>,
            _client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            if let Some(url) = target_url.map(CefString::to_string) {
                if !url.is_empty() {
                    let _ = self.ctx.event_tx.send(CefEvent::OpenPopup { url });
                }
            }
            // Cancel Chromium's standalone popup window so Excalibur opens it as a managed tab.
            1
        }

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

wrap_keyboard_handler! {
    struct ExcaliburKeyboardHandler {
        ctx: Arc<ClientContext>,
    }

    impl KeyboardHandler {
        fn on_pre_key_event(
            &self,
            _browser: Option<&mut Browser>,
            event: Option<&KeyEvent>,
            _os_event: Option<&mut sys::MSG>,
            is_keyboard_shortcut: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            let Some(event) = event else {
                return 0;
            };
            if event.type_ != KeyEventType::RAWKEYDOWN && event.type_ != KeyEventType::KEYDOWN {
                return 0;
            }

            let ctrl_flag = sys::cef_event_flags_t::CEF_EVENTFLAG_CONTROL_DOWN.0 as u32;
            let cmd_flag = sys::cef_event_flags_t::CEF_EVENTFLAG_COMMAND_DOWN.0 as u32;
            let shift_flag = sys::cef_event_flags_t::CEF_EVENTFLAG_SHIFT_DOWN.0 as u32;
            let alt_flag = sys::cef_event_flags_t::CEF_EVENTFLAG_ALT_DOWN.0 as u32;

            let cmd = (event.modifiers & (ctrl_flag | cmd_flag)) != 0
                && (event.modifiers & alt_flag) == 0;
            let shift = (event.modifiers & shift_flag) != 0;

            let shortcut = if event.windows_key_code == 0x74 {
                Some(BrowserShortcut::Reload)
            } else if cmd {
                match event.windows_key_code {
                    0x54 => Some(BrowserShortcut::NewTab),
                    0x57 => Some(BrowserShortcut::CloseTab),
                    0x4C => Some(BrowserShortcut::FocusOmnibox),
                    0x44 => Some(BrowserShortcut::ToggleBookmark),
                    0x42 => Some(BrowserShortcut::ToggleBookmarksDrawer),
                    0x48 => Some(BrowserShortcut::ToggleHistoryDrawer),
                    0x52 => Some(BrowserShortcut::Reload),
                    0x09 if shift => Some(BrowserShortcut::PrevTab),
                    0x09 => Some(BrowserShortcut::NextTab),
                    _ => None,
                }
            } else {
                None
            };

            if let Some(shortcut) = shortcut {
                if let Some(flag) = is_keyboard_shortcut {
                    *flag = 1;
                }
                let _ = self.ctx.event_tx.send(CefEvent::Shortcut(shortcut));
                return 1;
            }
            0
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
