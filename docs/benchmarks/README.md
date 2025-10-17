# Hearts Benchmark Harness

Stage 0 introduces a deterministic tournament harness that can pit any mix of Hearts bots against each other and surface aggregate statistics, JSONL deal logs, and optional telemetry.

## Quick Start

1. Review or copy `bench/bench.yaml`. Adjust the `agents` list to reference the bots you want to evaluate.
2. Run the harness:
   ```
   cargo run -p hearts-bench -- --config bench/bench.yaml
   ```
   Use `--run-id`, `--hands`, `--seed`, or `--permutations` CLI overrides to iterate quickly.
3. Inspect outputs under `bench/out/<run_id>/`:
   - `deals.jsonl`: per-seat metrics per deal.
   - `summary.md`: Markdown table with PPH deltas, confidence intervals, win/moon rates, and Wilcoxon p-values.
   - `plots/delta_pph.png`: optional plot (skipped if Plotters cannot render in the current environment).

For a fast sanity check, run the smoke configuration:
```
cargo run -p hearts-bench -- --config bench/smoke.yaml --validate-only
cargo run -p hearts-bench -- --config bench/smoke.yaml
```

To capture the Stage 0 external baseline, use the sample config that wires the shim adapter:
```
cargo run -p hearts-bench -- --config bench/xinxin_baseline.yaml --run-id stage0_xinxin_test
```
Replace `tools/xinxin_runner` with the OpenSpiel binary when ready; keep the rest of the config unchanged so Markdown/plots land under `bench/out/<run_id>/`.

## Telemetry

Structured logging is gated behind the YAML flag:
```yaml
logging:
  enable_structured: true
  tracing_level: "info"
```

When enabled, the harness writes `telemetry.jsonl` next to the summary file and sets `MDH_DEBUG_LOGS=1`. Heuristic bots emit per-pass and per-play events (seat, difficulty, style, card choices, unseen counts) guarded by `tracing::enabled!` so the default desktop client remains unaffected.

## External Agents

Agents declared with `kind: external` spawn a subprocess once per decision and communicate via newline-delimited JSON. Each request now includes an `"action"` field (`"pass"` or `"play"`). The helper script in `tools/xinxin_runner` demonstrates the protocol:
```
kind: external
params:
  command: "./tools/xinxin_runner"
  args: ["--mode", "auto"]
  timeout_ms: 1500
  fallback: "heuristic_normal"
```

Replace the shim with the OpenSpiel xinxin binary (or any compatible agent) when available, keeping the same contract:

- Read one JSON request from stdin.
- Return a JSON object with either `{"cards": [...]}` (pass) or `{"card": "QS"}` (play).
- Exit immediately after writing the response.
- Respect the `timeout_ms` budget; the harness emits a warning if the command exceeds it and falls back to the configured heuristic policy.

## Validation & Testing

- `cargo test -p hearts-bench` runs unit tests, including deterministic JSONL hashing for a 2-hand tournament.
- The Wilcoxon implementation is unit-tested against symmetric data to guard against regressions.
- Add longer tournament configs under `bench/` and keep smoke-sized setups runnable within 60 s for CI.
- Optional: run `python tools/validate_wilcoxon.py bench/out/<run_id>/summary.md` to compare Rust statistics with SciPy. This requires SciPy (and its NumPy dependency); if the default Python is PEP 668-managed, create a virtualenv first (`python3 -m venv .venv && .venv/bin/pip install -r python/requirements.txt`) or execute the script on a workstation where SciPy is already installed.
