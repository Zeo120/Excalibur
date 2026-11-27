# Excalibur

Status: Active Pivot to a Low-Memory Chromium-Based Browser

## Background

Excalibur began as a research project to build a web browser from scratch without Chromium. During research, the project pivoted to building a Chromium-based browser instead of maintaining a custom rendering engine.

The repository was cleared to `README.md` so the new Chromium-based implementation can start cleanly without legacy code from the earlier prototype.

## Architecture Direction

- **Host Language**: **Rust** (retained from the original choice). No garbage collector, low baseline memory footprint, and direct control over OS resources.
- **Rendering Layer**: **Chromium** via a thin native embedding layer (CEF or OS-provided WebView2 on Windows to share the system runtime and reduce disk and RAM duplication), rather than Electron or a Node.js-based shell.
- **UI Chrome**: Lightweight native UI instead of a second embedded web view for browser tabs, toolbars, and menus.

## Memory Constraints

Because Chromium's multi-process model can consume large amounts of RAM, the design prioritizes keeping total memory usage low:

- Keep the browser host process minimal (no secondary JS runtime for UI chrome).
- Discard or hibernate inactive background tabs after an idle timeout.
- Cap and consolidate renderer processes where isolation policy allows.
- Avoid background telemetry, bundled extensions, and unused Chromium services.

## Current State

- Repository reset to `README.md` only.
- Next step: finalize the Chromium embedding target (`cef` vs `WebView2`) and scaffold the minimal Rust shell and tab manager.

## License

MIT License. Maintained by [Zeo120](https://github.com/Zeo120).
