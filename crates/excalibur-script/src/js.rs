use anyhow::{Context as AnyhowContext, Result};
use rquickjs::{Context, Function, Runtime, Value};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Resource limits applied to every [`JsRuntime::eval`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionLimits {
    /// Wall-clock budget for a single `eval`; QuickJS is interrupted when it elapses.
    pub time_budget: Duration,
    /// Maximum heap the runtime may allocate.
    pub memory_limit_bytes: usize,
    /// Maximum native stack QuickJS may use (guards runaway recursion).
    pub max_stack_bytes: usize,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            time_budget: Duration::from_secs(2),
            memory_limit_bytes: 64 * 1024 * 1024,
            max_stack_bytes: 512 * 1024,
        }
    }
}

/// Why QuickJS execution was stopped by the interrupt handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum JsInterrupt {
    #[error("script exceeded its execution time budget")]
    TimedOut,
    #[error("script execution was cancelled")]
    Cancelled,
}

/// Thread-safe handle that stops a running (or any future) evaluation on its runtime.
#[derive(Debug, Clone, Default)]
pub struct JsCancelHandle(Arc<AtomicBool>);

impl JsCancelHandle {
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation; the running script is interrupted at its next check.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Severity or category of a sink event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SinkFinding {
    pub sink_name: String,
    pub argument: String,
    pub timestamp: u64,
}

/// Captured console message from the JavaScript environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsoleMessage {
    pub level: String,
    pub message: String,
}

/// Sandboxed JavaScript runtime powered by QuickJS with DOM mocks and XSS sink tracing.
pub struct JsRuntime {
    _runtime: Runtime,
    context: Context,
    sink_findings: Arc<Mutex<Vec<SinkFinding>>>,
    console_logs: Arc<Mutex<Vec<ConsoleMessage>>>,
    limits: ExecutionLimits,
    cancel: JsCancelHandle,
    deadline: Arc<Mutex<Option<Instant>>>,
    interrupted: Arc<Mutex<Option<JsInterrupt>>>,
}

impl JsRuntime {
    /// Creates and initializes a new JavaScript runtime with DOM mocks and sink hooks,
    /// using [`ExecutionLimits::default`] and a fresh cancel handle.
    pub fn new() -> Result<Self> {
        Self::with_limits(ExecutionLimits::default(), JsCancelHandle::new())
    }

    /// Creates a runtime whose evaluations are bounded by `limits` and can be stopped
    /// from any thread through `cancel`.
    pub fn with_limits(limits: ExecutionLimits, cancel: JsCancelHandle) -> Result<Self> {
        let runtime = Runtime::new().context("Failed to create QuickJS runtime")?;
        let context = Context::full(&runtime).context("Failed to create QuickJS context")?;

        let deadline: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
        let interrupted: Arc<Mutex<Option<JsInterrupt>>> = Arc::new(Mutex::new(None));
        {
            let cancel = cancel.clone();
            let deadline = Arc::clone(&deadline);
            let interrupted = Arc::clone(&interrupted);
            // QuickJS polls this periodically while executing; returning `true` throws an
            // uncatchable interrupt exception, unwinding the script on this thread.
            runtime.set_interrupt_handler(Some(Box::new(move || {
                let reason = if cancel.is_cancelled() {
                    Some(JsInterrupt::Cancelled)
                } else if deadline
                    .lock()
                    .map(|d| d.is_some_and(|d| Instant::now() >= d))
                    .unwrap_or(false)
                {
                    Some(JsInterrupt::TimedOut)
                } else {
                    None
                };
                if let Some(r) = reason {
                    if let Ok(mut slot) = interrupted.lock() {
                        *slot = Some(r);
                    }
                    true
                } else {
                    false
                }
            })));
        }

        let instance = Self {
            _runtime: runtime,
            context,
            sink_findings: Arc::new(Mutex::new(Vec::new())),
            console_logs: Arc::new(Mutex::new(Vec::new())),
            limits,
            cancel,
            deadline,
            interrupted,
        };

        instance.init_environment()?;
        // Apply heap and stack caps after the trusted preamble is installed.
        instance
            ._runtime
            .set_memory_limit(limits.memory_limit_bytes);
        instance._runtime.set_max_stack_size(limits.max_stack_bytes);
        Ok(instance)
    }

    /// Handle that cancels evaluations on this runtime from another thread.
    pub fn cancel_handle(&self) -> JsCancelHandle {
        self.cancel.clone()
    }

    pub fn limits(&self) -> ExecutionLimits {
        self.limits
    }

    fn init_environment(&self) -> Result<()> {
        let sinks_clone = Arc::clone(&self.sink_findings);
        let logs_clone = Arc::clone(&self.console_logs);

        self.context.with(|ctx| -> rquickjs::Result<()> {
            let globals = ctx.globals();

            // Native sink interception callback
            let sinks = sinks_clone.clone();
            let sink_hook = Function::new(ctx.clone(), move |sink: String, arg: String| {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                sinks.lock().unwrap().push(SinkFinding {
                    sink_name: sink,
                    argument: arg,
                    timestamp: now,
                });
            })?;
            globals.set("__native_sink_hit", sink_hook)?;

            // Native console logger callback
            let logs = logs_clone.clone();
            let console_hook = Function::new(ctx.clone(), move |level: String, msg: String| {
                logs.lock().unwrap().push(ConsoleMessage {
                    level,
                    message: msg,
                });
            })?;
            globals.set("__native_console_msg", console_hook)?;

            // Preamble JavaScript to set up window, document, location, console, and hook sinks
            let preamble = r#"
                (function() {
                    const global = globalThis;
                    global.window = global;

                    // Console mock
                    global.console = {
                        log: (...args) => __native_console_msg("log", args.map(String).join(" ")),
                        info: (...args) => __native_console_msg("info", args.map(String).join(" ")),
                        warn: (...args) => __native_console_msg("warn", args.map(String).join(" ")),
                        error: (...args) => __native_console_msg("error", args.map(String).join(" ")),
                        debug: (...args) => __native_console_msg("debug", args.map(String).join(" "))
                    };

                    // Location mock
                    global.location = {
                        href: "about:blank",
                        origin: "null",
                        protocol: "about:",
                        host: "",
                        hostname: "",
                        port: "",
                        pathname: "blank",
                        search: "",
                        hash: "",
                        assign: function(url) { this.href = url; },
                        replace: function(url) { this.href = url; }
                    };

                    // DOM mock with element factory
                    function createMockElement(tag) {
                        let innerHtmlVal = "";
                        let textVal = "";
                        const attrs = {};
                        const el = {
                            tagName: String(tag).toUpperCase(),
                            attributes: attrs,
                            getAttribute: (k) => attrs[k] || null,
                            setAttribute: (k, v) => { attrs[k] = String(v); },
                            children: []
                        };
                        Object.defineProperty(el, "innerHTML", {
                            get: () => innerHtmlVal,
                            set: (val) => {
                                innerHtmlVal = String(val);
                                __native_sink_hit("Element.innerHTML", innerHtmlVal);
                            }
                        });
                        Object.defineProperty(el, "outerHTML", {
                            get: () => innerHtmlVal,
                            set: (val) => {
                                innerHtmlVal = String(val);
                                __native_sink_hit("Element.outerHTML", innerHtmlVal);
                            }
                        });
                        Object.defineProperty(el, "textContent", {
                            get: () => textVal,
                            set: (val) => { textVal = String(val); }
                        });
                        return el;
                    }

                    global.document = {
                        location: global.location,
                        title: "",
                        cookie: "",
                        body: createMockElement("BODY"),
                        documentElement: createMockElement("HTML"),
                        head: createMockElement("HEAD"),
                        getElementById: (id) => {
                            const el = createMockElement("DIV");
                            el.id = String(id);
                            return el;
                        },
                        getElementsByTagName: (tag) => [createMockElement(tag)],
                        querySelector: (sel) => createMockElement("DIV"),
                        querySelectorAll: (sel) => [createMockElement("DIV")],
                        createElement: (tag) => createMockElement(tag),
                        write: (content) => {
                            __native_sink_hit("document.write", String(content));
                        },
                        writeln: (content) => {
                            __native_sink_hit("document.writeln", String(content));
                        }
                    };

                    // XSS Sink Hooks: eval()
                    const _origEval = global.eval;
                    global.eval = function(code) {
                        __native_sink_hit("eval", String(code));
                        return _origEval(code);
                    };

                    // Sink: Function() constructor
                    const _origFunction = global.Function;
                    global.Function = function(...args) {
                        const body = args.join("; ");
                        __native_sink_hit("Function", body);
                        return _origFunction.apply(this, args);
                    };

                    // Sink: setTimeout string evaluation
                    const _origSetTimeout = global.setTimeout;
                    global.setTimeout = function(handler, timeout, ...args) {
                        if (typeof handler === "string") {
                            __native_sink_hit("setTimeout", handler);
                        }
                        if (typeof _origSetTimeout === "function") {
                            return _origSetTimeout(handler, timeout, ...args);
                        }
                        return 0;
                    };

                    // Sink: setInterval string evaluation
                    const _origSetInterval = global.setInterval;
                    global.setInterval = function(handler, timeout, ...args) {
                        if (typeof handler === "string") {
                            __native_sink_hit("setInterval", handler);
                        }
                        if (typeof _origSetInterval === "function") {
                            return _origSetInterval(handler, timeout, ...args);
                        }
                        return 0;
                    };
                })();
            "#;

            ctx.eval::<(), _>(preamble)?;
            Ok(())
        })?;

        Ok(())
    }

    /// Evaluates JavaScript code and returns the stringified result.
    ///
    /// Execution is interrupted inside QuickJS when the time budget elapses or the
    /// cancel handle fires; the error then downcasts to [`JsInterrupt`].
    pub fn eval(&self, script: &str) -> Result<String> {
        if self.cancel.is_cancelled() {
            return Err(anyhow::Error::new(JsInterrupt::Cancelled));
        }
        if let Ok(mut slot) = self.interrupted.lock() {
            *slot = None;
        }
        if let Ok(mut d) = self.deadline.lock() {
            *d = Some(Instant::now() + self.limits.time_budget);
        }
        let result = self.eval_unbounded(script);
        if let Ok(mut d) = self.deadline.lock() {
            *d = None;
        }
        if let Some(reason) = self.interrupted.lock().ok().and_then(|mut s| s.take()) {
            return Err(anyhow::Error::new(reason));
        }
        result
    }

    fn eval_unbounded(&self, script: &str) -> Result<String> {
        let res = self.context.with(|ctx| -> rquickjs::Result<String> {
            let val: Value = ctx.eval(script)?;
            if val.is_string() {
                Ok(val.as_string().unwrap().to_string()?)
            } else if val.is_int() {
                Ok(val.as_int().unwrap().to_string())
            } else if val.is_float() {
                Ok(val.as_float().unwrap().to_string())
            } else if val.is_bool() {
                Ok(val.as_bool().unwrap().to_string())
            } else if val.is_undefined() {
                Ok("undefined".to_string())
            } else if val.is_null() {
                Ok("null".to_string())
            } else {
                // Stringify the value we already have; never re-run the script.
                match ctx.json_stringify(val)? {
                    Some(json) => Ok(json.to_string()?),
                    None => Ok("[object]".to_string()),
                }
            }
        })?;
        Ok(res)
    }

    /// Updates the browser location object with a target URL.
    pub fn set_url(&self, url: &str) -> Result<()> {
        let parsed =
            url::Url::parse(url).unwrap_or_else(|_| url::Url::parse("http://localhost/").unwrap());
        let href = parsed.as_str();
        let origin = parsed.origin().ascii_serialization();
        let protocol = parsed.scheme();
        let host = parsed.host_str().unwrap_or("");
        let port = parsed.port().map(|p| p.to_string()).unwrap_or_default();
        let pathname = parsed.path();
        let search = parsed
            .query()
            .map(|q| format!("?{}", q))
            .unwrap_or_default();
        let hash = parsed
            .fragment()
            .map(|f| format!("#{}", f))
            .unwrap_or_default();

        let update_script = format!(
            r#"
            (function() {{
                location.href = {:?};
                location.origin = {:?};
                location.protocol = {:?};
                location.host = {:?};
                location.hostname = {:?};
                location.port = {:?};
                location.pathname = {:?};
                location.search = {:?};
                location.hash = {:?};
            }})();
            "#,
            href, origin, protocol, host, host, port, pathname, search, hash
        );

        self.eval_unbounded(&update_script)?;
        Ok(())
    }

    /// Retrieves all intercepted XSS sink findings.
    pub fn get_sink_findings(&self) -> Vec<SinkFinding> {
        self.sink_findings.lock().unwrap().clone()
    }

    /// Retrieves all captured console messages.
    pub fn get_console_logs(&self) -> Vec<ConsoleMessage> {
        self.console_logs.lock().unwrap().clone()
    }

    /// Clears recorded sink findings and console messages.
    pub fn reset_findings(&self) {
        self.sink_findings.lock().unwrap().clear();
        self.console_logs.lock().unwrap().clear();
    }
}

impl Default for JsRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to initialize JsRuntime")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interrupt_of(err: &anyhow::Error) -> Option<JsInterrupt> {
        err.downcast_ref::<JsInterrupt>().copied()
    }

    #[test]
    fn test_infinite_loop_is_interrupted_by_time_budget() {
        let limits = ExecutionLimits {
            time_budget: Duration::from_millis(200),
            ..Default::default()
        };
        let js = JsRuntime::with_limits(limits, JsCancelHandle::new()).unwrap();
        let start = Instant::now();
        let err = js.eval("while (true) {}").unwrap_err();
        let elapsed = start.elapsed();
        assert_eq!(interrupt_of(&err), Some(JsInterrupt::TimedOut));
        assert!(
            elapsed >= Duration::from_millis(200),
            "returned too early: {elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_secs(3),
            "interrupt too slow: {elapsed:?}"
        );
        // The loop really stopped: the same runtime on the same thread runs new code.
        assert_eq!(js.eval("1 + 1").unwrap(), "2");
    }

    #[test]
    fn test_try_catch_cannot_swallow_interrupt() {
        let limits = ExecutionLimits {
            time_budget: Duration::from_millis(150),
            ..Default::default()
        };
        let js = JsRuntime::with_limits(limits, JsCancelHandle::new()).unwrap();
        let err = js
            .eval("for (;;) { try { while (true) {} } catch (e) {} }")
            .unwrap_err();
        assert_eq!(interrupt_of(&err), Some(JsInterrupt::TimedOut));
    }

    #[test]
    fn test_cancel_from_another_thread_stops_running_script() {
        let (handle_tx, handle_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let limits = ExecutionLimits {
                time_budget: Duration::from_secs(60),
                ..Default::default()
            };
            let js = JsRuntime::with_limits(limits, JsCancelHandle::new()).unwrap();
            handle_tx.send(js.cancel_handle()).unwrap();
            let start = Instant::now();
            let result = js.eval("while (true) {}");
            (result.map_err(|e| interrupt_of(&e)), start.elapsed())
        });
        let handle = handle_rx.recv().unwrap();
        std::thread::sleep(Duration::from_millis(100));
        handle.cancel();
        let (result, elapsed) = worker.join().unwrap();
        assert_eq!(result, Err(Some(JsInterrupt::Cancelled)));
        assert!(
            elapsed < Duration::from_secs(5),
            "cancel too slow: {elapsed:?}"
        );
    }

    #[test]
    fn test_cancelled_runtime_refuses_new_work() {
        let cancel = JsCancelHandle::new();
        let js = JsRuntime::with_limits(ExecutionLimits::default(), cancel.clone()).unwrap();
        cancel.cancel();
        let err = js.eval("1 + 1").unwrap_err();
        assert_eq!(interrupt_of(&err), Some(JsInterrupt::Cancelled));
    }

    #[test]
    fn test_memory_limit_stops_runaway_allocation() {
        let limits = ExecutionLimits {
            time_budget: Duration::from_secs(20),
            memory_limit_bytes: 16 * 1024 * 1024,
            ..Default::default()
        };
        let js = JsRuntime::with_limits(limits, JsCancelHandle::new()).unwrap();
        let start = Instant::now();
        let result = js.eval("let a = []; while (true) { a.push(new Array(100000).fill(1)); }");
        assert!(result.is_err());
        assert!(start.elapsed() < Duration::from_secs(20));
    }

    #[test]
    fn test_object_results_evaluate_script_once() {
        let js = JsRuntime::new().unwrap();
        let out = js
            .eval("globalThis.runs = (globalThis.runs || 0) + 1; ({ a: 1 })")
            .unwrap();
        assert_eq!(out, r#"{"a":1}"#);
        assert_eq!(js.eval("runs").unwrap(), "1");
    }

    #[test]
    fn test_js_eval_and_console() {
        let js = JsRuntime::new().expect("init js runtime");
        let res = js.eval("1 + 2").expect("eval math");
        assert_eq!(res, "3");

        js.eval("console.log('Excalibur JS Initialized', 42);")
            .expect("eval console.log");
        let logs = js.get_console_logs();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].level, "log");
        assert_eq!(logs[0].message, "Excalibur JS Initialized 42");
    }

    #[test]
    fn test_sink_interception_eval() {
        let js = JsRuntime::new().expect("init js runtime");
        js.eval("eval('window.__pwned = 1');")
            .expect("eval sink call");

        let findings = js.get_sink_findings();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].sink_name, "eval");
        assert_eq!(findings[0].argument, "window.__pwned = 1");
    }

    #[test]
    fn test_sink_interception_inner_html_and_document_write() {
        let js = JsRuntime::new().expect("init js runtime");
        js.eval("document.write('<img src=x onerror=alert(1)>');")
            .expect("eval doc write");
        js.eval(
            "const el = document.createElement('div'); el.innerHTML = '<svg onload=alert(2)>';",
        )
        .expect("eval innerHTML");

        let findings = js.get_sink_findings();
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].sink_name, "document.write");
        assert!(findings[0].argument.contains("alert(1)"));
        assert_eq!(findings[1].sink_name, "Element.innerHTML");
        assert!(findings[1].argument.contains("alert(2)"));
    }

    #[test]
    fn test_location_and_globals() {
        let js = JsRuntime::new().expect("init js runtime");
        js.set_url("https://victim.com:8443/login?user=admin#section")
            .expect("set url");

        assert_eq!(js.eval("location.hostname").unwrap(), "victim.com");
        assert_eq!(js.eval("location.pathname").unwrap(), "/login");
        assert_eq!(js.eval("location.search").unwrap(), "?user=admin");
        assert_eq!(js.eval("location.port").unwrap(), "8443");
        assert_eq!(js.eval("window === globalThis").unwrap(), "true");
    }
}
