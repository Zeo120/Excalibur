use anyhow::Result;
use excalibur_core::network::{HttpHeaders, HttpRequest, HttpResponse};
use mlua::Lua;
use std::sync::Mutex;

/// Lua 5.4 scripting engine for HTTP request/response mutation and fuzzing.
pub struct LuaRuntime {
    lua: Mutex<Lua>,
}

impl LuaRuntime {
    /// Creates and initializes a new Lua runtime with fuzzer utility functions.
    pub fn new() -> Result<Self> {
        let lua = Lua::new();
        Self::init_fuzz_module(&lua)?;
        Ok(Self {
            lua: Mutex::new(lua),
        })
    }

    fn init_fuzz_module(lua: &Lua) -> Result<()> {
        let fuzz = lua.create_table()?;

        // Payload generator
        let gen_payloads = lua.create_function(|_, category: String| {
            let payloads = match category.to_ascii_lowercase().as_str() {
                "sqli" => vec![
                    "' OR '1'='1",
                    "' UNION SELECT 1,2,3--",
                    "1' AND SLEEP(5)--",
                    "admin'--",
                    "' OR 1=1--",
                ],
                "xss" => vec![
                    "<script>alert(1)</script>",
                    "<img src=x onerror=alert(1)>",
                    "\"><svg/onload=alert(1)>",
                    "javascript:alert(1)",
                    "'-alert(1)-'",
                ],
                "traversal" => vec![
                    "../../../../etc/passwd",
                    "..\\..\\..\\..\\windows\\win.ini",
                    "%2e%2e%2f%2e%2e%2fetc%2fpasswd",
                    "....//....//etc/passwd",
                ],
                "cmd" => vec!["; id", "| whoami", "`id`", "$(id)", "; ping -c 1 127.0.0.1"],
                "ssti" => vec!["{{7*7}}", "${7*7}", "<%= 7*7 %>", "#{7*7}", "*{7*7}"],
                _ => vec!["FUZZ"],
            };
            Ok(payloads)
        })?;
        fuzz.set("generate_payloads", gen_payloads)?;

        // URL encode
        let url_enc = lua.create_function(|_, input: String| {
            let mut out = String::new();
            for b in input.bytes() {
                if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
                    out.push(b as char);
                } else {
                    out.push_str(&format!("%{:02X}", b));
                }
            }
            Ok(out)
        })?;
        fuzz.set("url_encode", url_enc)?;

        // URL decode
        let url_dec = lua.create_function(|_, input: String| {
            let mut bytes = Vec::new();
            let chars: Vec<u8> = input.bytes().collect();
            let mut i = 0;
            while i < chars.len() {
                if chars[i] == b'%' && i + 2 < chars.len() {
                    if let Ok(val) = u8::from_str_radix(
                        std::str::from_utf8(&chars[i + 1..i + 3]).unwrap_or(""),
                        16,
                    ) {
                        bytes.push(val);
                        i += 3;
                        continue;
                    }
                } else if chars[i] == b'+' {
                    bytes.push(b' ');
                    i += 1;
                    continue;
                }
                bytes.push(chars[i]);
                i += 1;
            }
            Ok(String::from_utf8_lossy(&bytes).to_string())
        })?;
        fuzz.set("url_decode", url_dec)?;

        // Base64 encode
        let b64_enc = lua.create_function(|_, input: String| {
            const TABLE: &[u8; 64] =
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let bytes = input.as_bytes();
            let mut out = String::new();
            let mut i = 0;
            while i < bytes.len() {
                let b0 = bytes[i] as u32;
                let b1 = if i + 1 < bytes.len() {
                    bytes[i + 1] as u32
                } else {
                    0
                };
                let b2 = if i + 2 < bytes.len() {
                    bytes[i + 2] as u32
                } else {
                    0
                };
                let triple = (b0 << 16) | (b1 << 8) | b2;
                out.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
                out.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
                if i + 1 < bytes.len() {
                    out.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
                } else {
                    out.push('=');
                }
                if i + 2 < bytes.len() {
                    out.push(TABLE[(triple & 0x3F) as usize] as char);
                } else {
                    out.push('=');
                }
                i += 3;
            }
            Ok(out)
        })?;
        fuzz.set("base64_encode", b64_enc)?;

        // Base64 decode
        let b64_dec = lua.create_function(|_, input: String| {
            let mut clean: Vec<u8> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
            while clean.ends_with(b"=") {
                clean.pop();
            }
            let mut out = Vec::new();
            let mut buf = 0u32;
            let mut bits = 0;
            for b in clean {
                let val = match b {
                    b'A'..=b'Z' => b - b'A',
                    b'a'..=b'z' => b - b'a' + 26,
                    b'0'..=b'9' => b - b'0' + 52,
                    b'+' => 62,
                    b'/' => 63,
                    _ => continue,
                };
                buf = (buf << 6) | (val as u32);
                bits += 6;
                if bits >= 8 {
                    bits -= 8;
                    out.push((buf >> bits) as u8);
                }
            }
            Ok(String::from_utf8_lossy(&out).to_string())
        })?;
        fuzz.set("base64_decode", b64_dec)?;

        // Random alphanumeric
        let rand_str = lua.create_function(|_, len: usize| {
            let mut seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let charset = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
            let mut s = String::with_capacity(len);
            for _ in 0..len {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let idx = (seed >> 32) as usize % charset.len();
                s.push(charset[idx] as char);
            }
            Ok(s)
        })?;
        fuzz.set("random_alphanumeric", rand_str)?;

        lua.globals().set("fuzz", fuzz)?;
        Ok(())
    }

    /// Executes arbitrary Lua script code.
    pub fn execute(&self, script: &str) -> Result<()> {
        let lua = self.lua.lock().unwrap();
        lua.load(script).exec()?;
        Ok(())
    }

    /// Evaluates a Lua expression and returns its string representation.
    pub fn eval_string(&self, expr: &str) -> Result<String> {
        let lua = self.lua.lock().unwrap();
        let val: mlua::Value = lua.load(expr).eval()?;
        match val {
            mlua::Value::String(s) => Ok(s.to_str()?.to_string()),
            mlua::Value::Integer(i) => Ok(i.to_string()),
            mlua::Value::Number(n) => Ok(n.to_string()),
            mlua::Value::Boolean(b) => Ok(b.to_string()),
            mlua::Value::Nil => Ok("nil".to_string()),
            _ => Ok(format!("{:?}", val)),
        }
    }

    /// Applies `on_request(req)` hook if defined in the Lua script.
    pub fn apply_request_hook(&self, mut req: HttpRequest) -> Result<HttpRequest> {
        let lua = self.lua.lock().unwrap();
        let globals = lua.globals();
        if let Ok(func) = globals.get::<mlua::Function>("on_request") {
            let req_table = lua.create_table()?;
            req_table.set("method", req.method.clone())?;
            req_table.set("url", req.url.clone())?;
            req_table.set("version", req.version.clone())?;

            let headers_table = lua.create_table()?;
            for (k, v) in req.headers.iter() {
                headers_table.set(k, v)?;
            }
            req_table.set("headers", headers_table)?;
            req_table.set("body", String::from_utf8_lossy(&req.body).to_string())?;

            let modified: mlua::Table = func.call(req_table)?;

            if let Ok(method) = modified.get::<String>("method") {
                req.method = method;
            }
            if let Ok(url) = modified.get::<String>("url") {
                req.url = url;
            }
            if let Ok(version) = modified.get::<String>("version") {
                req.version = version;
            }
            if let Ok(h_table) = modified.get::<mlua::Table>("headers") {
                let mut new_headers = HttpHeaders::new();
                for (k, v) in h_table.pairs::<String, String>().flatten() {
                    new_headers.append(k, v);
                }
                req.headers = new_headers;
            }
            if let Ok(body) = modified.get::<String>("body") {
                req.body = body.into_bytes();
            }
        }
        Ok(req)
    }

    /// Applies `on_response(res)` hook if defined in the Lua script.
    pub fn apply_response_hook(&self, mut res: HttpResponse) -> Result<HttpResponse> {
        let lua = self.lua.lock().unwrap();
        let globals = lua.globals();
        if let Ok(func) = globals.get::<mlua::Function>("on_response") {
            let res_table = lua.create_table()?;
            res_table.set("status", res.status)?;
            res_table.set("reason", res.reason.clone())?;
            res_table.set("version", res.version.clone())?;

            let headers_table = lua.create_table()?;
            for (k, v) in res.headers.iter() {
                headers_table.set(k, v)?;
            }
            res_table.set("headers", headers_table)?;
            res_table.set("body", String::from_utf8_lossy(&res.body).to_string())?;

            let modified: mlua::Table = func.call(res_table)?;

            if let Ok(status) = modified.get::<u16>("status") {
                res.status = status;
            }
            if let Ok(reason) = modified.get::<String>("reason") {
                res.reason = reason;
            }
            if let Ok(version) = modified.get::<String>("version") {
                res.version = version;
            }
            if let Ok(h_table) = modified.get::<mlua::Table>("headers") {
                let mut new_headers = HttpHeaders::new();
                for (k, v) in h_table.pairs::<String, String>().flatten() {
                    new_headers.append(k, v);
                }
                res.headers = new_headers;
            }
            if let Ok(body) = modified.get::<String>("body") {
                res.body = body.into_bytes();
            }
        }
        Ok(res)
    }
}

impl Default for LuaRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to initialize LuaRuntime")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzz_payloads_and_encoders() {
        let lua = LuaRuntime::new().expect("init lua");

        let sqli_count: String = lua
            .eval_string("#fuzz.generate_payloads('sqli')")
            .expect("sqli payloads");
        assert_eq!(sqli_count, "5");

        let encoded = lua
            .eval_string("fuzz.url_encode('hello world & user=1')")
            .expect("url encode");
        assert_eq!(encoded, "hello%20world%20%26%20user%3D1");

        let decoded = lua
            .eval_string(&format!("fuzz.url_decode('{}')", encoded))
            .expect("url decode");
        assert_eq!(decoded, "hello world & user=1");

        let b64 = lua
            .eval_string("fuzz.base64_encode('admin:secret')")
            .expect("b64 encode");
        assert_eq!(b64, "YWRtaW46c2VjcmV0");

        let b64_dec = lua
            .eval_string(&format!("fuzz.base64_decode('{}')", b64))
            .expect("b64 decode");
        assert_eq!(b64_dec, "admin:secret");

        let rand_val = lua
            .eval_string("fuzz.random_alphanumeric(16)")
            .expect("rand string");
        assert_eq!(rand_val.len(), 16);
    }

    #[test]
    fn test_lua_request_mutation_hook() {
        let lua = LuaRuntime::new().expect("init lua");
        let script = r#"
            function on_request(req)
                req.headers["X-Fuzzed-By"] = "Excalibur-Lua"
                req.body = req.body .. "; injected_token=1337"
                return req
            end
        "#;
        lua.execute(script).expect("execute hook definition");

        let mut req = HttpRequest::new("POST", "https://api.internal/transfer");
        req.body = b"amount=100".to_vec();

        let modified = lua.apply_request_hook(req).expect("apply request hook");
        assert_eq!(modified.headers.get("X-Fuzzed-By"), Some("Excalibur-Lua"));
        assert_eq!(
            String::from_utf8_lossy(&modified.body),
            "amount=100; injected_token=1337"
        );
    }

    #[test]
    fn test_lua_response_mutation_hook() {
        let lua = LuaRuntime::new().expect("init lua");
        let script = r#"
            function on_response(res)
                res.status = 200
                res.headers["Content-Type"] = "application/json"
                res.body = "{\"status\": \"spoofed\"}"
                return res
            end
        "#;
        lua.execute(script).expect("execute hook definition");

        let res = HttpResponse::new("req-1", 403);
        let modified = lua.apply_response_hook(res).expect("apply response hook");

        assert_eq!(modified.status, 200);
        assert_eq!(
            modified.headers.get("Content-Type"),
            Some("application/json")
        );
        assert_eq!(
            String::from_utf8_lossy(&modified.body),
            "{\"status\": \"spoofed\"}"
        );
    }
}
