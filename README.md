# mdhearts

Modern Rust revival of the classic Microsoft Hearts experience.

## Getting Started
1. Install Rust stable (`x86_64-pc-windows-msvc`).
2. Follow `docs/SETUP_WINDOWS.md` for Win32 build prerequisites.
3. Card art: the card atlas (`assets/cards.png`) and layout JSON (`assets/cards.json`) live in `assets/`.

## Useful Commands
- `cargo run -p hearts-app --bin mdhearts`
- `mdhearts.exe --export-snapshot snapshots/test.json [seed] [seat]`
- `mdhearts.exe --import-snapshot snapshots/test.json`

Additional references:
- Win32 UI roadmap: `docs/WIN32_UI_PLAN.md`
- Snapshot CLI usage: `docs/CLI_TOOLS.md`


## Configuration
- `MDH_BOT_DIFFICULTY` (`easy`, `normal`, `hard`): controls AI play style. `normal` enables the new heuristic planner; `easy` retains the legacy logic.
- `MDH_DEBUG_LOGS=1`: emits detailed AI decision output to DebugView for diagnostics.

## Testing
- **Windows (MSVC toolchain)**: `cargo test --workspace`
- **Non-Windows hosts (e.g., WSL/Linux)**: skip the Win32 launcher crate, which depends on COM marshalling support unavailable outside Windows:
  ```bash
  cargo test --workspace --exclude hearts-app
  ```
  All core logic (rules engine, telemetry, bots, benchmarks) continues to compile and run cross-platform.


## Release Notes
### 1.0.1
- New heuristic bot system with configurable difficulty levels.
- Improved Win32 UI polish (HUD placement, card animations, sharper rendering).
- Added comprehensive bot/unit tests and scripted round regression coverage.
- Documented configuration flags for debugging and AI tuning.


## Packaging
- Build the release binary: `cargo build --release`
- Run the installer script (requires Inno Setup): `iscc installers\Hearts.iss`
- Output setup executable is written to `installers/MDHearts-1.0.1-Setup.exe`.

