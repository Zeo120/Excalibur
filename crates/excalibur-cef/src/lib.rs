pub mod app;
pub mod client;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use cef::{
    api_hash, args::Args, browser_host_create_browser, do_message_loop_work, execute_process,
    initialize, shutdown, sys, Browser, BrowserSettings, CefString, ImplBrowser, ImplBrowserHost,
    ImplCommandLine, ImplFrame, Rect, Settings, WindowInfo,
};
use excalibur_core::BrowserConfig;
use raw_window_handle::RawWindowHandle;

use app::{ExcaliburApp, SharedAppState};
pub use client::{CefEvent, ExcaliburClientHandle};

#[cfg(target_os = "macos")]
pub type CefLibraryGuard = cef::library_loader::LibraryLoader;
#[cfg(not(target_os = "macos"))]
pub struct CefLibraryGuard;

/// Controls CEF initialization, message pumping, and browser instances.
pub struct CefRuntime {
    _library_guard: CefLibraryGuard,
    shared_state: Arc<SharedAppState>,
    event_rx: std::sync::mpsc::Receiver<CefEvent>,
    event_tx: std::sync::mpsc::Sender<CefEvent>,
    browsers: Arc<Mutex<Vec<ExcaliburClientHandle>>>,
    shutdown_done: AtomicBool,
}

/// Result of early process startup.
pub enum StartupOutcome {
    /// Current process was a Chromium helper sub-process and has finished.
    SubprocessExited(i32),
    /// Current process is the main browser process and CEF is initialized.
    BrowserInitialized(CefRuntime),
}

/// Initializes the CEF API, handles helper sub-processes, or boots the main browser process.
pub fn bootstrap_cef(config: &BrowserConfig) -> Result<StartupOutcome, String> {
    #[cfg(target_os = "macos")]
    let library_guard = {
        let exe = std::env::current_exe().map_err(|e| format!("current_exe failed: {e}"))?;
        let loader = cef::library_loader::LibraryLoader::new(&exe, false);
        if !loader.load() {
            return Err("Failed to load Chromium Embedded Framework library".to_owned());
        }
        loader
    };
    #[cfg(not(target_os = "macos"))]
    let library_guard = CefLibraryGuard;

    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);

    let args = Args::new();
    let Some(cmd_line) = args.as_cmd_line() else {
        return Err("Failed to parse CEF command line arguments".to_owned());
    };

    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let shared_state = Arc::new(SharedAppState::new(config.clone(), event_tx.clone()));
    let mut app = ExcaliburApp::new(shared_state.clone());

    let type_switch = CefString::from("type");
    let is_browser_process = cmd_line.has_switch(Some(&type_switch)) != 1;

    let sub_ret = execute_process(
        Some(args.as_main_args()),
        Some(&mut app),
        std::ptr::null_mut(),
    );
    if !is_browser_process {
        return Ok(StartupOutcome::SubprocessExited(sub_ret.max(0)));
    }
    if sub_ret >= 0 {
        return Ok(StartupOutcome::SubprocessExited(sub_ret));
    }

    let cache_dir = std::env::temp_dir().join("excalibur_cef_cache");
    let _ = std::fs::create_dir_all(&cache_dir);
    let root_cache_path = CefString::from(cache_dir.to_string_lossy().as_ref());

    let settings = Settings {
        no_sandbox: 1,
        external_message_pump: 1,
        persist_session_cookies: 0,
        root_cache_path,
        log_severity: cef::LogSeverity::WARNING,
        ..Default::default()
    };

    let init_ok = initialize(
        Some(args.as_main_args()),
        Some(&settings),
        Some(&mut app),
        std::ptr::null_mut(),
    );
    if init_ok != 1 {
        return Err("cef::initialize failed".to_owned());
    }

    Ok(StartupOutcome::BrowserInitialized(CefRuntime {
        _library_guard: library_guard,
        shared_state,
        event_rx,
        event_tx,
        browsers: Arc::new(Mutex::new(Vec::new())),
        shutdown_done: AtomicBool::new(false),
    }))
}

impl CefRuntime {
    /// Registers a callback invoked when CEF schedules message-pump work.
    pub fn set_wake_callback(&self, cb: Arc<dyn Fn(i64) + Send + Sync>) {
        self.shared_state.set_wake_callback(cb);
    }

    /// Returns `true` once CEF has fired `on_context_initialized`.
    pub fn is_context_ready(&self) -> bool {
        self.shared_state.is_context_ready()
    }

    /// Advances CEF's message loop work on the UI thread.
    pub fn pump_work(&self) {
        if !self.shutdown_done.load(Ordering::Acquire) {
            do_message_loop_work();
        }
    }

    /// Drains pending CEF events without blocking.
    pub fn poll_events(&self) -> Vec<CefEvent> {
        let mut events = Vec::new();
        while let Ok(ev) = self.event_rx.try_recv() {
            events.push(ev);
        }
        events
    }

    /// Creates a child Chromium browser embedded inside `parent` at physical `bounds`.
    pub fn create_child_browser(
        &self,
        tab_id: u64,
        parent: RawWindowHandle,
        bounds: (i32, i32, i32, i32),
        url: &str,
    ) -> Result<ExcaliburClientHandle, String> {
        let parent_handle = raw_handle_to_cef(parent)?;
        let handle = ExcaliburClientHandle::new(tab_id);
        let mut client = client::build_client(tab_id, handle.clone(), self.event_tx.clone());

        let cef_rect = Rect {
            x: bounds.0,
            y: bounds.1,
            width: bounds.2.max(1),
            height: bounds.3.max(1),
        };
        let window_info = WindowInfo::default().set_as_child(parent_handle, &cef_rect);
        let browser_settings = BrowserSettings::default();
        let url_cef = CefString::from(url);

        let created = browser_host_create_browser(
            Some(&window_info),
            Some(&mut client),
            Some(&url_cef),
            Some(&browser_settings),
            None,
            None,
        );
        if created != 1 {
            return Err("browser_host_create_browser returned 0".to_owned());
        }

        if let Ok(mut list) = self.browsers.lock() {
            list.push(handle.clone());
        }
        Ok(handle)
    }

    /// Navigates the browser associated with `tab_id` to `url`.
    pub fn navigate(&self, tab_id: u64, url: &str) {
        self.with_browser(tab_id, |browser| {
            if let Some(frame) = browser.main_frame() {
                let target = CefString::from(url);
                frame.load_url(Some(&target));
            }
        });
    }

    /// Navigates backward in history for `tab_id`.
    pub fn go_back(&self, tab_id: u64) {
        self.with_browser(tab_id, |browser| browser.go_back());
    }

    /// Navigates forward in history for `tab_id`.
    pub fn go_forward(&self, tab_id: u64) {
        self.with_browser(tab_id, |browser| browser.go_forward());
    }

    /// Reloads `tab_id`.
    pub fn reload(&self, tab_id: u64) {
        self.with_browser(tab_id, |browser| browser.reload());
    }

    /// Stops loading `tab_id`.
    pub fn stop_load(&self, tab_id: u64) {
        self.with_browser(tab_id, |browser| browser.stop_load());
    }

    /// Resizes and repositions the child browser viewport for `tab_id`.
    pub fn resize_browser(&self, tab_id: u64, x: i32, y: i32, width: i32, height: i32) {
        self.with_browser(tab_id, |browser| {
            let Some(host) = browser.host() else {
                return;
            };
            resize_native_child(&host, x, y, width.max(1), height.max(1));
            host.notify_move_or_resize_started();
            host.was_resized();
        });
    }

    /// Closes all active browsers and shuts down CEF cleanly.
    pub fn shutdown_cleanly(&self) {
        if self.shutdown_done.swap(true, Ordering::AcqRel) {
            return;
        }

        let handles: Vec<ExcaliburClientHandle> = self
            .browsers
            .lock()
            .map(|list| list.clone())
            .unwrap_or_default();

        for handle in &handles {
            if let Some(browser) = handle.browser() {
                if let Some(host) = browser.host() {
                    host.close_browser(1);
                }
            }
        }

        for _ in 0..200 {
            do_message_loop_work();
            let all_closed = handles.iter().all(|h| h.browser().is_none());
            if all_closed {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        if let Ok(mut list) = self.browsers.lock() {
            list.clear();
        }
        drop(handles);

        shutdown();
    }

    fn with_browser(&self, tab_id: u64, f: impl FnOnce(Browser)) {
        let browser = self.browsers.lock().ok().and_then(|list| {
            list.iter()
                .find(|h| h.tab_id() == tab_id)
                .and_then(|h| h.browser())
        });
        if let Some(browser) = browser {
            f(browser);
        }
    }
}

impl Drop for CefRuntime {
    fn drop(&mut self) {
        self.shutdown_cleanly();
    }
}

fn raw_handle_to_cef(handle: RawWindowHandle) -> Result<sys::cef_window_handle_t, String> {
    match handle {
        #[cfg(target_os = "windows")]
        RawWindowHandle::Win32(win32) => Ok(sys::HWND(
            std::ptr::without_provenance_mut::<std::ffi::c_void>(win32.hwnd.get() as usize).cast(),
        )),
        #[cfg(target_os = "macos")]
        RawWindowHandle::AppKit(appkit) => Ok(appkit.ns_view.as_ptr() as sys::cef_window_handle_t),
        #[cfg(target_os = "linux")]
        RawWindowHandle::Xlib(xlib) => Ok(xlib.window as sys::cef_window_handle_t),
        _ => Err("Unsupported native window handle for CEF child embedding".to_owned()),
    }
}

#[cfg(target_os = "windows")]
fn resize_native_child(host: &cef::BrowserHost, x: i32, y: i32, width: i32, height: i32) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER};
    let hwnd = host.window_handle().0;
    if !hwnd.is_null() {
        unsafe {
            SetWindowPos(
                hwnd as _,
                std::ptr::null_mut(),
                x,
                y,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn resize_native_child(_host: &cef::BrowserHost, _x: i32, _y: i32, _width: i32, _height: i32) {}
