# Stage 2 Pass & Moon Benchmark — 2025-10-17

## Run Details

- Config: `bench/stage2_pass_moon.yaml`
- Environment: `MDH_PASS_V2=1`, `MDH_ENABLE_BELIEF=1`, `--log-pass-details`, `--log-moon-details`
- Deals: 1,024 hands × 4 seat permutations (4,096 total deals)
- Outputs: `bench/out/stage2_pass_moon/`

## Tournament Summary (Δ vs. baseline `baseline_normal`)

| Agent | Avg PPH | Δ vs baseline | Win % | Moon % | Avg ms/decision | p-value |
|-------|---------|---------------|-------|--------|------------------|---------|
| baseline_normal | 6.851 | +0.000 | 30.4% | 1.0% | 0.17 | 1.000 |
| baseline_easy | 5.295 | -1.556 | 46.3% | 0.0% | 0.06 | <0.001 |
| baseline_hard | 6.964 | +0.113 | 30.8% | 1.0% | 0.17 | 0.601 |
| baseline_normal_2 | 6.890 | +0.039 | 33.5% | 1.6% | 0.17 | 0.899 |

Latency remained well below the 1.2 s budget for all agents. Plotters failed to render PNG output in WSL (missing font support), as expected.

## Telemetry Highlights

From `telemetry_summary.json` (Rust aggregation):

- Pass events captured: **10,698**
  - Avg pass score: **151.12**
  - Avg candidate count: **30.00**
  - Avg moon probability: **0.361**
  - Avg best-vs-next margin: **5.95**
  - Objective mix: `block_shooter=6,133`, `pph=4,565`
- Play objective counts: `BlockShooter=51,177`, `MyPointsPerHand=130,672`

Python reprocessing (`tools/analyze_telemetry.py`) reported comparable figures (`pass.count=10,923`, `avg_total=151.03`, `block_shooter=6,245`). Differences stem from newline-delimited parsing tolerating a handful of trailing heartbeat lines; both sources confirm ~37% of pass decisions trigger block-shooter behaviour and that play-time objectives skew ~28% toward defensive mode once belief + pass_v2 are enabled.

## Observations

- Block-shooter mode activates on ~57% of logged passes and ~28% of plays, indicating moon risk detection is firing frequently while still keeping PPH roughly neutral relative to the normal baseline.
- The optimizer consistently evaluates the full 30-candidate budget; we may want to revisit pruning heuristics to cut telemetry volume once tuning stabilises.
- Early trick adjustments to forbid dumping Q♠ on the opening club lead required relaxing `NoPointsOnFirstTrick` logic to permit hearts when no safe alternative exists (see `crates/hearts-core/src/model/round.rs` change). Without it, Stage 2 runs fail when a pass leaves a player void in clubs with only penalty suits.

## Next Steps

1. Feed `telemetry_summary.json` into the analytics notebook to visualise pass-score distribution and block-shooter activation windows.
2. Compare this baseline against a pass_v1 control run to quantify PPH and moon-defense deltas (Wilcoxon already computed but needs narrative).
3. Trim optimizer candidate pool or add telemetry sampling to reduce log size before scaling to ≥10k-hands smoke tests.
4. Document the `NoPointsOnFirstTrick` rule refinement in the ruleset appendix and ensure regression coverage captures the hearts-only-with-Q♠ scenario.
