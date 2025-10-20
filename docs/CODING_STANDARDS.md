# Coding Standards

_Last updated: 24 September 2025_

- **Rust edition:** Target Rust 1.81+ with the 2024 edition across all crates (`Cargo.toml` already set).
- **Workspace layout:**
  - `hearts-core` – deterministic rules/AI logic, pure Rust, no platform bindings.
  - `hearts-ui` – view models, theme metadata, presentation abstractions (no Win32/WinUI APIs directly).
  - `hearts-app` – platform entry point, Windows-specific interop (`windows` crate) and bootstrapping.
- **Tooling:** Run `cargo fmt --all` and `cargo clippy --workspace -- -D warnings` before pushing. CI enforces both plus `cargo test`.
- **Safety:** Keep unsafe blocks isolated in `hearts-app::platform` modules; document invariants and prefer safe abstractions in shared crates.
- **Testing:**
  - Core engine changes require unit tests in `hearts-core`.
  - UI/state view model logic should include integration tests where feasible (e.g., state transitions).
- **Dependencies:** Use workspace-wide versions; review licensing when importing assets or crates. Reuse `mdsol` assets via scripts checked into `tools/` (TBD in later phase).
- **Code review checklist:** API docs updated, public structs/enums documented, error handling uses `Result<T>` with descriptive variants, no `unwrap()` in production paths.
