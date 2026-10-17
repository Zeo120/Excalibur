# Contributing & Engineering Standards

This document establishes the code quality, testing, error handling, and contribution standards for **Excalibur**.

---

## 1. Code Quality & Formatting

All code committed to Excalibur must strictly adhere to automated style and linting standards.

### 1.1 Formatting
* Format all code with standard `rustfmt`:
  ```bash
  cargo fmt --all
  ```
* Continuous Integration enforces formatting via:
  ```bash
  cargo fmt --all -- --check
  ```

### 1.2 Clippy Linting
* Code must compile with zero warnings under Clippy:
  ```bash
  cargo clippy --workspace --all-targets -- -D warnings
  ```
* In particular, the following Clippy lint groups are strictly enforced:
  * `clippy::all`
  * `clippy::pedantic` (with selective exceptions documented inline)
  * `clippy::clone_on_ref_ptr`
  * `clippy::inefficient_to_string`

---

## 2. Error Handling Idioms

Excalibur enforces a clean separation between library error types and application-level error aggregation.

### 2.1 Internal Library Crates (`core`, `dom`, `proxy`, `script`, `audit`)
* Internal crates must define domain-specific, strongly-typed error enums using `thiserror`.
* Functions must never return generic strings or panic on recoverable errors.
* Example:
  ```rust
  use thiserror::Error;

  #[derive(Error, Debug)]
  pub enum CoreError {
      #[error("I/O error: {0}")]
      Io(#[from] std::io::Error),

      #[error("Database error: {0}")]
      Database(#[from] rusqlite::Error),

      #[error("Invalid URL: {0}")]
      InvalidUrl(#[from] url::ParseError),
  }

  pub type Result<T> = std::result::Result<T, CoreError>;
  ```

### 2.2 Application & Entrypoint Crates (`cli`, `gui`, `tui`)
* Top-level binary and UI controllers aggregate errors using `anyhow::Result<T>` and `anyhow::Context`.
* Errors shown to the user must provide actionable context (e.g. `"Failed to bind MITM proxy on 127.0.0.1:8080"` rather than raw OS error codes).

---

## 3. Unsafe Code Policy

* **Default Policy**: `#![forbid(unsafe_code)]` is enforced at the root of every pure-Rust crate (`excalibur-core`, `excalibur-dom`, `excalibur-audit`, `excalibur-tui`, `excalibur-cli`).
* **FFI Exceptions**: Crates that interface with C libraries via FFI (`rquickjs` or `mlua`) must isolate all `unsafe` blocks with detailed `// SAFETY:` comments explaining invariants, pointer validity, and lifetime guarantees.

---

## 4. Testing Conventions

Excalibur requires test-driven validation across all layers.

### 4.1 Unit Testing
* Unit tests reside in the same file as the code they test within a `#[cfg(test)] mod tests` module.
* Every parser, token scanner, rule matcher, and SQL query must have corresponding unit test coverage.

### 4.2 Integration Testing
* Integration tests reside in the `tests/` directory of each crate.
* Network and proxy integration tests should use local mock servers (such as `wiremock` or ephemeral Tokio TCP listeners) to avoid flakiness and external internet dependencies.

### 4.3 Running Tests
* Run the entire workspace test suite:
  ```bash
  cargo test --workspace
  ```

---

## 5. Documentation Standards

* Every public struct, enum, function, method, and trait must have Rustdoc comments (`///`).
* Code blocks in documentation must be valid, compilable doctests whenever practical.
* Module-level documentation (`//!`) must appear at the top of each crate's `lib.rs` and `main.rs`.

---

## 6. Commit Message Conventions

Commits follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

* `feat:` Introduces a new feature or user-visible capability.
* `fix:` Patches a bug or behavioral defect.
* `docs:` Documentation additions or updates.
* `refactor:` Code reorganization without behavioral alterations.
* `perf:` Performance optimizations.
* `test:` Adding or updating automated tests.
* `ci:` Changes to GitHub Actions workflows or build configurations.

**Rule**: Commit messages and documentation must remain strictly professional and contain zero emojis.
