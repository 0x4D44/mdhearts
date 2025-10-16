# Repository Guidelines

## Project Structure & Module Organization
- `crates/hearts-core`: deterministic rules engine and AI heuristics; keep it platform-agnostic and well covered by unit tests.
- `crates/hearts-ui`: presentation models and theming metadata shared across front ends.
- `crates/hearts-app`: Windows launcher (`mdhearts` bin), Win32 interop, and integration tests under `tests/`.
- Support directories: `assets/` for card art, `docs/` for design notes and standards, `python/` for RL training scripts, `tools/` for checkpoint and export utilities.

## Build, Test, and Development Commands
- `cargo run -p hearts-app --bin mdhearts`: launch the desktop client; pass `--export-snapshot` or `--import-snapshot` per `README.md` examples.
- `cargo test --workspace`: execute Rust unit/integration suites; use `--package hearts-core` when iterating on rules logic.
- `cargo fmt --all` and `cargo clippy --workspace -- -D warnings`: required before pushing; CI enforces both.
- `pip install -r python/requirements.txt` then `python test_e2e.py`: validate the RL pipeline end-to-end; outputs pass/fail summary to the console.

## Coding Style & Naming Conventions
- Target Rust 2024 edition (Rust 1.81+); rely on `rustfmt` defaults (4-space indent, trailing commas) and resolve all `clippy` warnings.
- Use `snake_case` for modules/functions, `UpperCamelCase` for types, and document public APIs; avoid `unwrap` in production paths.
- Isolate any `unsafe` to `hearts-app::platform` modules and describe invariants inline when unavoidable.
- Keep workspace dependencies aligned; prefer workspace versions already declared.

## Testing Guidelines
- Extend `crates/hearts-core` unit tests for rules or scoring changes; add scenario fixtures beside the code they exercise.
- For UI/view-model flows, add integration coverage in `crates/hearts-app/tests` to protect regressions in menu/state transitions.
- Run the Python RL checks (`python test_e2e.py`) whenever touching `python/hearts_rl` or reward shaping configs; TensorBoard logs write to `runs/`.
- Gate merges on a clean `cargo test --workspace` run; include focused regression tests when fixing bugs.

## Commit & Pull Request Guidelines
- Follow the repository history: short, imperative commit subjects (e.g. “Add Gen4 BC regularization training”).
- Squash incidental noise before opening PRs; ensure commits compile and pass formatting/lints.
- PRs should outline the change’s intent, note affected crates, link tracking issues, and attach UI screenshots or CLI output when user-facing behavior shifts.
- Confirm configuration updates (`MDH_BOT_DIFFICULTY`, `MDH_DEBUG_LOGS`) and docs in `docs/` stay in sync with code.

## Configuration & Diagnostics
- Key runtime switches live in environment variables (`MDH_BOT_DIFFICULTY`, `MDH_DEBUG_LOGS=1`) and apply to both debug and release builds.
- Snapshot workflows live under `docs/CLI_TOOLS.md`; use `mdhearts.exe --export-snapshot` before risky migrations.
- RL training outputs large artifacts into `gen4_*` and `runs/`; clean or relocate locally before committing.
