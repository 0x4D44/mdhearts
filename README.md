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
- `mdhearts.exe --explain-pass-once <seed> <seat>` (prints the 3 chosen pass cards for that snapshot)
- `mdhearts.exe --explain-pass-batch <seat> <seed_start> <count>` (prints hand and 3 chosen pass cards across many seeds)
  - Both `--explain-once` and `--explain-batch` accept an optional `[difficulty]` argument (`easy|normal|hard`).
- `mdhearts.exe --compare-once <seed> <seat>` (runs Normal and Hard explain for the same snapshot and prints top choices and Hard stats)
- `mdhearts.exe --compare-batch <seat> <seed_start> <count>` (prints CSV lines of Normal vs Hard top picks and Hard stats across many seeds)
- `mdhearts.exe --explain-json <seed> <seat> <path> [difficulty]` (writes a JSON file with candidates, difficulty, weights, and Hard stats)

Additional references:
- Win32 UI roadmap: `docs/WIN32_UI_PLAN.md`
- Snapshot CLI usage: `docs/CLI_TOOLS.md`


## Configuration
- `MDH_BOT_DIFFICULTY` (`easy`, `normal`, `hard`): controls AI play style. `normal` enables the new heuristic planner; `easy` retains the legacy logic.
- `hard` currently uses a shallow-search scaffold that orders by heuristic and considers the top-N branches (configurable with `MDH_HARD_BRANCH_LIMIT`).
- `MDH_HARD_TIME_CAP_MS` (default `10`): per-decision time cap for Hard’s candidate scanning; breaks early when exceeded.
- Hard continuation tuning (tiny weights):
  - `MDH_HARD_CONT_FEED_PERPEN` (default `60`): bonus per penalty point when current-trick rollout feeds the leader.
  - `MDH_HARD_CONT_SELF_CAPTURE_PERPEN` (default `80`): penalty per penalty point when rollout has us capture penalties.
  - `MDH_HARD_NEXTTRICK_SINGLETON` (default `25`): bonus per singleton non-hearts suit if we will lead the next trick (cap 3 suits).
  - `MDH_HARD_NEXTTRICK_HEARTS_PER` (default `2`): small per-heart bonus if hearts are broken and we lead next.
  - `MDH_HARD_NEXTTRICK_HEARTS_CAP` (default `10`): cap for the hearts component above.
- `MDH_HARD_NEXT_BRANCH_LIMIT` (default `3`): number of candidate leads to probe when we lead the next trick in Hard’s 2‑ply probe.
- `MDH_HARD_EARLY_CUTOFF_MARGIN` (default `300`): early cutoff guard in Hard; stops scanning candidates when the next base score cannot beat the best total even with this margin.
- `MDH_DEBUG_LOGS=1`: emits detailed AI decision output to DebugView for diagnostics.
- `MDH_CLI_POPUPS=1`: enable Windows message-box popups for CLI info/errors. By default, CLI prints to console only to avoid blocking automation.

### AI tuning (env weights)
When `MDH_DEBUG_LOGS=1` is enabled, the app prints active AI weights at startup and per-decision feature contributions. You can override some weights at runtime via environment variables (no rebuild required):

- `MDH_W_OFFSUIT_BONUS` (default `600`): bonus per penalty point when dumping off-suit while void.
- `MDH_W_CARDS_PLAYED` (default `10`): global pacing factor per card played.
- `MDH_W_EARLY_HEARTS_LEAD` (default `600`): cautious penalty for leading hearts early even if hearts are broken.
- `MDH_W_NEAR100_SELF_CAPTURE_BASE` (default `1300`): baseline penalty for capturing when own score ≥85.
- `MDH_W_NEAR100_SHED_PERPEN` (default `250`): bonus per penalty shed when own score ≥85.
- `MDH_W_HUNT_FEED_PERPEN` (default `800`): bonus per penalty fed to the current leader when hunting.
- `MDH_W_PASS_TO_LEADER_PENALTY` (default `1400`): passing-time penalty per penalty point when passing to the current leader.
- `MDH_W_LEADER_FEED_BASE` (default `120`): small base bonus per penalty fed to the current leader even below near-100 scenarios (planner-level bias).
- `MDH_W_NONLEADER_FEED_PERPEN` (default `1200`): penalty per penalty point when feeding a non-leader (discourages dumping QS to second place).
- `MDH_W_LEADER_FEED_GAP_PER10` (default `40`): per-penalty bonus added per 10 points of score gap vs. you when feeding the leader (caps at 30 gap).

Example (PowerShell):
```
$env:MDH_DEBUG_LOGS = "1"
$env:MDH_W_OFFSUIT_BONUS = "700"
cargo run -p hearts-app --bin mdhearts
```


## Release Notes
### Benches
- Optional criterion bench to gauge heuristic planner cost:
  - Run: `cargo bench -p hearts-app --bench heuristic_decision`
  - Measures `explain_candidates_for` across a few seeds/seats using stable snapshots.
  - Target guidance: normal heuristic decisions generally in single-digit microseconds on a typical desktop; aim to keep worst-case < 2–3ms.
- Optional bench for Hard planner (Stage 3 scaffold):
  - Run: `cargo bench -p hearts-app --bench hard_decision`
  - Set `MDH_HARD_BRANCH_LIMIT` to explore performance vs. branch width; keep typical decisions < 20–30ms.

### 1.0.1
- New heuristic bot system with configurable difficulty levels.
- Improved Win32 UI polish (HUD placement, card animations, sharper rendering).
- Added comprehensive bot/unit tests and scripted round regression coverage.
- Documented configuration flags for debugging and AI tuning.


## Packaging
- Build the release binary: `cargo build --release`
- Run the installer script (requires Inno Setup): `iscc installers\Hearts.iss`
- Output setup executable is written to `installers/MDHearts-1.0.1-Setup.exe`.

