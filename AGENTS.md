# AGENTS.md - Attic Development Guide

This file provides guidelines for agents working on the Attic codebase.

## Project Overview

Attic is a self-hostable Nix Binary Cache server backed by S3-compatible storage. It's a Rust workspace with four crates:
- `attic`: Core library
- `attic-client`: CLI client binary
- `attic-server`: Server binary and library
- `attic-token`: JWT token handling

## Build/Lint/Test Commands

### Using Nix (Primary)

```bash
# Enter development shell
nix develop

# Build all packages
nix build .#attic
nix build .#attic-client
nix build .#attic-server

# Run unit tests via Nix (requires Nix matrix version)
just ci-unit-tests 2.24
just ci-unit-tests default

# Run rustfmt check
just ci-rustfmt
cargo fmt --check

# Build WASM crates
just ci-build-wasm
```

### Using Cargo Directly

```bash
# Build
cargo build
cargo build -p attic-client
cargo build -p attic-server

# Run all tests
cargo test

# Run a single test (specify full module path)
cargo test attic::hash::tests::test_basic
cargo test --package attic --lib -- hash::tests::test_basic
cargo test --package attic-server --lib

# Run tests in specific crate
cargo test -p attic
cargo test -p attic-client
cargo test -p attic-server
cargo test -p attic-token

# Clippy lints
cargo clippy
cargo clippy -- -D warnings

# Format check
cargo fmt --check
```

### Running Integration Tests

Integration tests are Nix-based and run against actual Nix installations:

```bash
# Run basic integration tests (database + storage matrix)
# See integration-tests/default.nix for available combinations
```

## Code Style Guidelines

### General

- This project uses **Nix** for development and CI. Most workflows assume `nix develop` or `just` commands.
- EditorConfig is configured (see `.editorconfig`). Use an EditorConfig plugin in your editor.

### Rust Formatting

- **4 spaces** for Rust files (`.rs`)
- **2 spaces** for YAML, Markdown, Nix files (`.nix`, `.yaml`, `.md`)
- Maximum line width: 100 characters (default)
- Use `cargo fmt` to format code

### Imports

- Use absolute paths for crate-internal imports: `use crate::error::{AtticError, AtticResult};`
- Group imports by crate, then by standard library, then by external:
  ```rust
  use std::path::PathBuf;
  
  use serde::{Deserialize, Serialize};
  use tokio::fs;
  
  use crate::cache::CacheName;
  use crate::error::AtticResult;
  ```
- Use `#[allow(unused_imports)]` sparingly in test modules if needed

### Naming Conventions

- **Types**: `CamelCase` (e.g., `CacheName`, `AtticError`)
- **Functions/Variables**: `snake_case` (e.g., `cache_name()`, `validate_cache_name`)
- **Constants**: `SCREAMING_SNAKE_CASE` (e.g., `MAX_NAME_LENGTH`)
- **Modules**: `snake_case` (e.g., `mod nix_store`)
- **Traits**: `CamelCase` ending in `Trait` if not idiomatic (e.g., `Reader`)

### Error Handling

- Use the `AtticResult<T> = Result<T, AtticError>` pattern in the core library
- Use `anyhow::Result` in binary crates (client, server)
- Define errors using `displaydoc::Display` with doc comments:
  ```rust
  #[derive(Debug, Display)]
  pub enum AtticError {
      /// Invalid store path {path:?}: {reason}
      InvalidStorePath { path: PathBuf, reason: &'static str },
  }
  ```
- Implement `From` traits for error conversion
- Use descriptive error messages with format placeholders

### Lints

The project enforces strict linting. The following are required at the crate root:

```rust
#![deny(
    asm_sub_register,
    deprecated,
    missing_abi,
    unsafe_code,
    unused_macros,
    unused_must_use,
    unused_unsafe
)]
#![deny(clippy::from_over_into, clippy::needless_question_mark)]
#![cfg_attr(
    not(debug_assertions),
    deny(unused_imports, unused_mut, unused_variables,)
)]
```

- **Never use `unsafe_code`** - the entire crate denies it
- Always use `#[must_use]` or the `must_use` attribute for functions that return important values
- Use `Result` types explicitly rather than `.unwrap()` or `.expect()`

### Testing

- Use `#[cfg(test)]` module organization or `tests/` subdirectories
- Use the `cache!` macro in cache tests for convenience:
  ```rust
  macro_rules! cache {
      ($n:expr) => {
          CacheName::new($n.to_string()).unwrap()
      };
  }
  pub(crate) use cache;
  ```
- Use `insta` for snapshot testing if needed
- Use `tokio::test` for async tests

### Async/Tokio

- Use `#[tokio::main]` for binary entry points
- Use `async fn` with appropriate tokio features enabled in `Cargo.toml`
- Use `tokio::io` utilities for I/O operations

### Database (SeaORM)

- The server uses SeaORM with SQLite and PostgreSQL support
- Migrations are in `attic-server/src/migrations/`
- Use migrations for schema changes

### HTTP Server (Axum)

- The server uses Axum with `tower-http` for middleware
- Use `axum-macros` for request parsing
- Follow existing route handler patterns in `attic-server/src/`

### Dependencies

- Be careful with feature flags - the crate has features: `chunking`, `io`, `nix_store`, `tokio`
- Use `default-features = false` when the client/server doesn't need all features
- Keep dependencies in sync with the workspace policy

### Documentation

- Use doc comments (`///`) for public API
- Use module-level docs (`//!`) for crate root and major modules
- Document error variants with `///` comments (used by `displaydoc`)

### Git Conventions

- Follow conventional commits if contributing (but not strictly enforced)
- Run `cargo fmt` and `cargo clippy` before committing
- Test that the code compiles with different feature combinations:
  ```bash
  cargo build --all-features
  cargo test --all-features
  ```

### Additional Tools

The development shell includes:
- `cargo-audit` - security auditing
- `cargo-expand` - macro expansion
- `cargo-outdated` - dependency updates
- `cargo-udeps` - unused dependencies
- `tokio-console` - async debugging
- `clippy`, `rustfmt` - linting and formatting
- `editorconfig-checker` - editor config validation
- `just` - task runner

## atticadm Commands

The server binary includes administration utilities:

### make-token

Generate JWT tokens for authentication:

```bash
atticadm make-token --sub "alice" --validity "2y" --pull "prod" --push "dev-*"
```

### scrub-storage

Verify that data referenced in the database actually exists in S3 storage. This helps detect:
- Missing chunks (upload failed but DB has the record)
- Size mismatches (corrupted or partially uploaded files)

```bash
# Check both NARs and chunks (default)
atticadm scrub-storage

# Check only NAR completeness
atticadm scrub-storage --type nar

# Check only chunk storage
atticadm scrub-storage --type chunk
```
