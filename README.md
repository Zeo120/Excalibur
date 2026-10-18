# Excalibur

> **The Developer & Penetration Testing Browser**  
> *Independent, non-Chromium browser architecture with dual Native GUI and Terminal TUI interfaces.*

---

## Overview

**Excalibur** is a standalone, security-first browser and protocol inspection workbench written in 100% pure Rust. Engineered specifically for penetration testers, security researchers, and web developers, Excalibur combines low-level protocol inspection, live request interception, script analysis, and multi-view rendering into a single, high-performance binary.

---

## Key Features

- **Independent Engine**: Zero Chromium or WebKit dependency. Custom parsing and rendering pipeline with full inspection of every DOM node, script hook, and network frame.
- **Dual Interface Modes**:
  - **Premium Native GUI**: Hardware-accelerated (`egui` / `wgpu`) workstation featuring dockable tabs, interactive DOM tree explorer, multi-view canvas, and visual security badges.
  - **Full-Screen Terminal TUI**: Keyboard-driven (`ratatui`) interface for SSH sessions, headless environments, and terminal workflows.
- **Dual-Role Network Engine**:
  - Internal browser request dispatcher with JA3/JA4 TLS fingerprint spoofing and HTTP/2 framing.
  - Inbound MITM proxy listener (`127.0.0.1:8080`) with dynamic CA generation for external tools and devices.
  - Upstream chaining through SOCKS5, Tor, or corporate HTTP proxies.
- **Live Interceptor & Repeater**: Pause, tamper, replay, and diff HTTP/HTTPS requests and responses in real-time.
- **Dual Scripting Core**:
  - **QuickJS**: Embedded JavaScript engine for DOM emulation, script hooks, and DOM XSS sink analysis.
  - **Lua (`mlua`)**: Ultra-low-overhead scripting for custom packet manipulation, payload mutation, and high-frequency fuzzing.
- **Persistent Workspaces**: Fast, indexed SQLite session storage with full-text search across all traffic, plus an ephemeral `--in-memory` mode.

---

## Architecture

```
excalibur/
├── crates/
│   ├── excalibur-core/         # Networking, TLS/JA3, SQLite persistence
│   ├── excalibur-proxy/        # MITM proxy listener & CA management
│   ├── excalibur-dom/          # HTML5 parser, DOM tree, CSS matching
│   ├── excalibur-script/       # QuickJS & Lua scripting runtimes
│   ├── excalibur-audit/        # Real-time security auditors & secret hunter
│   ├── excalibur-gui/          # egui / wgpu desktop interface
│   ├── excalibur-tui/          # ratatui terminal interface
│   └── excalibur-cli/          # Unified CLI entrypoint & subcommands
├── Documentation/              # Architecture, decisions, and dev roadmap
└── .github/workflows/          # Continuous Integration suite
```

---

## Installation & Building

Prerequisites:
- Rust 1.80+ (2021 edition)
- Cargo

Clone and build the release binary:

```bash
git clone https://github.com/Zeo120/Excalibur.git
cd Excalibur
cargo build --release
```

The compiled executable will be located at `target/release/excalibur`.

---

## Usage

### 1. Launching the Native GUI
Starts the hardware-accelerated desktop workstation:

```bash
cargo run --bin excalibur
# Or specify a target URL:
cargo run --bin excalibur -- https://example.com
```

### 2. Launching the Terminal User Interface (TUI)
Starts the full-screen terminal workbench:

```bash
cargo run --bin excalibur -- --tui https://example.com
```

### 3. Headless Security Audit Scan
Scans a target for security header misconfigurations, high-entropy secret leaks, unvalidated forms, and DOM XSS sinks:

```bash
# Formatted terminal report
cargo run --bin excalibur -- scan https://example.com

# Structured JSON report (ideal for CI/CD pipelines)
cargo run --bin excalibur -- scan https://example.com --json

# Execute with custom Lua fuzzer/auditor hook
cargo run --bin excalibur -- scan https://example.com --script scripts/fuzzer.lua
```

### 4. Running the Standalone MITM Proxy
Listens for inbound HTTP/HTTPS traffic from external browsers or mobile devices, dynamically generates leaf certificates, and records all sessions:

```bash
# Ephemeral in-memory mode
cargo run --bin excalibur -- proxy --bind 127.0.0.1:8080

# Persistent SQLite workspace mode
cargo run --bin excalibur -- proxy --bind 127.0.0.1:8080 --storage my_session.db
```

### 5. Replaying HTTP Requests (Repeater)
Replays any saved or manual request from the command line:

```bash
cargo run --bin excalibur -- repeat https://example.com/api/v1/auth --method POST
```

---

## Running the Automated Test Suite

Run all unit and integration tests across all crates:

```bash
cargo test --workspace
```

Run code formatting and static analysis checks:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## Documentation

Comprehensive architecture, assumptions, and design records are documented under the `Documentation/` directory:
- [Assumptions](Documentation/assumptions.md)
- [Design Decisions](Documentation/design_decisions.md)
- [Step-by-Step Dev Process](Documentation/step_by_step_dev_process.md)
- [Architecture Deep Dive](Documentation/architecture_deep_dive.md)
- [Contributing and Standards](Documentation/contributing_and_standards.md)

---

## License

MIT License. Designed and maintained by [Zeo120](https://github.com/Zeo120).
