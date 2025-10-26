# CLI Tools Addendum (Hard Endgame knobs)

This addendum lists additional environment toggles for the Hard (FutureHard) planner that are not present in `docs/CLI_TOOLS.md` due to an encoding limitation in that file.

- `MDH_HARD_ENDGAME_DP_ENABLE` — Enable the tiny endgame micro-solver (choose-only). The micro-solver activates when all seats have ≤ N cards, simulates the remaining ≤ N tricks deterministically with the canonical void-aware follow-ups, and applies a small continuation signal favoring feeding the score leader and avoiding self-capture. Default: off.
- `MDH_HARD_ENDGAME_MAX_CARDS` — Maximum cards per hand to trigger the micro-solver. Default: `3`.

Notes
- The micro-solver only affects the Hard choose path; explain remains deterministic and unchanged.
- The micro-solver’s continuation contribution is subject to the global Hard continuation cap (`MDH_HARD_CONT_CAP`).

Additional CSV stats (mixed/match commands)
- Both mixed-seat and A/B match commands accept an optional `--stats` flag to include Hard planner stats in CSV outputs:
  - Columns: `scanned`, `elapsed_ms`, `dp_hits` (from the most recent Hard decision).
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
