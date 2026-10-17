# ⚔️ Excalibur

> **The Developer & Penetration Testing Browser**  
> *Independent, non-Chromium browser architecture with dual Native GUI and Terminal TUI interfaces.*

---

## ⚡ Overview

**Excalibur** is a standalone, security-first browser and protocol inspection workbench written in 100% pure Rust. Engineered specifically for penetration testers, security researchers, and web developers, Excalibur combines low-level protocol inspection, live request interception, script analysis, and multi-view rendering into a single, high-performance binary.

---

## 🚀 Key Features

- **Independent Engine**: Zero Chromium / WebKit dependency. Custom parsing and rendering pipeline with full inspection of every DOM node, script hook, and network frame.
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

## 🛠️ Architecture

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
```

---

## 📜 License

MIT License. Designed and maintained by [Zeo120](https://github.com/Zeo120).
