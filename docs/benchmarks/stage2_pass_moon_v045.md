# Stage 2 Pass & Moon Benchmark — Threshold 0.45 (2025-10-17)

## Run Details

- Config: `bench/stage2_pass_moon.yaml`
- Overrides: `--run-id stage2_pass_moon_v045`
- Environment: `MDH_PASS_V2=1`, `MDH_ENABLE_BELIEF=1`
- Deals: 1,024 hands × 4 seat permutations (4,096 total deals)
- Outputs: `bench/out/stage2_pass_moon_v045/`

## Tournament Summary (Δ vs. baseline `baseline_normal`)

| Agent | Avg PPH | Δ vs baseline | Win % | Moon % | Avg ms/decision | p-value |
|-------|---------|---------------|-------|--------|------------------|---------|
| baseline_normal | 6.475 | +0.000 | 30.6% | 0.7% | 0.13 | 1.000 |
| baseline_easy | 6.657 | +0.182 | 39.7% | 0.2% | 0.04 | 0.072 |
| baseline_hard | 6.448 | -0.027 | 30.3% | 0.6% | 0.12 | 0.767 |
| baseline_normal_2 | 6.419 | -0.056 | 32.3% | 1.1% | 0.12 | 0.568 |

Latencies remain well within the 1.2 s budget. Plotters again failed to render PNGs under WSL due to font support; waveform export is handled by `tools/render_stage2_plots.py`.

## Telemetry Highlights

- Pass events captured: **10,677**
  - Avg pass score: **151.13**
  - Avg candidate count: **30.00**
  - Avg moon probability: **0.361**
  - Avg best-vs-next margin: **5.96**
  - Objective mix: `block_shooter=1,991`, `pph=8,686`
- Play objective counts: `BlockShooter=16,852`, `MyPointsPerHand=164,618`
- Python reprocessing (`tools/analyze_telemetry.py`) confirms the block-shooter share: **18.6 %** of pass decisions, **9.3 %** of plays.
- *Note:* the Python parser reports 10,971 pass events because it tolerates trailing heartbeat lines; all published ratios use the harness summary total (10,677).

## Block-Shooter Outcomes

`tools/analyze_block_shooter.py pass_v045=bench/out/stage2_pass_moon_v045`

- Block-shooter passes: **2,056**
  - Successes (no moon): **1,994** (96.98 %)
  - Failures (other seat shot moon): **26**
  - Failures (same seat shot moon): **36**
- Moon occurrences: **106** (2.59 % of deals)
- Probability bins:
  - `[0.4,0.5)`: 656 attempts, 98.3 % success
  - `[0.5,0.6)`: 1,120 attempts, 97.3 % success
  - `[0.6,0.7)`: 230 attempts, 93.9 % success
  - `[0.7,0.8)`: 50 attempts, 86.0 % success

## Observations

- Raising `MoonEstimatorConfig::block_threshold` to 0.45 dropped block-shooter passes from ~57 % to **18.6 %** of decisions while keeping success near 97 %.
- Moon rate per hand declined to **2.59 %** (from 3.6 % in the previous tuning), suggesting the higher threshold reduces overeager defensive passes.
- PPH deltas vs. the legacy control soften (baseline seats hold small positive margins, Easy seat still loses ~1.5 PPH), indicating we may need additional weight tuning to regain the prior competitive edge without reintroducing excess block passes.
- High-probability bins (>0.6) remain the weakest performers; further heuristic work should target these scenarios (e.g., weighting hearts control, partner coordination).

## Artefacts

- Summary: `bench/out/stage2_pass_moon_v045/summary.md`
- Telemetry: `bench/out/stage2_pass_moon_v045/telemetry.jsonl`
- Telemetry summary: `bench/out/stage2_pass_moon_v045/telemetry_summary.json`
- Aggregate digest: `docs/stage2_metrics_digest.md`
- Plots: `docs/benchmarks/plots/`
