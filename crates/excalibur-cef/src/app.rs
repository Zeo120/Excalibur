use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::Sender,
    Arc, Mutex,
};

use cef::{rc::Rc, *};
use excalibur_core::BrowserConfig;

use crate::client::CefEvent;

type WakeCallback = Arc<dyn Fn(i64) + Send + Sync>;

/// Shared state accessed by CEF application and process handlers.
pub struct SharedAppState {
    config: BrowserConfig,
    context_ready: AtomicBool,
    wake_callback: Mutex<Option<WakeCallback>>,
    event_tx: Sender<CefEvent>,
}

impl SharedAppState {
    pub fn new(config: BrowserConfig, event_tx: Sender<CefEvent>) -> Self {
        Self {
            config,
            context_ready: AtomicBool::new(false),
            wake_callback: Mutex::new(None),
            event_tx,
        }
    }

    pub fn set_wake_callback(&self, cb: WakeCallback) {
        if let Ok(mut guard) = self.wake_callback.lock() {
            *guard = Some(cb);
        }
    }

    pub fn is_context_ready(&self) -> bool {
        self.context_ready.load(Ordering::Acquire)
    }

    fn notify_context_initialized(&self) {
        self.context_ready.store(true, Ordering::Release);
        let _ = self.event_tx.send(CefEvent::ContextInitialized);
        self.wake(0);
    }

    fn wake(&self, delay_ms: i64) {
        if let Ok(guard) = self.wake_callback.lock() {
            if let Some(cb) = guard.as_ref() {
                cb(delay_ms);
            }
        }
    }
}

wrap_app! {
    pub struct ExcaliburApp {
        state: Arc<SharedAppState>,
    }

    impl App {
        fn on_before_command_line_processing(
            &self,
            process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            let Some(command_line) = command_line else {
                return;
            };
            let is_browser_process = process_type
                .map(CefString::to_string)
                .unwrap_or_default()
                .is_empty();
            if !is_browser_process {
                return;
            }

            // Strip background services and cap renderer proliferation to limit baseline RAM.
            for switch in [
                "disable-background-networking",
                "disable-sync",
                "disable-component-update",
                "disable-domain-reliability",
                "disable-breakpad",
                "no-pings",
                "disable-extensions",
                "disable-speech-api",
                "hide-crash-restore-bubble",
                "noerrdialogs",
            ] {
                command_line.append_switch(Some(&CefString::from(switch)));
            }

            let limit = self.state.config.renderer_process_limit.to_string();
            command_line.append_switch_with_value(
                Some(&CefString::from("renderer-process-limit")),
                Some(&CefString::from(limit.as_str())),
            );
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(ExcaliburBrowserProcessHandler::new(self.state.clone()))
        }
    }
}

wrap_browser_process_handler! {
    struct ExcaliburBrowserProcessHandler {
        state: Arc<SharedAppState>,
    }

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            self.state.notify_context_initialized();
        }

        fn on_schedule_message_pump_work(&self, delay_ms: i64) {
            self.state.wake(delay_ms);
        }
    }
}
