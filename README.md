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
