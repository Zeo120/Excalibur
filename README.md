# Excalibur

Status: Project Pivoted (Research Ongoing)

## Overview

Excalibur has pivoted from its earlier codebase and architecture. Active research is ongoing, and the AI-assisted implementation direction has changed away from the previous prototype.

All prior source crates, build configurations, and legacy documentation have been removed from this repository to provide a clean slate while the next phase is researched and defined.

## What Changed

- The earlier effort to build an independent, non-Chromium rendering engine in Rust hit practical compatibility limits with modern web applications (CSS layout, font shaping, WebGL/Wasm, and full JavaScript execution).
- The subsequent prototype combined a Rust proxy and security tools with an OS WebView (`wry`), `egui`, and a terminal UI.
- Rather than continuing to patch the old prototype, the project has been reset to `README.md` only while research continues on the pivoted direction.

## Repository State

- All previous branches, source files, and build artifacts have been cleared.
- Only this `README.md` remains in the repository until the new architecture and scope are finalized.

## License

MIT License. Maintained by [Zeo120](https://github.com/Zeo120).
