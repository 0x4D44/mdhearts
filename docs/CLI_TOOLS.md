# Developer CLI Commands

Run `mdhearts.exe` (or `cargo run -p hearts-app --bin mdhearts --`) with the following options:

- `--export-snapshot <path> [seed] [seat]`
  - Example: `mdhearts.exe --export-snapshot snapshots/test.json 123 east`
  - Creates directories as needed and writes a deterministic snapshot using the given seed and starting seat.
- `--import-snapshot <path>`
  - Loads the snapshot, restores the match state, prints details to stdout, and shows a message box for quick confirmation.
- `--show-weights`
  - Prints Normal and Hard weight summaries (respects env overrides).
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

Seats accept `north`, `east`, `south`, `west` (or `n/e/s/w`). Seed defaults to `0` when omitted.

Tips:
- Set `MDH_DEBUG_LOGS=1` to include per-candidate parts and Hard stats on console.
- CLI popups are disabled by default; set `MDH_CLI_POPUPS=1` to enable.
