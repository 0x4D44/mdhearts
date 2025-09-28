# Developer CLI Commands

Run `mdhearts.exe` with the following options for snapshot debugging:

- `--export-snapshot <path> [seed] [seat]`
  - Example: `mdhearts.exe --export-snapshot snapshots/test.json 123 east`
  - Creates directories as needed and writes a deterministic snapshot using the given seed and starting seat.
- `--import-snapshot <path>`
  - Loads the snapshot, restores the match state, prints details to stdout, and shows a message box for quick confirmation.

Seats accept `north`, `east`, `south`, `west` (or `n/e/s/w`). Seed defaults to `0` when omitted.
