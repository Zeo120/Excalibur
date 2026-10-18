use anyhow::{Context as AnyhowContext, Result};
use rquickjs::{Context, Function, Runtime, Value};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

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
}

impl JsRuntime {
    /// Creates and initializes a new JavaScript runtime with DOM mocks and sink hooks.
    pub fn new() -> Result<Self> {
        let runtime = Runtime::new().context("Failed to create QuickJS runtime")?;
        let context = Context::full(&runtime).context("Failed to create QuickJS context")?;
        let sink_findings = Arc::new(Mutex::new(Vec::new()));
        let console_logs = Arc::new(Mutex::new(Vec::new()));

        let instance = Self {
            _runtime: runtime,
            context,
            sink_findings,
            console_logs,
        };

        instance.init_environment()?;
        Ok(instance)
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

    /// Evaluates JavaScript code and returns stringified result.
    pub fn eval(&self, script: &str) -> Result<String> {
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
                let json_code = format!("JSON.stringify(({}))", script);
                let json: Value = ctx.eval(json_code.as_str())?;
                if json.is_string() {
                    Ok(json.as_string().unwrap().to_string()?)
                } else {
                    Ok("[object]".to_string())
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

        self.eval(&update_script)?;
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
