# Excalibur Documentation

Welcome to the technical documentation for **Excalibur**, the developer and penetration testing browser. This directory contains comprehensive specifications, design rationales, operational assumptions, and the step-by-step development process for the entire system.

---

## Documentation Index

| Document | Description |
| :--- | :--- |
| [Assumptions](assumptions.md) | Comprehensive record of all operational, security, network, and environmental assumptions underpinning Excalibur. |
| [Design Decisions](design_decisions.md) | Exhaustive explanations and justifications for every architectural, technology, and engine decision. |
| [Step-by-Step Development Process](step_by_step_dev_process.md) | The end-to-end phased development roadmap, implementation tasks, verification criteria, and milestone gates. |
| [Architecture Deep Dive](architecture_deep_dive.md) | In-depth breakdown of internal data flows, interception state machines, DOM layout rendering, and concurrency models. |
| [Contributing & Standards](contributing_and_standards.md) | Engineering standards, linting guidelines, error handling idioms, test requirements, and repository workflows. |

---

## High-Level System Map

```
                          +------------------------------------------+
                          |              excalibur-cli               |
                          | (Unified Entrypoint & CLI Subcommands)  |
                          +--------------------+---------------------+
                                               |
                     +-------------------------+-------------------------+
                     |                                                   |
                     v                                                   v
        +-------------------------+                         +-------------------------+
        |      excalibur-gui      |                         |      excalibur-tui      |
        |  (egui / wgpu Desktop)  |                         |  (ratatui Terminal UI)  |
        +------------+------------+                         +------------+------------+
                     |                                                   |
                     +-------------------------+-------------------------+
                                               |
                                               v
        +-----------------------------------------------------------------------------+
        |                             Core Engine Layer                               |
        |                                                                             |
        |  +--------------------+  +-----------------------+  +--------------------+  |
        |  |   excalibur-dom    |  |   excalibur-script    |  |  excalibur-audit   |  |
        |  | (HTML5/CSS Parser) |  | (QuickJS / Lua mlua)  |  |  (Rules & Scanner) |  |
        |  +--------------------+  +-----------------------+  +--------------------+  |
        +--------------------------------------+--------------------------------------+
                                               |
                                               v
        +-----------------------------------------------------------------------------+
        |                          Networking & Storage Layer                         |
        |                                                                             |
        |  +--------------------+  +-----------------------+  +--------------------+  |
        |  |   excalibur-core   |  |    excalibur-proxy    |  |  SQLite Database   |  |
        |  | (HTTP/TLS/JA3/JA4) |  | (Inbound MITM & Cert) |  | (Traffic & History)|  |
        |  +--------------------+  +-----------------------+  +--------------------+  |
        +-----------------------------------------------------------------------------+
```

---

## Reading Order

1. **For Reviewers & Stakeholders**: Start with [Assumptions](assumptions.md) and [Design Decisions](design_decisions.md) to understand the project philosophy and rationale.
2. **For Core Developers**: Follow [Step-by-Step Development Process](step_by_step_dev_process.md) alongside the [Architecture Deep Dive](architecture_deep_dive.md).
3. **For Contributors**: Review [Contributing & Standards](contributing_and_standards.md) before submitting pull requests.
