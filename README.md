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
- `mdhearts.exe --show-weights` (prints active AI weights)
- `mdhearts.exe --explain-once <seed> <seat>` (prints candidate scores at first decision for that seat)
- `mdhearts.exe --explain-batch <seat> <seed_start> <count>` (prints candidates across a range of seeds)
- `mdhearts.exe --explain-snapshot <path> <seat>` (prints candidates for a seat from a saved snapshot)

Additional references:
- Win32 UI roadmap: `docs/WIN32_UI_PLAN.md`
- Snapshot CLI usage: `docs/CLI_TOOLS.md`


## Configuration
- `MDH_BOT_DIFFICULTY` (`easy`, `normal`, `hard`): controls AI play style. `normal` enables the new heuristic planner; `easy` retains the legacy logic.
- `MDH_DEBUG_LOGS=1`: emits detailed AI decision output to DebugView for diagnostics.

### AI tuning (env weights)
When `MDH_DEBUG_LOGS=1` is enabled, the app prints active AI weights at startup and per-decision feature contributions. You can override some weights at runtime via environment variables (no rebuild required):

- `MDH_W_OFFSUIT_BONUS` (default `600`): bonus per penalty point when dumping off-suit while void.
- `MDH_W_CARDS_PLAYED` (default `10`): global pacing factor per card played.
- `MDH_W_EARLY_HEARTS_LEAD` (default `600`): cautious penalty for leading hearts early even if hearts are broken.
- `MDH_W_NEAR100_SELF_CAPTURE_BASE` (default `1300`): baseline penalty for capturing when own score ≥85.
- `MDH_W_NEAR100_SHED_PERPEN` (default `250`): bonus per penalty shed when own score ≥85.
- `MDH_W_HUNT_FEED_PERPEN` (default `800`): bonus per penalty fed to the current leader when hunting.
- `MDH_W_PASS_TO_LEADER_PENALTY` (default `1400`): passing-time penalty per penalty point when passing to the current leader.

Example (PowerShell):
```
$env:MDH_DEBUG_LOGS = "1"
$env:MDH_W_OFFSUIT_BONUS = "700"
cargo run -p hearts-app --bin mdhearts
```


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

