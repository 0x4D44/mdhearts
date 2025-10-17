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

## Benchmark Harness (`hearts-bench`)

- `cargo run -p hearts-bench -- --config bench/bench.yaml` runs the deterministic tournament harness.
- Use `--validate-only` to perform schema validation without simulating games.
- A fast smoke run is available via `bench/smoke.yaml`; CI uses this for guardrails.
- Set `logging.enable_structured: true` in the YAML to emit JSONL telemetry (bench enables `MDH_DEBUG_LOGS=1` automatically).
- External bots can be wired through `kind: external` agents; see `tools/xinxin_runner --help` for the adapter shim.

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
