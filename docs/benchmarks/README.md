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
  pass_details: true        # include pass rankings + component scores
  moon_details: true        # include moon objective events
```

When enabled, the harness writes `telemetry.jsonl` next to the summary file and sets `MDH_DEBUG_LOGS=1`. Set `pass_details` to emit `hearts_bot::pass_decision` events with the selected trio, top-N candidate breakdowns, and moon probability/objective. Set `moon_details` to forward `hearts_bot::play` events (seat, style, objective, chosen card, legal set) for downstream analysis. The harness now also generates `telemetry_summary.json` and `telemetry_summary.md` with aggregated pass/play metrics after each run. Both toggles can also be activated from the CLI:

```
cargo run -p hearts-bench -- --config bench/smoke.yaml --log-pass-details --log-moon-details
```

Heuristic bots only compute the additional telemetry when the corresponding environment variables (`MDH_PASS_DETAILS`, `MDH_MOON_DETAILS`) are present, so the default desktop client remains unaffected.

For Stage 2 evaluations, `bench/stage2_pass_moon.yaml` captures 1,024 hands (four seat permutations) with structured telemetry enabled by default. Run with the pass/v2 and belief flags to exercise the new logic:

```
MDH_PASS_V2=1 MDH_ENABLE_BELIEF=1 \
  cargo run -p hearts-bench -- --config bench/stage2_pass_moon.yaml \
  --log-pass-details --log-moon-details
```

The run emits `bench/out/<run_id>/telemetry.jsonl` plus the derived summaries. `summary.md` now appends a **Telemetry Highlights** section with pass counts, moon probabilities, and objective mixes so stakeholders can see Stage 2 behaviour without opening additional files. For ad-hoc reprocessing or notebook workflows:

```
python tools/analyze_telemetry.py \
  bench/out/<run_id>/telemetry.jsonl \
  --json bench/out/<run_id>/telemetry_summary.json \
  --markdown bench/out/<run_id>/telemetry_summary.md \
  --csv bench/out/<run_id>/telemetry_summary.csv \
  --run-id <run_id>
```

The script mirrors the harness-generated artefacts and reports average pass scores, moon probability distribution, objective counts, plus block-shooter ratios for both pass and play telemetry. The optional CSV output makes it easy to ingest metrics into notebooks or dashboards; rerunning the command appends/updates a single-row dataset per run.

Quickly turn multiple CSV rows into a Markdown table (for docs or decks) with:

```
python tools/telemetry_to_markdown.py \
  bench/out/stage2_pass_moon/telemetry_summary.csv \
  bench/out/stage2_pass_moon_legacy/telemetry_summary.csv \
  --output docs/benchmarks/stage2_pass_block_ratios.md
```

### Comparing Runs

Use `tools/compare_pass_runs.py` to produce side-by-side Markdown comparing two benchmark outputs (PPH, win/moon percentages, telemetry counts). Example:

```
python tools/compare_pass_runs.py \
  bench/out/stage2_pass_moon \
  bench/out/stage2_pass_moon_legacy \
  --label-a pass_v2 \
  --label-b pass_v1 \
  --markdown-output docs/benchmarks/stage2_pass_comparison.md
```

The script reads each run’s `summary.md` and `telemetry_summary.json`, prints the table to stdout, and optionally writes the formatted report for doc inclusion.

To evaluate block-shooter effectiveness, join telemetry with deal outcomes:

```
python tools/analyze_block_shooter.py \
  pass_v2=bench/out/stage2_pass_moon \
  pass_v1=bench/out/stage2_pass_moon_legacy \
  --csv bench/out/block_shooter_summary.csv \
  --json bench/out/block_shooter_summary.json
```

This reports per-run success counts (e.g., 1,994 / 2,056 block passes prevented a moon in the latest Stage 2 run) and probability-bin summaries ready for dashboards or the acceptance deck.

Render deck-ready charts directly from the telemetry/block summaries with:

```
.venv/bin/python tools/render_stage2_plots.py \
  pass_v2=bench/out/stage2_pass_moon \
  pass_v1=bench/out/stage2_pass_moon_legacy \
  --block-summary bench/out/block_shooter_summary.json \
  --output-dir docs/benchmarks/plots
```

The script produces PNG (or SVG with `--format svg`) assets such as `stage2_pass_block_ratio.png` and `stage2_block_success_bins_pass_v2.png`, which the deck references on Slide 5.

> NOTE (2025-10-17): `MoonEstimatorConfig::block_threshold` now defaults to **0.45**. Re-run the Stage 2 benchmarks after pulling this change so telemetry/block plots reflect the tighter gating.

Need a quick what-if on the new threshold before rerunning the full benchmark? Estimate it with:

```
python3 tools/sweep_block_threshold.py \
  pass_v2=bench/out/stage2_pass_moon \
  --thresholds 0.32 0.40 0.45 0.50 \
  --markdown docs/stage2_block_threshold_sweep.md
```

This consumes the existing telemetry + deal logs and projects how many block passes and successes you would retain at higher cutoffs.

### Belief Snapshots

Stage 1 adds a probability-driven tracker that can be toggled for any run. Set the environment variable before invoking the harness (or desktop client):

```
MDH_ENABLE_BELIEF=1
MDH_PASS_V2=1
```

With the flag enabled, the controller maintains per-seat beliefs, applies soft likelihood updates, and logs entropy metrics on every play using the `mdhearts::belief` tracing target. Additional knobs allow runtime tuning without recompilation:

```
MDH_BELIEF_VOID_THRESHOLD=0.15   # probability mass required to consider a suit "safe"
MDH_BELIEF_SOFT_QUEEN=0.65       # weight when a candidate declines to dump QS on spade leads
MDH_BELIEF_SOFT_SLOUGH=1.15      # weight applied when sloughing penalty cards off-suit
MDH_BELIEF_SOFT_MIN=0.10         # minimum clamp to avoid zeroing columns entirely
```

All values must be finite floats. Thresholds outside their clamps are ignored. Combine `MDH_ENABLE_BELIEF=1` with `MDH_PASS_V2=1` to evaluate the new Stage 2 passing logic (direction-aware scoring and moon prep); structured telemetry exposes entropy, sampler stats, and pass-ranking events when the flags are active.

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
