# Design Decisions & Technical Rationale

This document provides exhaustive explanations and justifications for every architectural, technological, and engine-level decision in **Excalibur**.

---

## 1. Programming Language: Why Rust?

### Evaluated Alternatives
* **C / C++**: Historically used for browser engines (Chromium, Firefox, WebKit) and security tools (Wireshark, Nmap).
* **Go**: Common for modern security networking tools (ProjectDiscovery suite, Amass, Bettercap).
* **Python**: Standard for rapid security prototyping, exploit development, and tools like Scapy or Mitmproxy.
* **Rust**: Modern systems language emphasizing memory safety, concurrency, and zero-cost abstractions.

### Decision
**Rust was selected as the sole implementation language for Excalibur.**

### Rationale
1. **Memory Safety Without Garbage Collection (GC)**:
   * Penetration testing workflows process continuous, high-volume HTTP streams, WebSocket messages, and large response payloads. In GC-dependent languages (Go, Python), high allocation rates cause unpredictable GC pauses, latency spikes, and ballooning memory overhead during high-speed fuzzing.
   * C/C++ offers zero GC pauses but introduces catastrophic memory safety risks (buffer overflows, use-after-free, integer overflows) when parsing untrusted, malformed web content. Rust guarantees memory safety at compile time without a runtime collector.
2. **True Standalone Executable**:
   * Unlike Python or Node.js, Rust compiles to a self-contained, native machine code binary with zero external runtime dependencies. A penetration tester can drop a single `excalibur.exe` or `excalibur` binary onto any target workstation or jump box and run immediately.
3. **Deep Low-Level Protocol Control**:
   * Rust provides direct control over byte buffers (`bytes`), raw TCP/TLS stream negotiation (`tokio`, `rustls`), HTTP/2 frame construction, and custom socket options required for TLS fingerprint spoofing (JA3/JA4).
4. **Fearless Concurrency**:
   * Running background proxy listeners, multi-threaded parameter fuzzers, live packet decoders, and UI event loops concurrently requires robust thread safety. Rust's `Send` and `Sync` type system catches race conditions and data corruption at compile time.

---

## 2. Browser Engine: Why Independent, Non-Chromium?

### Evaluated Alternatives
* **Chromium / CEF (Chromium Embedded Framework) / Electron**: Used by traditional desktop browsers, Postman, and Burp Suite's embedded browser.
* **WebKit**: Safari's engine; on Windows typically consumed via WebKitGTK or Cairo bindings.
* **Servo**: Mozilla/Linux Foundation independent browser engine in Rust.
* **Custom Independent Engine**: Dedicated parsing, DOM, styling, and layout pipeline tailored for security analysis.

### Decision
**Excalibur implements a dedicated, independent non-Chromium engine.**

### Rationale
1. **Elimination of Massive Architectural Bloat**:
   * The Chromium codebase exceeds 35 million lines of C++ code, compiles to hundreds of megabytes of binaries, and consumes gigabytes of RAM upon launching just a few tabs. This bloat is unacceptable for a lean, agile security testing workbench.
2. **Freedom From Chromium's Enforced Security Constraints**:
   * Consumer browsers are engineered to enforce web security policies strictly: CORS restrictions, HSTS preloading, Content Security Policy (CSP), cookie `SameSite` flags, and strict certificate revocation checking.
   * In a penetration testing context, security researchers actively need to bypass, disable, inspect, or tamper with these exact policies to assess vulnerability surfaces. Modifying Chromium's internal security checks requires maintaining an immense fork with ongoing merge conflicts. With an independent engine, security toggles are native first-class flags.
3. **Direct Introspection of Every Byte & AST Node**:
   * An independent engine allows Excalibur to inspect and hook every stage of processing: raw socket bytes -> TLS handshake -> HTTP framing -> HTML tokenization -> DOM node creation -> Script AST execution. No data is hidden behind internal browser abstraction layers.
4. **Why Not WebKit or Servo as Base?**:
   * On Windows, standard webview wrappers (such as Tauri/Wry) default to Microsoft Edge WebView2, which is Chromium-based. Embedding actual WebKit on Windows requires heavyweight external GTK/Cairo DLLs.
   * Servo is an impressive project, but its build footprint is massive, compilation on Windows is brittle, and it is designed for general consumer web compatibility rather than security interception and inspection.

---

## 3. Presentation Architecture: Why a Dual-Mode (GUI + TUI) Single Binary?

### Evaluated Alternatives
* **GUI Only**: Traditional desktop application only.
* **CLI/TUI Only**: Headless or terminal-only tool (similar to Lynx, Wget, or HTTPie).
* **Two Separate Binaries**: `excalibur-gui` and `excalibur-tui`.
* **Single Dual-Mode Binary**: Unified executable dispatching between GUI, TUI, and CLI subcommands based on execution context.

### Decision
**A single unified binary supporting both a Hardware-Accelerated Native GUI and an Interactive Full-Screen Terminal TUI.**

### Rationale
1. **Operational Reality of Security Work**:
   * Security researchers alternate constantly between multi-monitor desktop environments (where visual inspection, side-by-side repeaters, and DOM trees shine) and remote headless servers (SSH bastion hosts, cloud VPS, Kali Linux VMs, Docker containers) where no display server exists.
   * Supporting both environments within the same tool ensures zero context switching: keybindings, data models, and audit rules remain identical.
2. **Advantages of a Single Binary**:
   * Simplifies installation and packaging: package managers (`winget`, `cargo install`, `homebrew`, `apt`) distribute one executable named `excalibur`.
   * CLI dispatching logic intelligently detects whether a graphical environment is available or if stdout is an interactive TTY, defaulting gracefully or allowing explicit `--gui` / `--tui` overrides.
3. **Maximum Code Reuse**:
   * 85% of the codebase (networking, MITM proxy, SQLite storage, DOM parser, QuickJS runtime, audit rules) is completely UI-agnostic and shared across both interfaces via internal workspace crates.

---

## 4. Native GUI: Why `egui` + `wgpu`?

### Evaluated Alternatives
* **Qt / QML (via C++ or Rust bindings)**: Battle-tested, but introduces heavy C++ dependencies, licensing considerations, and complex cross-compilation.
* **Slint**: Modern declarative desktop UI framework with clean syntax, but immediate-mode flexibility was preferred for real-time security data streams.
* **Iced**: Elm-inspired reactive GUI, but requires state architecture that is less intuitive for live packet inspection streams.
* **egui with `wgpu` Backend**: Immediate Mode GUI rendered natively with pure Rust GPU acceleration.

### Decision
**`egui` with `eframe` and the `wgpu` hardware-accelerated backend.**

### Rationale
1. **Immediate Mode UI for High-Frequency Streaming Data**:
   * Penetration testing tools are characterized by rapidly updating, highly dynamic data: live scrolling HTTP request tables, continuous fuzzer progress streams, real-time response diffing, and dynamic hex viewers.
   * Retained-mode GUI frameworks require allocating and updating complex object trees for every UI element. In contrast, `egui` rebuilds the UI description every frame directly from the current application state. This eliminates synchronization bugs between backend data and UI components.
2. **Pure Rust with Zero System C++ Dependencies**:
   * `egui` compiles cleanly across Windows, Linux, and macOS without requiring external C/C++ GUI runtimes, GTK packages, or system DLLs.
3. **Hardware Acceleration via `wgpu` with Fallback**:
   * `wgpu` maps directly to modern native graphics APIs: DirectX 12 on Windows, Metal on macOS, and Vulkan on Linux. It operates at 60-144 FPS with negligible CPU overhead.
   * For virtual machines or remote desktop sessions where GPU hardware acceleration is unavailable, `eframe` supports automatic fallback to OpenGL (`glow`).
4. **Theme Customization for "Premium UI"**:
   * `egui` allows granular control over every color, corner radius, panel margin, and font glyph. This enables Excalibur to deliver a custom, high-contrast, modern dark "cyber/security" aesthetic.

---

## 5. Terminal UI: Why `ratatui` + `crossterm`?

### Evaluated Alternatives
* **Raw ANSI escape sequences**: Fragile, unmaintainable across different terminal emulators.
* **Ncurses (via Rust bindings)**: Requires C ncurses library, notoriously difficult to build portably on Windows.
* **`ratatui` with `crossterm`**: The community standard for modern terminal user interfaces in Rust.

### Decision
**`ratatui` powered by the `crossterm` backend.**

### Rationale
1. **Flawless Cross-Platform Terminal Compatibility**:
   * `crossterm` handles low-level terminal manipulation (raw mode, cursor positioning, alternate screen buffers, mouse event capture, color palettes) natively across Windows Console, Windows Terminal, ConEmu, Linux VTs, and macOS Terminal without external C dependencies.
2. **Vim-Inspired Ergonomics**:
   * Security practitioners value keyboard efficiency. `ratatui` enables split panes, numbered tabs (`[1] Browser`, `[2] Inspector`, `[3] Repeater`, `[4] Scanner`), and fast vim-like navigation (`h/j/k/l`, `/` search, `Esc` back).
3. **Rich Widget Ecosystem**:
   * Out-of-the-box support for tables with fixed headers, tree explorers for DOM nodes, scrollable syntax blocks, and gauge bars for fuzzing progress.

---

## 6. Scripting Runtime: Why Dual Engine (QuickJS + Lua)?

### Evaluated Alternatives
* **JavaScript Only (QuickJS / Boa / V8)**: Standard web language, but heavier for raw packet mutations.
* **Lua Only (mlua)**: Extremely fast and lightweight, but unfamiliar for web DOM manipulation.
* **Python (PyO3)**: Rich security ecosystem, but requires embedding a 50+ MB Python runtime and complicates distribution.
* **Dual Engine (QuickJS for Web/DOM + Lua for Fuzzing/Packets)**: Specialized runtimes mapped to specific workloads.

### Decision
**Dual Scripting Engine: QuickJS for DOM/Web scripting and Lua (`mlua`) for high-speed packet/fuzzing hooks.**

### Rationale
1. **QuickJS for Web Standards and DOM Emulation**:
   * Web developers and penetration testers write scripts targeting the browser in JavaScript. QuickJS provides an ultra-compact, fast, ES2020-compliant engine.
   * It allows Excalibur to expose mock `window`, `document`, and `location` objects so that SPAs execute cleanly, and enables security researchers to hook dangerous functions like `eval()` or track DOM XSS sinks (`element.innerHTML`) using native JS proxies.
2. **Lua (`mlua`) for High-Throughput Packet Mutation & Fuzzing**:
   * During fuzzing campaigns, a script may need to transform or mutate HTTP headers and payloads 10,000 times per second. Lua is renowned in the security industry (e.g. Nmap NSE, Wireshark dissectors) for minimal memory footprint and near-native execution speed.
   * `mlua` provides a safe, idiomatic Rust-to-Lua binding with zero overhead.

---

## 7. Dual-Role Networking & Proxy Architecture

### Evaluated Alternatives
* **Internal Browser Client Only**: Inspects only requests generated by Excalibur itself.
* **Standalone Proxy Only**: Similar to Burp Suite or OWASP ZAP, requires an external browser to be configured with proxy settings.
* **Dual-Role Engine**: Internal browser network dispatcher + Inbound MITM Proxy listener + Upstream Proxy chaining.

### Decision
**Dual-Role Engine combining internal browser requests and an inbound local MITM proxy listener.**

### Rationale
1. **End-to-End Workflow Independence**:
   * As an independent browser, Excalibur allows a user to navigate, click links, submit forms, and inspect requests immediately without configuring OS proxy settings, setting up extensions, or dealing with external tools.
2. **Interception for External Devices**:
   * By simultaneously exposing an inbound MITM proxy listener (`127.0.0.1:8080`), Excalibur can intercept traffic from external tools (curl, mobile devices on the local Wi-Fi, secondary browsers, or thick-client applications). All external traffic flows into the exact same Inspector, Repeater, and SQLite storage.
3. **Upstream Proxy Chaining**:
   * Real-world penetration testing frequently mandates routing through Tor (`127.0.0.1:9050`) for anonymity or internal corporate SOCKS5 bastions. Excalibur supports upstream chaining natively in both modes.

---

## 8. Persistence & Storage: Why Embedded SQLite?

### Evaluated Alternatives
* **Flat Files (JSON / YAML / Raw HTTP files)**: Simple, human-readable, but disastrous performance and file-descriptor exhaustion when managing 50,000+ requests.
* **External Database (PostgreSQL / MySQL)**: High performance, but requires users to install, configure, and maintain an external database server.
* **Embedded SQLite**: Single-file, relational database engine embedded directly into the Rust binary.

### Decision
**Embedded SQLite (`rusqlite`) with WAL mode and Full-Text Search (FTS5).**

### Rationale
1. **Zero Configuration**:
   * SQLite runs entirely in-process. The user never needs to start a daemon or configure database credentials.
2. **Project Portability**:
   * An entire penetration testing engagement (request history, notes, repeater tabs, scanner findings, custom wordlists) is saved into a single `.excalibur` SQLite file that can be shared, archived, or backed up.
3. **Full-Text Search (FTS5)**:
   * Security researchers frequently need to search through gigabytes of recorded traffic for specific tokens, parameters, error messages, or reflected strings. SQLite's FTS5 module enables sub-millisecond full-text queries across all recorded headers and bodies.
4. **Stealth Ephemeral Mode**:
   * Passing `--in-memory` opens SQLite in RAM (`:memory:`), ensuring zero forensic traces are committed to disk during sensitive audits.

---

## 9. Modular Cargo Workspace Architecture

### Evaluated Alternatives
* **Monolithic Crate**: Single `excalibur` crate containing all modules.
* **Modular Multi-Crate Workspace**: 8 focused crates with distinct boundaries.

### Decision
**A multi-crate Cargo workspace separating core concerns into 8 discrete crates.**

### Rationale
1. **Compilation Speed & Incremental Caching**:
   * When modifying UI code in `excalibur-gui`, the core networking (`excalibur-core`) and parsing (`excalibur-dom`) crates do not need to be recompiled. Cargo caches each crate artifact independently.
2. **Enforced Architectural Separation**:
   * Strict dependency graphs prevent spaghetti code. For example, `excalibur-core` cannot accidentally depend on `excalibur-gui`, ensuring that headless CLI subcommands remain lean and completely free of GUI graphics dependencies.
3. **Isolated Test Suites**:
   * Each crate maintains its own unit and integration tests, enabling fast, parallelized test execution locally and in CI.

---

## 10. Modern Daily Browser vs. Penetration Testing "X-Mode" Toggle

### Evaluated Alternatives
* **Always-Exposed Penetration Testing UI**: Launching with Burp-like dense grids, hex dumps, and interception buttons occupying 70% of the screen at all times.
* **Pure Clean Consumer Browser**: Hiding or severely restricting penetration testing tools behind deep sub-menus or external windows.
* **Modal View Switcher with X-Mode Toggle**: Defaulting to a modern, clean web browser layout (multi-tab browsing, omnibox, visual DOM rendering) with a single, high-visibility menu toggle (`[ X-MODE ]`) that instantly exposes the full penetration testing workbench.

### Decision
**A dual-state Modern Browser with an instant X-Mode Penetration Testing Workbench Toggle.**

### Rationale
1. **Uncluttered Daily Browsing Experience**:
   * Modern security research requires exploring and interacting with web applications naturally (clicking links, filling forms, observing layout and behavior) before diving into protocol interception. An overly cluttered UI impairs web application workflow and navigation.
2. **Instant On-Demand Weaponization**:
   * When a researcher identifies a suspicious endpoint, form, or script, clicking `[ X-MODE ]` on the menu bar instantly docks or reveals the entire security workbench (Traffic Interceptor, Request Repeater, Vulnerability Radar, DOM and QuickJS Sink Inspector, and MITM Proxy Manager) without restarting or reloading the session.
3. **Flexible Screen Real Estate**:
   * X-Mode supports both a bottom Split Drawer (allowing live web navigation on top while viewing live intercepted packets below) and a Fullscreen Workbench view for dedicated request manipulation and repeating.

