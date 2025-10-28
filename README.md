# mdhearts

Modern Rust revival of the classic Microsoft Hearts experience.

CI: see GitHub Actions workflow in `.github/workflows/ci.yml` (builds/tests on Windows/Linux; PR eval smoke).

## Getting Started
1. Install Rust stable (`x86_64-pc-windows-msvc`).
2. Follow `docs/SETUP_WINDOWS.md` for Win32 build prerequisites.
3. Card art: the card atlas (`assets/cards.png`) and layout JSON (`assets/cards.json`) live in `assets/`.

## Useful Commands
- `cargo run -p hearts-app --bin mdhearts`
- `mdhearts.exe --export-snapshot snapshots/test.json [seed] [seat]`
- `mdhearts.exe --import-snapshot snapshots/test.json`
- `mdhearts.exe --show-weights` (prints active AI weights)
- `mdhearts.exe --show-hard-telemetry [--out <path>]` (writes the latest Hard decision telemetry to NDJSON and prints summary aggregates)
- `mdhearts.exe --explain-once <seed> <seat>` (prints candidate scores at first decision for that seat)
- `mdhearts.exe --explain-batch <seat> <seed_start> <count>` (prints candidates across a range of seeds)
- `mdhearts.exe --explain-snapshot <path> <seat>` (prints candidates for a seat from a saved snapshot)
- `mdhearts.exe --explain-pass-once <seed> <seat>` (prints the 3 chosen pass cards for that snapshot)
- `mdhearts.exe --explain-pass-batch <seat> <seed_start> <count>` (prints hand and 3 chosen pass cards across many seeds)
  - Both `--explain-once` and `--explain-batch` accept an optional `[difficulty]` argument (`easy|normal|hard`).
- `mdhearts.exe --compare-once <seed> <seat>` (runs Normal and Hard explain for the same snapshot and prints top choices and Hard stats)
- `mdhearts.exe --compare-batch <seat> <seed_start> <count> [--out <path>] [--only-disagree]` (prints CSV lines of Normal vs Hard top picks and Hard stats; `--out` writes to file, `--only-disagree` filters to disagreements)
- `mdhearts.exe --explain-json <seed> <seat> <path> [difficulty]` (writes a JSON file with candidates, difficulty, weights, and Hard stats)
- `mdhearts.exe --bench-check <difficulty> <seat> <seed_start> <count> [Hard flags]` (quick perf stats: avg and p95 µs over a seed range)
- `mdhearts.exe --match-batch <seat> <seed_start> <count> [difficultyA difficultyB] [--out <path>] [Hard flags]` (simulate one round per seed with two difficulties and emit CSV of penalties for the given seat)
- `mdhearts.exe --match-batch <seat> <seed_start> <count> [difficultyA difficultyB] [--out <path>] [Hard flags]` (simulate one round per seed with two difficulties and emit CSV of penalties for the given seat)
- `mdhearts.exe --match-mixed-file <seat> <mix> --seeds-file <path> [--out <path>] [Hard flags]` (run mixed-seat evaluations using a seed file; `<mix>` is 4 chars for N,E,S,W using `e|n|h`)
- `mdhearts.exe --export-play-dataset <seat> <seed_start> <count> <difficulty> <out> [Hard flags]` (stream NDJSON records with candidates, continuation parts, beliefs, and adviser bias for downstream analysis)

Hard (FutureHard) flags (for explain/compare/json)
- Append flags to control Hard without env vars:
  - `--hard-deterministic`, `--hard-steps <n>`, `--hard-phaseb-topk <k>`, `--hard-branch-limit <n>`, `--hard-next-branch-limit <n>`, `--hard-time-cap-ms <ms>`, `--hard-cutoff <margin>`, `--hard-cont-boost-gap <n>`, `--hard-cont-boost-factor <n>`, `--hard-verbose`
- CLI prints a one-line `hard-flags:` summary for quick visibility.

Advanced (env): tiering and stats
- `MDH_HARD_TIERS_ENABLE=1` - enable leverage-based tiering of Hard limits (defaults off).
- Thresholds: `MDH_HARD_LEVERAGE_THRESH_NARROW` (default 20), `MDH_HARD_LEVERAGE_THRESH_NORMAL` (default 50).
- `MDH_HARD_BELIEF_CACHE_SIZE=<n>` - capacity of the Hard-mode belief cache (default 128 entries).
- `MDH_HARD_TELEMETRY_KEEP=<n>` - maximum number of telemetry exports retained on disk (default 20).
- `MDH_HARD_BELIEF_TOPK=<n>` - number of highest-probability cards to prioritize when sampling void follow-ups (default 3).
- `MDH_HARD_BELIEF_DIVERSITY=<n>` - additional candidates beyond top-K to allow diversity sampling (default 2).
- `MDH_HARD_BELIEF_FILTER=1` - drop zero-probability cards from sampling pools (default off).
- `MDH_HARD_ADVISER_PLAY=1` - enable adviser bias application for Hard candidates (defaults off). Use `MDH_ADVISER_PLAY_PATH=<path>` to point to a JSON file (defaults to `assets/adviser/play.json`) mapping card strings such as `"QC"` to bias values.
- MDH_HARD_PLANNER_LEADER_FEED_NUDGE=<n> - per-penalty planner nudge for Hard when feeding a unique score leader on a penalty trick (defaults to 12). Guarded by MDH_HARD_PLANNER_MAX_BASE_FOR_NUDGE (default 220), MDH_HARD_PLANNER_NUDGE_NEAR100 (default 90), and MDH_HARD_PLANNER_NUDGE_GAP_MIN (default 4).
- With `MDH_DEBUG_LOGS=1`, `--explain-once` prints extended Hard stats (tier, leverage, utilization, and effective limits).

Additional references:
- Win32 UI roadmap: `docs/WIN32_UI_PLAN.md`
- Snapshot CLI usage: `docs/CLI_TOOLS.md`

Hard Defaults Gate (choose-only)
- Enable Hard determinization by default (choose paths only) to stabilize and slightly deepen Hard without affecting explain output:
  - PowerShell: `$env:MDH_HARD_DET_DEFAULT_ON = "1"; $env:MDH_HARD_DET_SAMPLE_K = "3"; $env:MDH_HARD_DETERMINISTIC = "1"; $env:MDH_HARD_TEST_STEPS = "120"`
  - Then run mixed-seat evals, e.g.: `mdhearts --match-mixed west 1000 200 nnhh --out designs/tuning/mixed_nnhh_demo.csv`
  - Explain paths remain deterministic; choose uses determinization when Hard is active.
- Tuning quickstart: `designs/tuning/2025-10-22 - Tuning Quickstart.md`
- Tuning artifacts index: `designs/2025.10.22 - Tuning Artifacts Index.md`
- Evaluation & stability plan: `designs/2025.10.22 - Stage 6 (Evaluation & Stability) Plan.md`
- AI tuning contributor guide: `docs/CONTRIBUTING_AI_TUNING.md`
- Designs folder index: `designs/INDEX.md`

## Deterministic Evaluation (HOWTO)
- For a quick, reproducible sweep across seats, use the helper script:
  - PowerShell: `powershell -ExecutionPolicy Bypass -File tools/run_eval.ps1 -Verbose`
  - The script sets deterministic Hard flags (`MDH_HARD_DETERMINISTIC=1`, `MDH_HARD_TEST_STEPS=<n>`) and runs:
    - `--compare-batch` with `--only-disagree` per seat
    - `--match-batch` Normal vs Hard per seat
  - Outputs are timestamped CSVs under `designs/tuning/` and a single summary Markdown `designs/tuning/eval_summary_<timestamp>.md`.
  - Adjust ranges via parameters (defaults: West 1000..1149, South 1080..1229, East 2000..2149, North 1100..1299); see script header.
  - Tip: keep `MDH_CLI_POPUPS` unset so results go to console/files (no message boxes).


## Configuration
- `MDH_BOT_DIFFICULTY` (`easy`, `normal`, `hard`): controls AI play style. `normal` enables the new heuristic planner; `easy` retains the legacy logic.
- `hard` currently uses a shallow-search scaffold that orders by heuristic and considers the top-N branches (configurable with `MDH_HARD_BRANCH_LIMIT`).
- `MDH_HARD_TIME_CAP_MS` (default `10`): per-decision time cap for Hard’s candidate scanning; breaks early when exceeded.
- `MDH_HARD_DETERMINISTIC` (default `off`): when enabled, uses a deterministic step-capped budget instead of wall-clock timing for Hard scanning (stable tests/logs).
- `MDH_HARD_TEST_STEPS` (no default): optional step cap used when deterministic mode is on.
- `MDH_HARD_DET_DEFAULT_ON` (default `off`): enable determinization by default for Hard choose paths (explain remains deterministic). Pair with `MDH_HARD_DET_SAMPLE_K` and related flags as needed.
- Hard continuation tuning (tiny weights):
  - `MDH_HARD_CONT_FEED_PERPEN` (default `60`): bonus per penalty point when current-trick rollout feeds the leader.
  - `MDH_HARD_CONT_SELF_CAPTURE_PERPEN` (default `80`): penalty per penalty point when rollout has us capture penalties.
  - `MDH_HARD_NEXTTRICK_SINGLETON` (default `25`): bonus per singleton non-hearts suit if we will lead the next trick (cap 3 suits).
  - `MDH_HARD_NEXTTRICK_HEARTS_PER` (default `2`): small per-heart bonus if hearts are broken and we lead next.
  - `MDH_HARD_NEXTTRICK_HEARTS_CAP` (default `10`): cap for the hearts component above.
  - `MDH_HARD_CONT_BOOST_GAP` (default `0`=off): when > 0, apply a multiplicative boost to the continuation component for candidates whose base is within this gap of the top base.
  - `MDH_HARD_CONT_BOOST_FACTOR` (default `1`=no boost): multiplicative factor applied to continuation when the above gap condition holds.
  - `MDH_HARD_QS_RISK_PER` (default `0`=off): small negative when we’ll lead next and still hold A♠ (QS exposure risk).
  - `MDH_HARD_CTRL_HEARTS_PER` (default `0`=off): small positive per heart when we’ll lead next and hearts are broken.
  - `MDH_HARD_CTRL_HANDOFF_PEN` (default `0`=off): small negative when we will not lead next (loss of initiative).
  - `MDH_HARD_CONT_CAP` (default `0`=off): symmetric cap on total continuation magnitude to keep effects tiny.
  - `MDH_HARD_MOON_RELIEF_PERPEN` (default `0`=off): small positive per penalty when we win a trick and moon state is Considering/Committed.
- `MDH_HARD_NEXT_BRANCH_LIMIT` (default `3`): number of candidate leads to probe when we lead the next trick in Hard’s 2‑ply probe.
- `MDH_HARD_EARLY_CUTOFF_MARGIN` (default `300`): early cutoff guard in Hard; stops scanning candidates when the next base score cannot beat the best total even with this margin.
- `MDH_HARD_PHASEB_TOPK` (default `0`): compute continuation only for top‑K candidates; candidates beyond K use base‑only (monotonic fallback under budget).
- `MDH_HARD_NEXT3_ENABLE` (default `off`): enable a minimal third‑opponent branch in the next‑trick probe.

Head‑to‑head (match) evaluation
- Use `--match-batch` to compare two difficulties across a seed range for a specific seat. Columns: `seed,seat,diffA,diffB,a_pen,b_pen,delta` where `delta=b_pen-a_pen`.
- Example (console): `mdhearts --match-batch west 1000 50 normal hard --hard-deterministic --hard-steps 80`
- Example (to file): `mdhearts --match-batch east 2000 100 normal hard --out designs/tuning/match_east_2000_100.csv`

Moon tuning (env)
- `MDH_MOON_COMMIT_MAX_CARDS` (default `20`): max cards played in round to consider committing to moon.
- `MDH_MOON_COMMIT_MAX_SCORE` (default `70`): max current score to consider committing.
- `MDH_MOON_COMMIT_MIN_TRICKS` (default `2`): minimum tricks won before commit consideration (bot will mark Considering after the first clean control trick, and Commit when this threshold is met).
- `MDH_MOON_COMMIT_MIN_HEARTS` (default `5`): minimum hearts in hand before commit consideration.
- `MDH_MOON_COMMIT_MIN_CONTROL` (default `3`): minimum high hearts (≥10) before commit consideration.
- `MDH_MOON_ABORT_OTHERS_HEARTS` (default `3`): abort if opponents have collected at least this many hearts total.
- `MDH_MOON_ABORT_NEAREND_CARDS` (default `36`): abort when at or beyond this many cards played (near end of round).
- `MDH_MOON_ABORT_MIN_HEARTS_LEFT` (default `3`): abort if we have fewer hearts than this.
- `MDH_MOON_ABORT_LOST_CONTROL` (default `true`): abort when we fail to capture a clean trick while attempting moon.
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

- Flags helpful for tuning/inspection:
  - `--show-weights [--out <path>]` — print or write Normal/Hard weights summary (respects env overrides)
  - `--hard-verbose` — with `MDH_DEBUG_LOGS=1`, print continuation part breakdown for Hard in explain commands
- Evaluation helper script
  - tools/run_eval.ps1 — run deterministic compare/match across all seats; writes timestamped CSVs and a single summary Markdown.
