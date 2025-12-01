# Developer CLI Commands

Run `mdhearts.exe` (or `cargo run -p hearts-app --bin mdhearts --`) with the following options:

- `--export-snapshot <path> [seed] [seat]`
  - Exports a full, restorable snapshot (hands, current trick, trick history, passing state, hearts_broken, scores/passing index) using the given seed/seat. Creates directories as needed.
- `--export-seed <path> [seed] [seat]`
  - Legacy seed-only export (recreates a fresh deal on import; not restorable mid-game).
- `--import-snapshot <path> [--legacy-ok]`
  - Restores a snapshot. Legacy seed-only files are rejected unless `--legacy-ok` is provided (then a new deal is created).
- `--show-weights`
  - Prints Normal and Hard weight summaries (respects env overrides).
  - Optional: `--out <path>` writes the summary to a file.
- `--explain-once <seed> <seat> [difficulty]`
  - Explains the current decision for the given seat and seed. `difficulty` may be `easy|normal|hard`.
- `--explain-batch <seat> <seed_start> <count> [difficulty]`
  - Repeats explain across a range of seeds for one seat.
- `--explain-snapshot <path> <seat>`
  - Restores the snapshot (full when present) and explains the given seat.
- `--explain-pass-once <seed> <seat>` / `--explain-pass-batch <seat> <seed_start> <count>`
  - Prints the 3-card pass decisions.
- `--compare-once <seed> <seat>`
  - Compares Normal vs Hard top selection and prints Hard stats (scanned, elapsed).
- `--compare-batch <seat> <seed_start> <count> [--out <path>] [--only-disagree]`
  - Emits CSV rows `seed,seat,normal_top,hard_top,agree,hard_scanned,hard_elapsed_ms`.
  - `--out <path>` writes to file; `--only-disagree` filters to rows where Normal and Hard differ.
- `--explain-json <seed> <seat> <path> [difficulty]`
  - Writes a JSON dump containing candidates, difficulty, weights, and (for hard) verbose candidate breakdown and stats.
  - Use `--hard-verbose` with explain commands to include continuation part breakdown on console when `MDH_DEBUG_LOGS=1`.
- `--match-batch <seat> <seed_start> <count> [difficultyA difficultyB] [--out <path>] [--telemetry-out <path>] [Hard flags]`
  - Simulates one round per seed twice (A vs B difficulties) and emits CSV lines: `seed,seat,diffA,diffB,a_pen,b_pen,delta` where `delta=b_pen-a_pen`.
  - Defaults: difficultyA=normal, difficultyB=hard (accepts `search`/`lookahead`). Append Hard flags to control Hard determinism/time caps.
  - `--telemetry-out <path>` writes the Hard telemetry sink (NDJSON).
- `--match-mixed <seat> <seed_start> <count> <mix> [--out <path>] [--telemetry-out <path>] [--stats] [Hard flags]`
  - Mixed-seat evaluation with inline seed ranges. `<mix>` is 4 characters (N,E,S,W) using `e|n|h|s` (Easy/Normal/Hard/Search).
- `--match-mixed-file <seat> <mix> --seeds-file <path> [--out <path>] [--telemetry-out <path>] [Hard flags]`
  - Runs mixed-seat evaluations using a seed file (same `e|n|h|s` syntax).

Helper scripts (deterministic evaluation)
- PowerShell: `powershell -ExecutionPolicy Bypass -File tools/run_eval.ps1 -Verbose`
- Bash/*nix: `bash tools/run_eval.sh`
  - Both write timestamped CSVs under `designs/tuning/` and a summary Markdown `eval_summary_<timestamp>.md`.
  - Parameters/ranges can be adjusted (PowerShell via parameters; Bash via env vars like `SEAT_START_WEST`, `COUNT_WEST`, etc.).
- Search vs Hard sweeps: `powershell -ExecutionPolicy Bypass -File tools/run_search_vs_hard.ps1 -VerifyTimeoutTelemetry`
- Mixed-seat sweeps: `powershell -ExecutionPolicy Bypass -File tools/run_search_vs_mixed.ps1 -Mixes shsh -ThinkLimitsMs @(5000,0)`

Notes:
- Seats accept `north`, `east`, `south`, `west` (or `n/e/s/w`).
- For reproducible CSVs or goldens, prefer `--hard-deterministic --hard-steps <n>`.
- CLI popups are disabled by default; set `MDH_CLI_POPUPS=1` to enable (automation/CI should keep it unset).