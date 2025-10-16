# Developer CLI Commands

Run `mdhearts.exe` with the following options for snapshot debugging and self-play data capture:

- `--export-snapshot <path> [seed] [seat]`
  - Example: `mdhearts.exe --export-snapshot snapshots/test.json 123 east`
  - Creates directories as needed and writes a deterministic snapshot using the given seed and starting seat.
- `--import-snapshot <path>`
  - Loads the snapshot, restores the match state, and prints summary details to stdout.
- `eval <games> --self-play --collect-rl <path> [--reward-mode <mode>]`
  - Runs headless self-play and exports RL experiences in JSONL format. Reward modes `shaped`, `per_trick`, and `terminal` now emit per-step rewards immediately after each action.

Seats accept `north`, `east`, `south`, `west` (or `n/e/s/w`). Seed defaults to `0` when omitted.

### CLI Pop-up Dialogs

Snapshot and help commands no longer display modal Windows message boxes by default—ideal for automation and CI. To re-enable pop-ups locally, set:

```
set MDH_CLI_POPUPS=1
```

or on PowerShell:

```
$env:MDH_CLI_POPUPS = "1"
```

Any truthy value (`1`, `true`, `on`, `yes`) activates the dialogs for that process.
