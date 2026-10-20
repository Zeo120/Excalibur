use excalibur_core::network::{HttpRequest, HttpResponse};
use excalibur_script::{JsRuntime, LuaRuntime};

#[test]
fn test_js_runtime_xss_sink_pipeline() {
    let js = JsRuntime::new().expect("create js runtime");

    // Simulating DOM XSS sink vulnerability
    let malicious_script = r#"
        function handleUserComment(comment) {
            document.write("<div>Comment: " + comment + "</div>");
            eval("var lastComment = '" + comment + "'");
        }
        handleUserComment("<script>alert(document.cookie)</script>");
    "#;

    js.eval(malicious_script).expect("eval script");

    let findings = js.get_sink_findings();
    assert_eq!(findings.len(), 2);

    let doc_write = findings
        .iter()
        .find(|f| f.sink_name == "document.write")
        .unwrap();
    assert!(doc_write
        .argument
        .contains("<script>alert(document.cookie)</script>"));

    let eval_sink = findings.iter().find(|f| f.sink_name == "eval").unwrap();
    assert!(eval_sink.argument.contains("lastComment"));
}

#[test]
fn test_lua_packet_mutation_pipeline() {
    let lua = LuaRuntime::new().expect("create lua runtime");

    // Script modifies Authorization header and injects SQLi test in body
    let script = r#"
        function on_request(req)
            req.headers["Authorization"] = "Bearer injected-token-99"
            req.headers["X-Custom-Scan"] = "true"
            local payloads = fuzz.generate_payloads("sqli")
            req.body = "username=" .. fuzz.url_encode(payloads[1])
            return req
        end

        function on_response(res)
            res.headers["X-Audit-Passed"] = "false"
            return res
        end
    "#;

    lua.execute(script).expect("exec lua script");

    // Test request mutation
    let req = HttpRequest::new("POST", "https://api.victim.com/v1/auth");
    let mutated_req = lua.apply_request_hook(req).expect("apply request hook");

    assert_eq!(
        mutated_req.headers.get("Authorization"),
        Some("Bearer injected-token-99")
    );
    assert_eq!(mutated_req.headers.get("X-Custom-Scan"), Some("true"));
    let body_str = String::from_utf8_lossy(&mutated_req.body);
    assert!(body_str.contains("username=%27%20OR%20%271%27%3D%271"));

    // Test response mutation
    let res = HttpResponse::new("req-101", 200);
    let mutated_res = lua.apply_response_hook(res).expect("apply response hook");
    assert_eq!(mutated_res.headers.get("X-Audit-Passed"), Some("false"));
}

#[test]
fn test_lua_sandbox_restricts_dangerous_apis() {
    let lua = LuaRuntime::new().expect("create lua runtime");

    // os and io modules must be nil to prevent RCE and arbitrary file read/write
    assert_eq!(lua.eval_string("type(os)").unwrap(), "nil");
    assert_eq!(lua.eval_string("type(io)").unwrap(), "nil");
    assert_eq!(lua.eval_string("type(package)").unwrap(), "nil");
    assert_eq!(lua.eval_string("type(loadfile)").unwrap(), "nil");
    assert_eq!(lua.eval_string("type(dofile)").unwrap(), "nil");
}

#[test]
fn test_js_telemetry_cannot_be_tampered_with() {
    let js = JsRuntime::new().expect("create js runtime");

    // Native internal hooks must not be directly callable in global scope
    assert_eq!(
        js.eval("typeof globalThis.__native_sink_hit").unwrap(),
        "undefined"
    );
    assert_eq!(
        js.eval("typeof globalThis.__native_console_msg").unwrap(),
        "undefined"
    );

    // eval hook must be non-writable and non-configurable
    let _ = js.eval("globalThis.eval = function() { return 42; };");
    js.eval("eval('var secret = 1337;')").unwrap();

    let findings = js.get_sink_findings();
    assert!(findings
        .iter()
        .any(|f| f.sink_name == "eval" && f.argument.contains("secret")));
}
