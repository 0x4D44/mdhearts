# Stage 2 Pass & Moon Benchmark — Threshold 0.45 (Directional Liability Prototype)  
*Run: `stage2_pass_moon_v045_directional` — 2025-10-17*

## Run Details
- Config: `bench/stage2_pass_moon.yaml`
- Environment: `MDH_PASS_V2=1`, `MDH_ENABLE_BELIEF=1`
- Change under test: directional/high-liability moon scaling (West/North emphasis)
- Deals: 1,024 hands × 4 seat permutations (4,096 deals)
- Outputs: `bench/out/stage2_pass_moon_v045_directional/`

## Tournament Summary

| Agent | Avg PPH | Δ vs baseline | Win % | Moon % | Avg ms/decision | p-value |
|-------|---------|---------------|-------|--------|------------------|---------|
| baseline_normal | 6.483 | +0.000 | 30.5% | 0.7% | 0.12 | 1.000 |
| baseline_easy | 6.645 | +0.163 | 39.7% | 0.2% | 0.04 | 0.094 |
| baseline_hard | 6.453 | -0.029 | 30.2% | 0.6% | 0.12 | 0.755 |
| baseline_normal_2 | 6.419 | -0.064 | 32.3% | 1.0% | 0.12 | 0.543 |

PPH results are effectively unchanged from the selective baseline. Latency remains well within budget.

## Telemetry Highlights

- Pass events: **10,803** (block ratio **18.7 %**)
- Avg pass score: **151.17**, avg moon probability: **0.361**, avg best-vs-next margin: **5.96**
- Objective mix: `block_shooter=2,020`, `pph=8,783`
- Play objectives: `BlockShooter=17,118`, `MyPointsPerHand=166,527`
- Python reprocessing (`tools/analyze_telemetry.py`) corroborates a **0.188** pass block ratio and **0.093** play block ratio.

## Block-Shooter Outcomes

`tools/analyze_block_shooter.py pass_v045_directional=bench/out/stage2_pass_moon_v045_directional`

- Block passes: **2,081**
  - Successes: **2,020** (**97.07 %**)
  - Failures (other shooter): **26**
  - Failures (self shooter): **35**
- Moon rate per hand: **2.56 %**
- Probability bins: identical to the selective baseline (see `bench/out/block_shooter_summary_v045_directional.json`)

## High-Probability Failure Log

`tools/list_block_failures.py pass_v045_directional=… --threshold 0.6`

- Failures ≥ 0.60 probability: **20** (no change vs. selective baseline)
- All failures remain inverted moons; shooter distribution unchanged (West 8, South 5, North 5, East 2)
- Detailed table: `docs/benchmarks/stage2_block_failures_directional.md`

## Takeaways

- The directional/high-liability scaling prototype did **not** reduce failure count or shift the probability bins.
- PPH impact is neutral; Easy seat gains ~0.16 PPH vs. baseline, so no benefit observed.
- We should revert to the selective baseline for now and pursue more targeted heuristics (e.g., explicit high-heart handoffs, partner coordination) before the next benchmark rerun.

## Artefacts

- Summary: `bench/out/stage2_pass_moon_v045_directional/summary.md`
- Telemetry: `telemetry.jsonl`, `telemetry_summary*.{json,md,csv}`
- Block analysis: `bench/out/block_shooter_summary_v045_directional.{json,csv}`
- Failure log: `docs/benchmarks/stage2_block_failures_directional.md`
