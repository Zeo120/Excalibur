# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Power users and developers running on modest-RAM Windows (and cross-platform desktop) machines who want Chromium web compatibility without browser-shell bloat or background services consuming memory.

## Product Purpose

Excalibur is a low-memory Chromium-based desktop browser built with a minimal Rust host (`eframe`/`egui`) embedding Chromium via CEF (`RuntimeStyle::ALLOY`). Success means rendering modern web pages reliably while capping renderer proliferation, discarding idle background tabs, blocking tracker/ad subresources before network fetch, and exposing live OS process-tree RAM usage directly in the toolbar.

## Positioning

Strict low-RAM Chromium: native Rust UI chrome (no secondary JS/Electron runtime), real background tab sleeping via CEF browser closing, pre-network domain blocking, and zero telemetry or bundled extensions.

## Operating Context

- **Host environment**: Native desktop binary (`excalibur.exe`, `8.66` MB release) launched directly from CLI (`excalibur [url]`) or desktop shortcut, working cleanly even when started from read-only system directories (`C:\Windows\System32` -> switches CWD to executable directory before CEF initialization).
- **Primary workflows**:
  - Multi-tab browsing (`Ctrl+T`, `Ctrl+W`, `Ctrl+Tab`, `Ctrl+Shift+Tab`, pin/unpin, middle-click close).
  - Omnibox URL entry and search normalization (`Ctrl+L`, default DuckDuckGo search template).
  - Manual or automatic idle tab sleeping (`Sleep Idle`, tab context menu `Sleep Tab` / `Wake Tab`, inline restore card for sleeping tabs).
  - Built-in pre-network ad/tracker shield toggle (`DomainBlocker`) with live blocked-request counter.
  - Persistent SQLite bookmarks, browsing history, and settings (`Storage`).

## Capabilities and Constraints

- **Implemented capabilities**:
  - Multi-tab native `egui` tab strip and toolbar (`excalibur-app`) driving embedded native CEF child browsers (`excalibur-cef`).
  - Live OS process-tree memory sampler (`excalibur-core::memory::sample_process_tree_memory`) reading Windows Toolhelp + `K32GetProcessMemoryInfo` (and `/proc` on Linux).
  - Built-in `DomainBlocker` canceling matched ad/tracker/telemetry subresource requests in `on_before_resource_load` while preserving top-level main-frame navigation.
  - SQLite persistence (`rusqlite`) for bookmarks, history, `renderer_process_limit`, `tab_sleep_timeout_secs`, `home_url`, `search_template`, and `ad_block_enabled`.
- **Technical constraints**:
  - Chromium background services, breakpad, sync, component updates, domain reliability, pings, speech API, and extensions are disabled at CEF startup (`--do-not-de-elevate`, `--renderer-process-limit`).
  - No secondary webview or JS runtime for browser chrome.
  - No corporate fluff, emojis, or telemetry.

## Brand Commitments

- **Name**: Excalibur.
- **Voice**: Direct, technical, and plain-spoken. State facts and numbers without marketing language or hype.
- **Visual restraint**: High-contrast dark Operate-mode interface with zero decorative clutter, zero emojis in UI chrome, and custom `1.5px`-stroke vector icons.

## Evidence on Hand

- `README.md`: Documents the pivot from custom engine research to low-memory Rust + Chromium architecture.
- `crates/excalibur-core`: 12 passing unit tests covering URL normalization, tab open/pin/sleep/wake lifecycle, `DomainBlocker`, SQLite storage CRUD/settings, and OS process-tree memory sampling.
- Live release benchmark (`D:\dev\excalibur-t\release\excalibur.exe https://example.com`): `8.66` MB binary, 7 processes, `172.82` MB total private memory (`455.36` MB total working set).
- Absence to preserve: No external user testimonials, telemetry dashboards, or cloud sync services exist; do not invent or add them.

## Product Principles

1. **Keep the host minimal**: Browser chrome stays in native Rust/`egui`; never embed a second web runtime for UI.
2. **Reclaim idle memory aggressively**: Sleep inactive background tabs by closing their underlying CEF browser and restoring state on demand.
3. **Block waste before the network**: Drop known tracker and ad subresources in CEF request hooks before DNS/TLS/render cost occurs.
4. **Show real numbers**: Display actual OS process-tree memory and blocked request counts without hiding overhead.
5. **No bloat or noise**: Avoid background services, bundled extensions, emojis, and filler UI copy.
