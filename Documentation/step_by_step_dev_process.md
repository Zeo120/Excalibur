# Step-by-Step Development Process

This document defines the complete, phased engineering roadmap for implementing **Excalibur** from foundational networking to final standalone release packaging.

---

## Process Overview & Quality Gates

Development follows a strict, phased, test-driven methodology. Each phase establishes clean architectural boundaries, automated verification suites, and explicit acceptance criteria before dependent phases commence.

```
+-----------------------------------------------------------------------------------------+
| Phase 0: Toolchain & Baseline Environment                                               |
| Setup Rust GNU toolchain, verify MinGW GCC, configure root Cargo workspace.             |
+-----------------------------------------------------------------------------------------+
                                           |
                                           v
+-----------------------------------------------------------------------------------------+
| Phase 1: Core Networking & Storage Engine (excalibur-core)                               |
| Hyper HTTP/1.1 & HTTP/2, Rustls TLS, JA3/JA4 spoofing, SQLite session persistence.     |
+-----------------------------------------------------------------------------------------+
                                           |
                                           v
+-----------------------------------------------------------------------------------------+
| Phase 2: Inbound MITM Proxy & Interception (excalibur-proxy)                            |
| Dynamic Root CA, on-the-fly leaf certs, request/response pause & tamper pipeline.       |
+-----------------------------------------------------------------------------------------+
                                           |
                                           v
+-----------------------------------------------------------------------------------------+
| Phase 3: DOM Tokenization, Parsing & Layout Tree (excalibur-dom)                         |
| HTML5 tokenizer, DOM node tree explorer, CSS rule matching, hybrid canvas layout.       |
+-----------------------------------------------------------------------------------------+
                                           |
                                           v
+-----------------------------------------------------------------------------------------+
| Phase 4: Dual Scripting & Security Auditor (excalibur-script, excalibur-audit)           |
| QuickJS DOM runtime + Lua fuzzer hooks, secret regex hunter, header security auditor.   |
+-----------------------------------------------------------------------------------------+
                                           |
                                           v
+-----------------------------------------------------------------------------------------+
| Phase 5: Terminal User Interface (excalibur-tui)                                         |
| Ratatui + crossterm full-screen TUI, vim keybindings, split panes, status overlays.     |
+-----------------------------------------------------------------------------------------+
                                           |
                                           v
+-----------------------------------------------------------------------------------------+
| Phase 6: Hardware-Accelerated Native GUI (excalibur-gui)                                |
| egui + wgpu desktop workstation, dark cyber theme, dockable tabs, live visual canvas.   |
+-----------------------------------------------------------------------------------------+
                                           |
                                           v
+-----------------------------------------------------------------------------------------+
| Phase 7: Unified Entrypoint & CLI Subcommands (excalibur-cli)                           |
| Clap CLI router, headless scan, fuzz, and repeat subcommands, piping automation.        |
+-----------------------------------------------------------------------------------------+
                                           |
                                           v
+-----------------------------------------------------------------------------------------+
| Phase 8: Hardening, Performance Profiling & Standalone Release Packaging                 |
| Clippy audit, fuzzing test runs, memory leak checks, multi-platform release artifacts.  |
+-----------------------------------------------------------------------------------------+
```

---

## Phase 0: Toolchain & Baseline Environment

### Objectives
* Establish a reproducible local development environment and cross-platform CI/CD baseline.

### Step-by-Step Tasks
1. **Toolchain Installation**:
   * Verify MinGW GCC 16.x or MSVC build tools.
   * Install `rustup` and set default toolchain to `stable-x86_64-pc-windows-gnu` (or MSVC when available).
   * Confirm installation of `cargo`, `rustc`, `rustfmt`, and `clippy`.
2. **Workspace Initialization**:
   * Create root `Cargo.toml` with `resolver = "2"` and centralize all workspace dependency versions.
   * Configure `.gitignore` to protect against committing binaries, database files (`*.sqlite`, `*.excalibur`), and private keys/certificates (`*.pem`, `*.key`).
3. **Repository CI Pipeline**:
   * Configure GitHub Actions workflow (`.github/workflows/ci.yml`) matrix across Ubuntu, Windows, and macOS.
   * Add automated checks for `cargo fmt --check`, `cargo check --workspace`, `cargo clippy -- -D warnings`, and `cargo test --workspace`.

### Acceptance Criteria
* `cargo check --workspace` passes cleanly.
* GitHub Actions CI runs successfully across all three operating systems.

---

## Phase 1: Core Networking & Storage Engine (`excalibur-core`)

### Objectives
* Build the low-level HTTP/1.1 and HTTP/2 client engine with custom TLS parameters, JA3/JA4 fingerprint emulation, upstream proxy routing, and SQLite workspace persistence.

### Step-by-Step Tasks
1. **Data Models (`src/network.rs`)**:
   * Define structured types: `HttpRequest`, `HttpResponse`, `HttpHeaders`, `HttpMethod`, and `HttpVersion`.
   * Implement binary serialization and deserialization via `serde`.
2. **Storage Subsystem (`src/storage.rs`)**:
   * Implement `WorkspaceStorage` wrapping `rusqlite::Connection`.
   * Configure SQLite optimizations: WAL mode (`PRAGMA journal_mode=WAL;`), synchronous normal (`PRAGMA synchronous=NORMAL;`), and memory cache sizing.
   * Create indexed schemas for `requests`, `responses`, `bookmarks`, and `audit_findings`.
   * Implement FTS5 full-text search table across headers, URLs, and text bodies.
   * Provide in-memory constructor (`WorkspaceStorage::in_memory()`) for ephemeral stealth mode.
3. **Custom TLS Connector (`src/tls.rs`)**:
   * Implement `RustlsConnector` with configurable verification policies (allow self-signed certificates, bypass hostname validation on user flag).
   * Add support for custom cipher suite ordering and TLS extension customization to emulate specific browser JA3/JA4 fingerprints.
4. **HTTP Client Dispatcher (`src/client.rs`)**:
   * Wrap `hyper_util::client::legacy::Client` with support for HTTP/1.1 and HTTP/2 multiplexing.
   * Implement upstream proxy chaining supporting HTTP CONNECT proxies and SOCKS5 endpoints.

### Verification & Automated Tests
* Unit test: Insert and query 10,000 HTTP requests in SQLite; measure execution time (<100ms in WAL mode).
* Unit test: Verify full-text search query retrieves requests by header token and body substring.
* Integration test: Execute HTTPS request against a local test server with a self-signed certificate, verifying bypass flag functionality.

### Acceptance Criteria
* Fast, crash-resilient HTTP/HTTPS request dispatching with complete recording of raw bytes, timestamps, and headers into SQLite.

---

## Phase 2: Inbound MITM Proxy & Interception Engine (`excalibur-proxy`)

### Objectives
* Implement an inbound HTTP/HTTPS MITM proxy server with on-the-fly certificate forging, upstream routing, and a real-time request/response interception pipeline.

### Step-by-Step Tasks
1. **Dynamic Certificate Authority (`src/cert.rs`)**:
   * Use `rcgen` to generate an internal root CA keypair and self-signed certificate.
   * Implement on-demand leaf certificate generation for intercepted domains matching incoming `CONNECT` requests, cached in memory via `moka` or LRU map to avoid repeated signing overhead.
   * Support exporting the root CA certificate in PEM and DER formats for user installation on target devices.
2. **Inbound Proxy Server (`src/server.rs`)**:
   * Bind Tokio TCP listener to configurable address (default `127.0.0.1:8080`).
   * Handle plain HTTP proxy requests (`GET http://target.com/path`).
   * Handle HTTPS tunneling (`CONNECT target.com:443`): negotiate dynamic TLS handshake with downstream client, establish upstream TLS connection, and bridge the traffic.
3. **Interception Pipeline (`src/interceptor.rs`)**:
   * Implement thread-safe breakpoint engine with Tokio channels (`mpsc::channel`).
   * Define `InterceptAction`: `Forward`, `Drop`, `ModifyRequest`, `ModifyResponse`.
   * Provide rule-matching filters (regex matching on URL, method, status code, or header presence).
   * Forward all intercepted and forwarded traffic into `excalibur-core` storage.

### Verification & Automated Tests
* Unit test: Generate dynamic leaf certificate for `example.com` and verify signature against the root CA.
* Integration test: Route an external `curl -x 127.0.0.1:8080 https://httpbin.org/get` request through the proxy, verifying transparent TLS termination and SQLite record creation.
* Integration test: Set interception breakpoint on `/test`, modify request header from `foo` to `bar`, forward, and verify the upstream server received the modified header.

### Acceptance Criteria
* Fully functional MITM proxy capable of intercepting, modifying, and replaying arbitrary HTTPS traffic.

---

## Phase 3: DOM Tokenization, Parsing & Layout Tree (`excalibur-dom`)

### Objectives
* Implement a robust, non-Chromium HTML5 parser and DOM node tree explorer capable of extracting links, forms, scripts, computed styles, and rendering layout geometry.

### Step-by-Step Tasks
1. **HTML5 Tokenization & Tree Construction (`src/parser.rs`)**:
   * Implement HTML5 tokenizer and DOM tree builder using `html5ever` and `kuchiki`.
   * Construct `DomNode` hierarchy representing document elements, attributes, text nodes, and comments.
   * Handle malformed, unclosed, or deeply nested HTML safely without stack overflow or panics.
2. **DOM Node Tree Explorer (`src/tree.rs`)**:
   * Provide tree navigation methods: parent, children, siblings, depth-first and breadth-first iterators.
   * Implement CSS selector querying (`selectors` crate) to locate elements (`document.querySelectorAll`).
   * Support live attribute mutation and node insertion for interactive testing.
3. **Form & Resource Extractor (`src/extractor.rs`)**:
   * Extract all interactive forms: action URL, method, input fields, CSRF tokens, hidden fields.
   * Extract all external resource references: `<script src>`, `<link rel="stylesheet">`, `<img src>`, `<iframe>`.
4. **Hybrid Canvas Layout Engine (`src/layout.rs`)**:
   * Convert DOM nodes into visual layout blocks (boxes, text spans, buttons, input fields).
   * Calculate bounding boxes, padding, and text wrap geometry for rendering in both the GUI canvas and formatted terminal flow.

### Verification & Automated Tests
* Unit test: Parse complex real-world HTML documents (Wikipedia, GitHub, Hacker News) and assert tree structure validity.
* Unit test: Extract forms and inputs from login templates, validating input name and type extraction.
* Stress test: Parse deeply nested HTML (1,000 levels deep) and malformed tags to confirm zero crashes.

### Acceptance Criteria
* Accurate DOM node hierarchy generation, fast CSS selector querying, and layout geometry ready for presentation.

---

## Phase 4: Dual Scripting & Real-Time Security Auditor (`excalibur-script`, `excalibur-audit`)

### Objectives
* Integrate QuickJS for sandboxed JavaScript DOM execution, Lua (`mlua`) for high-throughput packet mutation, and build automated security audit checks.

### Step-by-Step Tasks
1. **QuickJS Web Runtime (`excalibur-script/src/js.rs`)**:
   * Initialize embedded QuickJS context via `rquickjs`.
   * Bind minimal browser globals: `window`, `document`, `location`, `console.log`.
   * Implement DOM XSS sink interception: hook `eval()`, `Function()`, `innerHTML`, `document.write` to log and trace unvalidated inputs.
2. **Lua Fuzzing Engine (`excalibur-script/src/lua.rs`)**:
   * Initialize embedded Lua 5.4 state via `mlua`.
   * Expose request transform hooks: `on_request(req)`, `on_response(res)`.
   * Expose fuzzer payload generation helper functions.
3. **Security Header & Configuration Auditor (`excalibur-audit/src/headers.rs`)**:
   * Audit response headers: Strict-Transport-Security (HSTS), Content-Security-Policy (CSP), X-Frame-Options, X-Content-Type-Options, Referrer-Policy.
   * Audit CORS configurations: detect wildcard `Access-Control-Allow-Origin: *` with credentials.
   * Audit Cookie attributes: detect missing `Secure`, `HttpOnly`, or `SameSite` flags.
4. **Secret & Sensitive Data Hunter (`excalibur-audit/src/secrets.rs`)**:
   * Implement multi-pattern regex engine for high-entropy tokens: AWS keys (`AKIA...`), GitHub personal tokens (`ghp_...`), JWT tokens, private keys, API keys.
   * Scan HTML source, JavaScript bundles, and response bodies in real-time.

### Verification & Automated Tests
* Unit test: Execute JavaScript in QuickJS context, verify `console.log` capture and global variable scoping.
* Unit test: Hook `eval()` in QuickJS and assert that sink call triggers an audit event.
* Unit test: Run header auditor against mock HTTP responses with missing security headers; verify finding enumeration.
* Unit test: Scan response body containing dummy AWS keys; verify secret hunter flags the exact match and offset.

### Acceptance Criteria
* Sandboxed JavaScript execution with sink monitoring, high-speed Lua hooks, and automated vulnerability detection.

---

## Phase 5: Terminal User Interface (`excalibur-tui`)

### Objectives
* Construct a full-screen, keyboard-driven terminal application using `ratatui` with split panes, numbered tabs, and vim-style navigation.

### Step-by-Step Tasks
1. **Application State & Event Loop (`src/app.rs`)**:
   * Initialize `crossterm` in alternate screen mode and raw keyboard capture.
   * Implement asynchronous event loop handling keyboard inputs, mouse clicks, and background network notifications.
   * Manage tab navigation: `[1] Browser`, `[2] Inspector`, `[3] Repeater`, `[4] Scanner`.
2. **Browser Viewport (`src/views/browser.rs`)**:
   * Display formatted text flow of the rendered web page with highlighted hyperlinks and form inputs.
   * Support inline URL bar (`Ctrl+L` or `/`) with navigation history (`H`/`L` for back/forward).
3. **Traffic Inspector View (`src/views/inspector.rs`)**:
   * Render real-time table of intercepted HTTP requests (ID, Method, Status, URL, Latency, Size).
   * Split-pane request/response viewer with formatted headers and body previews.
4. **Repeater Workbench (`src/views/repeater.rs`)**:
   * Dual-pane raw HTTP request editor and response viewer.
   * One-key request resend (`Ctrl+R`), response diffing, and latency display.
5. **Security Scanner View (`src/views/scanner.rs`)**:
   * Real-time list of audit findings (missing headers, detected secrets, CORS flaws) categorized by severity.

### Verification & Automated Tests
* Terminal UI test: Render frames in an in-memory test backend (`ratatui::backend::TestBackend`), verifying widget layout and tab switching.
* Keybinding test: Simulate keyboard events (`1`, `2`, `3`, `q`), asserting state updates.

### Acceptance Criteria
* Highly responsive, full-screen terminal interface operating cleanly inside any standard ANSI terminal emulator.

---

## Phase 6: Hardware-Accelerated Native GUI (`excalibur-gui`)

### Objectives
* Build the desktop workstation using `egui` and `eframe` (`wgpu` backend) featuring a modern cyber/dark aesthetic, dockable panels, live visual canvas, and DOM tree inspector.

### Step-by-Step Tasks
1. **Theme & Design System (`src/theme.rs`)**:
   * Configure custom cyber/dark theme: deep dark backgrounds (`#0C0E12`, `#12141A`), crisp borders, neon cyan/green accent highlights, and high-legibility monospace fonts for raw code/hex.
2. **Navigation Bar & Tab Controller (`src/app.rs`)**:
   * Top navigation bar with URL input field, navigation buttons (Back, Forward, Reload), interception toggle switch, and mode switcher.
   * Dockable panel layout supporting horizontal and vertical split panes.
3. **Multi-View Hybrid Canvas (`src/views/canvas.rs`)**:
   * Visual Viewport: Render styled HTML elements (headings, text, buttons, form inputs, links) directly into `egui` painter layouts.
   * DOM Tree Explorer: Expandable/collapsible node tree with node attribute editing, computed style preview, and XSS sink badges.
   * Raw Source & Hex Viewer: Syntax-highlighted HTML/JS viewer with regex search, line numbers, and hex dump toggle.
4. **Desktop Inspector & Repeater (`src/views/workbench.rs`)**:
   * Live streaming table of all intercepted requests with search and status-code filtering.
   * Side-by-side Repeater tab with syntax highlighting, parameter table editor, and visual response diffing.
5. **Vulnerability Overlay**:
   * Render contextual warning badges directly over visual elements (e.g. CSRF-vulnerable forms, password inputs transmitted over plaintext HTTP).

### Verification & Automated Tests
* Visual rendering test: Verify frame generation without panic using `egui` headless context.
* Event handling test: Simulate button clicks, URL submission, and tab switching.

### Acceptance Criteria
* Premium desktop user experience with instant responsiveness, 60+ FPS rendering, dockable split views, and rich visual inspection.

---

## Phase 7: Unified Entrypoint & CLI Subcommands (`excalibur-cli`)

### Objectives
* Implement the top-level command-line parser, intelligent interface dispatcher (GUI vs TUI vs CLI), and headless automation subcommands.

### Step-by-Step Tasks
1. **CLI Argument Parser (`src/main.rs`)**:
   * Define argument structure using `clap` derive macros:
     * `excalibur [URL]` (defaults to GUI, or TUI if headless)
     * `excalibur --gui` (force GUI mode)
     * `excalibur --tui` (force Terminal mode)
     * `excalibur --in-memory` (stealth ephemeral storage)
     * `excalibur --proxy-upstream <URL>` (upstream proxy chaining)
2. **Subcommand Implementations**:
   * `excalibur scan <URL>`: Headless security audit; crawls target, executes passive checks, outputs structured JSON or SARIF report to stdout.
   * `excalibur proxy [--bind <ADDR>]`: Launches standalone MITM proxy server without UI.
   * `excalibur repeat <REQUEST_ID>`: Replays a stored request and prints response to stdout.
   * `excalibur fuzz <URL> --wordlist <FILE>`: Multi-threaded parameter fuzzer.
3. **TTY Auto-Detection**:
   * Detect whether stdout is a TTY and whether a display server is reachable, defaulting gracefully to TUI on headless servers.

### Verification & Automated Tests
* End-to-end test: Execute `excalibur --help` and verify all flags and subcommands.
* End-to-end test: Execute `excalibur scan https://example.com` headlessly, verifying JSON output on stdout.
* End-to-end test: Execute `excalibur proxy --bind 127.0.0.1:9999` and assert proxy listener initializes.

### Acceptance Criteria
* Single binary capable of running the desktop GUI, full-screen terminal TUI, or headless automation pipelines from the exact same executable.

---

## Phase 8: Hardening, Performance Profiling & Standalone Release Packaging

### Objectives
* Rigorously audit the codebase for security, eliminate compiler warnings, profile memory/CPU utilization, and package multi-platform release binaries.

### Step-by-Step Tasks
1. **Security & Code Quality Audit**:
   * Run `cargo clippy --workspace --all-targets -- -D warnings`.
   * Run `cargo audit` to scan dependencies for known vulnerabilities.
   * Verify `#![forbid(unsafe_code)]` across all crates (except strictly audited FFI in QuickJS/Lua).
2. **Performance & Memory Profiling**:
   * Profile memory footprint under sustained load (10,000 requests processed).
   * Verify memory remains bounded and zero memory leaks occur.
   * Profile UI frame times in `egui` to ensure consistent 60+ FPS rendering.
3. **Release Artifact Generation**:
   * Configure optimized release profile in root `Cargo.toml`:
     * `opt-level = 3`
     * `lto = true` (Link-Time Optimization)
     * `codegen-units = 1`
     * `panic = "abort"`
     * `strip = true`
   * Compile standalone release binaries:
     * Windows: `excalibur.exe` (x86_64-pc-windows-gnu / msvc)
     * Linux: `excalibur` (x86_64-unknown-linux-gnu, musl static)
     * macOS: `excalibur` (Universal binary: x86_64 & aarch64 Apple Silicon)
4. **Documentation Finalization**:
   * Review all documentation in `Documentation/` to ensure 100% alignment with the final implementation.

### Acceptance Criteria
* Zero Clippy warnings, zero known dependency vulnerabilities, single standalone binary output, and verified cross-platform release builds.
