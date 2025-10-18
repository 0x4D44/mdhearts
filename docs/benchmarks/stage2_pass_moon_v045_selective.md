# Stage 2 Pass & Moon Benchmark — Threshold 0.45 (Selective Moon Scaling)  
*Run: `stage2_pass_moon_v045_selective` — 2025-10-17*

## Run Details
- Config: `bench/stage2_pass_moon.yaml`
- Environment: `MDH_PASS_V2=1`, `MDH_ENABLE_BELIEF=1`
- Source control: selective moon-support scaling for high-urgency, high-liability cards (current default)
- Deals: 1,024 hands × 4 seat permutations (4,096 deals)
- Outputs: `bench/out/stage2_pass_moon_v045_selective/`

## Tournament Summary (Δ vs. baseline `baseline_normal`)

| Agent | Avg PPH | Δ vs baseline | Win % | Moon % | Avg ms/decision | p-value |
|-------|---------|---------------|-------|--------|------------------|---------|
| baseline_normal | 6.479 | +0.000 | 30.5% | 0.7% | 0.12 | 1.000 |
| baseline_easy | 6.655 | +0.176 | 39.7% | 0.2% | 0.03 | 0.079 |
| baseline_hard | 6.450 | -0.029 | 30.3% | 0.6% | 0.12 | 0.753 |
| baseline_normal_2 | 6.416 | -0.063 | 32.2% | 1.0% | 0.12 | 0.547 |

Latencies remain well within the 1.2 s budget; plotters continue to fail under WSL (missing fonts), so use `tools/render_stage2_plots.py` for deck assets.

## Telemetry Highlights
- Pass events: **10,764**
  - Avg pass score: **151.11**
  - Avg candidate count: **30.00**
  - Avg moon probability: **0.361**
  - Avg best-vs-next margin: **5.96**
  - Objective mix: `block_shooter=2,010`, `pph=8,754`
- Play objectives: `BlockShooter=17,002`, `MyPointsPerHand=165,971`
- Python reprocessing (`tools/analyze_telemetry.py`) yields an **18.7 %** block-pass ratio and **9.3 %** block-play ratio (see `telemetry_summary_py.*`).

## Block-Shooter Outcomes
`tools/analyze_block_shooter.py pass_v045_selective=bench/out/stage2_pass_moon_v045_selective`

- Block passes: **2,071**
  - Successes (no moon): **2,010** (**97.05 %**)
  - Failures (other shooter): **26**
  - Failures (self-shooter): **35**
- Moon occurrences: **105** (2.56 % of deals)
- Probability bins:
  - `[0.4,0.5)`: 664 attempts, 98.3 % success
  - `[0.5,0.6)`: 1,125 attempts, 97.3 % success
  - `[0.6,0.7)`: 232 attempts, 93.97 % success
  - `[0.7,0.8)`: 50 attempts, 88.0 % success

## Observations
- Selective weighting keeps the block-pass ratio aligned with the 0.45 threshold (~18.7 %) while slightly improving success in the troublesome >0.6 bins compared with the earlier untuned run.
- PPH deltas remain similar to the threshold baseline: competitive seats retain modest advantages, Easy seat still loses ~1.5 PPH relative to the control.
- High-probability failures persist mainly in `[0.6,0.7)`; further tuning should focus on those scenarios (e.g., additional heuristics around partner voids and high heart coordination).

## Artefacts
- Summary: `bench/out/stage2_pass_moon_v045_selective/summary.md`
- Telemetry: `bench/out/stage2_pass_moon_v045_selective/telemetry.jsonl`
- Telemetry summaries: `telemetry_summary.json`, `telemetry_summary_py.{json,md,csv}`
- Block analysis: `bench/out/block_shooter_summary_v045_selective.{json,csv}`
- Consolidated digest: `docs/stage2_metrics_digest.md`
- Plots: regenerate via `.venv/bin/python tools/render_stage2_plots.py pass_v045_selective=bench/out/stage2_pass_moon_v045_selective pass_v1=bench/out/stage2_pass_moon_legacy --block-summary bench/out/block_shooter_summary_v045_selective.json --output-dir docs/benchmarks/plots`
