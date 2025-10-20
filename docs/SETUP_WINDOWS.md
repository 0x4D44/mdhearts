# Windows Development Setup

_Last updated: 25 September 2025_

## Required software
1. **Rust toolchain** targeting `stable-x86_64-pc-windows-msvc`.
2. **Visual Studio Build Tools 2022** (optional but recommended) with the "Desktop development with C++" workload for headers/libraries.
3. (Optional) **PowerShell 7** and **ImageMagick** if you plan to tinker with assets locally (not required).

## Environment configuration
1. Ensure `cargo`, `rustfmt`, and `clippy` are on `PATH` (`rustup component add rustfmt clippy`).
2. To build from a regular command prompt, initialize the MSVC environment once via `Developer PowerShell for VS` or `vcvars64.bat`.
3. Ensure `assets/cards.png` and `assets/cards.json` exist (they are checked in). No importer is required.

## Repository workflow
- `cargo fmt --all`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`
- `cargo run -p hearts-app --bin mdhearts`

## Notes on UI stack
The project targets a **standalone Win32/Direct2D** window rather than WinUI. No Windows App SDK runtime is required. The Win32 message loop, rendering, and input handling live in `crates/hearts-app/src/platform/`.

See also:
- `docs/CLI_TOOLS.md` – snapshot import/export commands.
- `docs/WIN32_UI_PLAN.md` – roadmap for the Win32/Direct2D UI implementation.
