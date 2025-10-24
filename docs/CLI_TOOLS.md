# Developer CLI Commands

Run `mdhearts.exe` (or `cargo run -p hearts-app --bin mdhearts --`) with the following options:

- `--export-snapshot <path> [seed] [seat]`
  - Example: `mdhearts.exe --export-snapshot snapshots/test.json 123 east`
  - Creates directories as needed and writes a deterministic snapshot using the given seed and starting seat.
- `--import-snapshot <path>`
  - Loads the snapshot, restores the match state, prints details to stdout, and shows a message box for quick confirmation.
- `--show-weights`
  - Prints Normal and Hard weight summaries (respects env overrides).
  - Optional: `--out <path>` writes the summary to a file.
- `--explain-once <seed> <seat> [difficulty]`
  - Explains the current decision for the given seat and seed. `difficulty` may be `easy|normal|hard`.
- `--explain-batch <seat> <seed_start> <count> [difficulty]`
  - Repeats explain across a range of seeds for one seat.
- `--explain-snapshot <path> <seat>`
  - Explains from a previously exported snapshot file.
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
- `--match-batch <seat> <seed_start> <count> [difficultyA difficultyB] [--out <path>] [Hard flags]`
  - Simulates one round per seed twice (A vs B difficulties) and emits CSV lines:
    `seed,seat,diffA,diffB,a_pen,b_pen,delta` where `delta=b_pen-a_pen`.
  - Defaults: difficultyA=normal, difficultyB=hard. Append Hard flags to control Hard determinism/time caps.
  - Example: `--match-batch west 1000 50 normal hard --out designs/tuning/match_west_1000_50.csv`
 - `--match-mixed-file <seat> <mix> --seeds-file <path> [--out <path>] [Hard flags]`
   - Runs mixed-seat evaluations using a seed file. `<mix>` is 4 characters (N,E,S,W) using `e|n|h`.
   - Example: `--match-mixed-file west nnhh --seeds-file designs/tuning/seeds_example.txt --out designs/tuning/mixed_seeds_west.csv`

Helper scripts (deterministic evaluation)
- PowerShell: `powershell -ExecutionPolicy Bypass -File tools/run_eval.ps1 -Verbose`
- Bash/*nix: `bash tools/run_eval.sh`
  - Both write timestamped CSVs under `designs/tuning/` and a summary Markdown `eval_summary_<timestamp>.md`.
  - Parameters/ranges can be adjusted (PowerShell via parameters; Bash via env vars like `SEAT_START_WEST`, `COUNT_WEST`, etc.).

## Hard (FutureHard) flags (optional)

You can append the following flags to supported commands (explain/compare/json) to control Hard behavior without setting environment variables:

- `--hard-deterministic` — enable deterministic, step-capped scanning (for stable tests/logs)
- `--hard-steps <n>` — step cap used when deterministic mode is on
- `--hard-phaseb-topk <k>` — compute continuation only for top-K candidates (base-only beyond K)
- `--hard-branch-limit <n>` — number of top candidates to consider
- `--hard-next-branch-limit <n>` — number of next-trick leads to probe
- `--hard-time-cap-ms <ms>` — wall-clock cap per decision (when not deterministic)
- `--hard-cutoff <margin>` — early cutoff margin against next base
- `--hard-cont-boost-gap <n>` — apply continuation boost when base is within this gap (default off)
- `--hard-cont-boost-factor <n>` — multiplicative boost to continuation under the gap
- `--hard-verbose` — print continuation part breakdown on console (requires `MDH_DEBUG_LOGS=1`)

Additional env-gated toggles (advanced)
- `MDH_HARD_NEXT3_ENABLE` — enable a minimal third-opponent branch in the next-trick probe (default off)
- `MDH_HARD_PROBE_AB_MARGIN` — small alpha-like margin for next-trick probe; prunes deeper replies once local branch = margin (choose only; explain unchanged)

Tiering and leverage (advanced; env-gated)
- `MDH_HARD_TIERS_ENABLE=1` — enable leverage-based tiering of limits (defaults off).
- `MDH_HARD_TIERS_DEFAULT_ON_HARD=1` — auto-enable tiering only when difficulty=Hard (FutureHard) without setting the global flag.
- `MDH_HARD_LEVERAGE_THRESH_NARROW` (default 20), `MDH_HARD_LEVERAGE_THRESH_NORMAL` (default 50)
  - Leverage score maps to tiers: Narrow (<NARROW), Normal ([NARROW, NORMAL)), Wide (>= NORMAL).
  - Per-tier defaults (overridable by flags/env):
    - PhaseB top-K: Narrow=4, Normal=6, Wide=8
    - Next-trick probe M: Narrow=1, Normal=2, Wide=3
    - AB margin: Narrow=100, Normal=150, Wide=200

Hard stats output
- When `MDH_DEBUG_LOGS=1`, `--explain-once` prints:
  - `hard-stats: scanned=<n> elapsed=<ms>`
  - `tier=<Tier> leverage=<0-100> util=<0-100%> limits: topk=<k> nextM=<m> ab=<margin>`
  - Stats also populate `last_stats()` and JSON explain outputs.

Promoted defaults (advanced; env-gated)
- `MDH_HARD_PROMOTE_DEFAULTS=1` — apply small Hard-oriented default promotions:
  - Slightly higher continuation weights (feed/self) and a small continuation cap (˜250) when env weights are not set.
  - Auto-enables tiering for Hard difficulty only (equivalent to `MDH_HARD_TIERS_DEFAULT_ON_HARD=1`).

Notes:
- For reproducible CSVs or goldens, prefer `--hard-deterministic --hard-steps <n>`.
- The CLI prints a one-line summary of active Hard flags (hard-flags: …) for explain/compare commands.

Seats accept `north`, `east`, `south`, `west` (or `n/e/s/w`). Seed defaults to `0` when omitted.

Tips:
- Set `MDH_DEBUG_LOGS=1` to include per-candidate details and Hard stats; pair with `--hard-verbose` to show continuation parts.
- CLI popups are disabled by default; set `MDH_CLI_POPUPS=1` to enable.
  - For automation and CI, keep `MDH_CLI_POPUPS` unset to avoid blocking message boxes.

Hard continuation tuning (env; optional):
- `MDH_HARD_QS_RISK_PER` — small negative when leading next while holding A?
- `MDH_HARD_CTRL_HEARTS_PER` — small positive per heart when we lead next and hearts are broken
- `MDH_HARD_CTRL_HANDOFF_PEN` — small negative when we won’t lead next
- `MDH_HARD_CONT_CAP` — symmetric cap on total continuation magnitude
- `MDH_HARD_NEXT3_ENABLE` — enable a minimal third-opponent branch in next-trick probe (default off)
- `MDH_HARD_MOON_RELIEF_PERPEN` — small positive per penalty when we win the trick and moon state is Considering/Committed
- `MDH_HARD_PROBE_AB_MARGIN` — small alpha-like margin for next-trick probe; prunes deeper replies once local branch = margin (choose only; explain unchanged)
- `MDH_HARD_CONT_SCALE_FEED_PERMIL` — per-mille per penalty scaling added to continuation when feeding the leader (default 0)
- `MDH_HARD_CONT_SCALE_SELFCAP_PERMIL` — per-mille per penalty scaling added to continuation when self-capturing penalties (default 0)
- `MDH_HARD_WIDE_PERMIL_BOOST_FEED` — additional per-mille boost applied only in Wide tier to feed continuation (default 0)
- `MDH_HARD_WIDE_PERMIL_BOOST_SELFCAP` — additional per-mille boost applied only in Wide tier to self-capture continuation magnitude (default 0)
- `MDH_HARD_DET_ENABLE` - enable determinized averaging for current-trick rollout in Hard (default 0/off)
- `MDH_HARD_DET_SAMPLE_K` - number of samples to average when determinization is enabled (default 0 => no extra samples)
- `MDH_HARD_DET_TIME_MS` - optional display/telemetry knob for determinization phase budget; overall planner budget still governed by `MDH_HARD_TIME_CAP_MS`
 - `MDH_HARD_DET_PROBE_WIDE_LIKE` - widen next-trick probe limit by a small amount during determinized runs (choose-only)
 - `MDH_HARD_DET_NEXT3_ENABLE` - allow minimal third-opponent branching during next-trick probe in determinized runs (choose-only)
- `MDH_HARD_DET_DEFAULT_ON` - enable determinization by default for Hard choose paths (explain remains deterministic)
- `MDH_HARD_WIDE_PHASEB_TOPK` - override PhaseB top-K only when tier=Wide (otherwise use global/default)
- `MDH_HARD_WIDE_NEXT_BRANCH_LIMIT` - override next-trick probe M only when tier=Wide
- `MDH_HARD_NEXT3_TINY_NORMAL` - enable tiny minimal next3 branching under Normal tier (choose-only; env-gated)

Hard defaults gate (example)
- To enable a conservative Hard default with determinization on choose paths only:
  - PowerShell: `$env:MDH_HARD_DET_DEFAULT_ON = "1"; $env:MDH_HARD_DET_SAMPLE_K = "3"; $env:MDH_HARD_DETERMINISTIC = "1"; $env:MDH_HARD_TEST_STEPS = "120"`
  - Linux/macOS: `MDH_HARD_DET_DEFAULT_ON=1 MDH_HARD_DET_SAMPLE_K=3 MDH_HARD_DETERMINISTIC=1 MDH_HARD_TEST_STEPS=120`
- Explain commands remain deterministic; choose uses determinization when Hard is active.

Limits precedence
- Wide-tier-only overrides (`MDH_HARD_WIDE_PHASEB_TOPK`, `MDH_HARD_WIDE_NEXT_BRANCH_LIMIT`) apply when Tier::Wide is selected and the corresponding global envs are not set.
- If global envs (`MDH_HARD_PHASEB_TOPK`, `MDH_HARD_NEXT_BRANCH_LIMIT`) are set, they take precedence for all tiers.

Planner-level nudges (env; optional; defaults off)
- `MDH_HARD_PLANNER_NUDGES=1` — enable tiny planner-level nudges in Normal planner.
- `MDH_HARD_PLANNER_LEADER_FEED_NUDGE` — per-penalty bonus when feeding the score leader on a penalty trick under a small-base guard.
- `MDH_HARD_PLANNER_SELF_CAPTURE_NUDGE` — per-penalty extra penalty when we’d capture penalties and our score = 85.
- `MDH_HARD_PLANNER_MAX_BASE_FOR_NUDGE` — guard threshold for existing per-penalty leader-feed base; nudge applies only if below this.
