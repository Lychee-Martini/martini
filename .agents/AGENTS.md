# Project Rules

## Rust CI Invariant Rules
- Before declaring any task complete or committing changes, always run the repository's CI checks:
  1. Code Formatting: `cargo fmt --all -- --check` (or execute `cargo fmt` to automatically resolve formatting mismatches).
  2. Clippy compiler lints: `cargo clippy --all-targets -- -D warnings` to verify code is free from lints or compilation warnings.
  3. Testing: `cargo test --all-targets` to run all unit, integration, and doc tests.
