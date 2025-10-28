# CLI Tools Addendum (Hard Endgame knobs)

This addendum lists additional environment toggles for the Hard (FutureHard) planner that are not present in `docs/CLI_TOOLS.md` due to an encoding limitation in that file.

- `MDH_HARD_ENDGAME_DP_ENABLE` — Enable the tiny endgame micro-solver (choose-only). The micro-solver activates when all seats have ≤ N cards, simulates the remaining ≤ N tricks deterministically with the canonical void-aware follow-ups, and applies a small continuation signal favoring feeding the score leader and avoiding self-capture. Default: off.
- `MDH_HARD_ENDGAME_MAX_CARDS` — Maximum cards per hand to trigger the micro-solver. Default: `3`.

Notes
- The micro-solver only affects the Hard choose path; explain remains deterministic and unchanged.
- The micro-solver’s continuation contribution is subject to the global Hard continuation cap (`MDH_HARD_CONT_CAP`).

Additional CSV stats (mixed/match commands)
- Both mixed-seat and A/B match commands accept an optional `--stats` flag to include Hard planner stats in CSV outputs:
  - Columns: `scanned`, `elapsed_ms`, `dp_hits`, `nudge_hits` (from the most recent Hard decision).
  - Examples:
    - `--match-mixed <seat> <seed_start> <count> <mix> [--out <path>] [--stats] [Hard flags]`
    - `--match-mixed-file <seat> <mix> --seeds-file <path> [--out <path>] [--stats] [Hard flags]`
    - `--match-batch <seat> <seed_start> <count> [difficultyA difficultyB] [--out <path>] [--stats] [Hard flags]`
  - Use `MDH_HARD_DETERMINISTIC=1` with `MDH_HARD_TEST_STEPS=<n>` for stable, reproducible stats.

DP Flip tools
- `--seek-dp-flip <seat> <seed_start> <count> [--out <path>] [Hard flags]`
  - Scans seeds, simulates to the target seat with all hands ≤ 3 cards, and compares Hard choose() with DP OFF vs ON.
  - Writes CSV rows `seed,seat,legal,top_off,top_on` for flips.
  - Tip: set deterministic env and (optionally) endgame-only boosts to surface candidates faster during seeking.

- `--compare-dp-once <seed> <seat> [Hard flags]`
  - For a single seed/seat, simulates to a late endgame position and prints `dp-legal`, `dp-off`, `dp-on`.
  - Useful for quick inspection of a candidate before writing tests.

- `--export-endgame <seed> <seat> <out>`
  - Simulates to a late endgame state (≤ 3 cards/hand) and writes a JSON snapshot containing hands, leader, and hearts-broken flag to aid constructing minimal RoundState tests.

Telemetry command and retention
- `--show-hard-telemetry [--out <path>]`
  - Writes the accumulated Hard decision telemetry to NDJSON (default location `designs/tuning/telemetry/`) and prints summary aggregates (record count, average belief entropy, cache hit rate).
- `MDH_HARD_BELIEF_CACHE_SIZE=<n>` - sets the Hard belief cache capacity (default 128).
- `MDH_HARD_TELEMETRY_KEEP=<n>` - rotates telemetry exports, keeping the most recent `n` files (default 20).

- MDH_HARD_BELIEF_TOPK=<n> / MDH_HARD_BELIEF_DIVERSITY=<n> / MDH_HARD_BELIEF_FILTER=1 - configure Hard belief-sampler prioritisation (top-k emphasis, diversity depth, and zero-probability filtering).

Play dataset export
- `--export-play-dataset <seat> <seed_start> <count> <difficulty> <out> [Hard flags]`
  - Streams per-decision snapshots (seed, trick context, candidate list, continuation parts, adviser bias) to NDJSON for offline analysis or tuning.
  - Example: `mdhearts --export-play-dataset west 1000 50 hard designs/tuning/play_samples.ndjson`
  - Honours the usual Hard planner flags (`MDH_HARD_*`) plus CLI overrides parsed earlier in the command.
- Adviser bias toggles:
  - `MDH_HARD_ADVISER_PLAY=1` enables applying the loaded bias values during Hard candidate ranking.
  - `MDH_ADVISER_PLAY_PATH=<path>` points to a JSON file (defaults to `assets/adviser/play.json`) with entries like `"QC": 2500`.
- Planner nudge toggles:
  - `MDH_HARD_PLANNER_LEADER_FEED_NUDGE=<n>` - per-penalty planner nudge applied when feeding a unique score leader on a penalty trick (defaults to 12).
  - `MDH_HARD_PLANNER_MAX_BASE_FOR_NUDGE=<n>` - guard to skip the nudge when the existing base per-penalty leader-feed score exceeds this value (defaults to 220).
  - `MDH_HARD_PLANNER_NUDGE_NEAR100=<score>` - skip the nudge when the leader is at or above this score (defaults to 90).
  - `MDH_HARD_PLANNER_NUDGE_GAP_MIN=<gap>` - minimum score gap required for the nudge to apply (defaults to 4).
