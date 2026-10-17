# Assumptions & Operational Scope

This document details all technical, operational, architectural, and security assumptions underpinning the design and implementation of **Excalibur**. Every architectural decision in Excalibur stems directly from these foundational premises.

---

## 1. User Profile and Operational Model

### 1.1 Single-User Model
* **Assumption**: Excalibur is designed, operated, and accessed by a single user on a local machine.
* **Implications**:
  * There is no requirement for multi-tenant isolation, user authentication to access the browser binary, or multi-user access control lists (ACLs) within the local database.
  * The local SQLite workspace assumes single-process, exclusive write access per session.
  * Configuration files, secrets, and project files reside securely in the operating system user profile directory (`%USERPROFILE%` on Windows, `~` on Linux/macOS).

### 1.2 User Technical Competency
* **Assumption**: The target user is a security researcher, penetration tester, reverse engineer, or advanced web developer.
* **Implications**:
  * Users expect raw protocol visibility, granular control over TLS handshakes, unredacted headers, and direct hex byte manipulation.
  * The UI prioritizes information density, low latency, and rapid keyboard navigation over hand-holding or consumer-oriented simplifications.
  * Security warning dialogs (such as invalid upstream TLS certificates) can be globally disabled or bypassed with explicit user flags, as testing self-signed or misconfigured targets is routine in penetration testing.

---

## 2. Target Application and Network Scope

### 2.1 Target Web Architectures
* **Assumption**: Targets tested with Excalibur span traditional server-rendered websites, modern Single Page Applications (SPAs), microservices, REST APIs, GraphQL endpoints, and WebSocket channels.
* **Implications**:
  * Excalibur must provide rich DOM and visual inspection for modern HTML/CSS structures, as well as raw JSON/XML payload formatting for headless API endpoints.
  * The embedded JavaScript engine must emulate essential browser globals (`window`, `document`, `location`, `fetch`, `XMLHttpRequest`, `localStorage`) to execute client-side routing and authentication flows without crashing.

### 2.2 Network Topology & Proxying
* **Assumption**: Penetration testing often requires routing through intermediate networks, internal corporate proxies, SOCKS5 bastions, or Tor anonymization circuits.
* **Implications**:
  * Excalibur must provide native upstream proxy chaining (HTTP, HTTPS, SOCKS5, Tor `127.0.0.1:9050`) without relying on system-wide OS proxy settings.
  * Inbound MITM proxy capabilities must support binding to `127.0.0.1` (default) or configurable interfaces (e.g. `0.0.0.0`) to intercept mobile device traffic on the local subnet.

### 2.3 TLS/HTTPS Interception and Certificate Trust
* **Assumption**: Users require transparent interception of HTTPS traffic for both Excalibur's internal navigation and external downstream clients.
* **Implications**:
  * Excalibur must generate an ephemeral or persisted root Certificate Authority (CA).
  * Excalibur must dynamically forge on-the-fly leaf certificates for any target domain matching incoming `CONNECT` tunnels.
  * The user has administrative rights on their testing machine or device to install Excalibur's generated root CA certificate into their trust store when external device testing is required.

---

## 3. Host Environment and System Resources

### 3.1 Operating System and Platform Support
* **Assumption**: Excalibur must execute natively across major desktop and server operating systems: Windows 10/11, Linux (Ubuntu, Debian, Fedora, Arch), and macOS (Apple Silicon and Intel).
* **Implications**:
  * Core networking, storage, parsing, and terminal interfaces must avoid OS-specific APIs, using standard Rust cross-platform crates (`tokio`, `hyper`, `crossterm`, `rusqlite`).
  * The GUI stack (`egui`/`eframe`) must support multiple graphics backends: `wgpu` (DirectX 12 / Vulkan / Metal) with automatic software/OpenGL fallback (`glow`) when hardware acceleration is unavailable.

### 3.2 Terminal and Headless Operation
* **Assumption**: Researchers frequently conduct security audits from remote SSH jump boxes, headless cloud VPS instances, Docker containers, or continuous integration pipelines.
* **Implications**:
  * Excalibur cannot have an unconditional hard dependency on a display server (X11, Wayland, Windows Desktop).
  * In the absence of a display or when explicitly requested via `--tui`, Excalibur must execute its full suite within standard ANSI/Unicode terminal emulators.
  * Non-interactive CLI subcommands (`scan`, `fuzz`, `repeat`) must output clean JSON or structured text to `stdout` to enable shell piping (`jq`, `awk`, shell scripts).

### 3.3 Resource Bounds & Memory Management
* **Assumption**: Extended penetration testing sessions and fuzzing campaigns can capture tens of thousands of requests and run for hours or days.
* **Implications**:
  * Excalibur must avoid garbage collection pauses, uncontrolled memory growth, and leaking network buffers.
  * Large response bodies (multi-megabyte files, binary downloads) must be streamed directly to temporary SQLite storage rather than kept unbounded in system RAM.
  * The memory footprint must remain significantly lower than Chromium-based solutions (targeting <150 MB baseline RAM in GUI mode and <40 MB in TUI mode).

---

## 4. Threat Model and Browser Security Boundaries

### 4.1 Parser Memory Safety
* **Assumption**: Excalibur will consume malicious, malformed, or intentionally hostile HTML, CSS, JavaScript, and HTTP/2 frames designed to exploit memory vulnerabilities in legacy C/C++ parsers.
* **Implications**:
  * The entire codebase is implemented in safe Rust. The `#![forbid(unsafe_code)]` or strictly audited minimal unsafe policy protects against buffer overflows, use-after-free, double-free, and out-of-bounds reads.
  * Parsing crashes or panics in untrusted input must be isolated and recovered gracefully without terminating the entire browser process.

### 4.2 Script Execution Sandboxing
* **Assumption**: Executing untrusted client-side JavaScript from target websites must not grant host operating system access to the attacker.
* **Implications**:
  * The embedded QuickJS runtime must run strictly sandboxed: zero file system access, zero shell execution primitives, and zero access to host environment variables.
  * Network requests initiated from client-side scripts (`fetch`, `xhr`) must route through Excalibur's own network stack and interception rules rather than opening arbitrary raw sockets.

### 4.3 Data Confidentiality & Stealth
* **Assumption**: Certain penetration testing engagements require absolute stealth, where client data, proprietary tokens, or engagement artifacts must not leave residual forensic footprints on the host disk.
* **Implications**:
  * An `--in-memory` mode must be supported, running SQLite completely in RAM (`:memory:`) and zeroing out state on exit.
  * Log files and session captures must only be written to disk upon explicit user command or when running in a named persistent workspace.
