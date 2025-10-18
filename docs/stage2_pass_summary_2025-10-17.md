# Stage 2 Passing Findings — 2025-10-17

## Overview

- **Configurations compared**: `bench/stage2_pass_moon.yaml` with `MDH_PASS_V2=1` (new logic, threshold 0.45) vs. the same run with `MDH_PASS_V2=0` (legacy) and `MDH_ENABLE_BELIEF=1`.
- **Harness outputs**: `bench/out/stage2_pass_moon_v045_selective/` (`pass_v045_selective`, current default), `bench/out/stage2_pass_moon_v045/` (initial threshold run), and `bench/out/stage2_pass_moon_legacy/` (`pass_v1`).
- **Comparison artefacts**:
  - Stage 2 (0.45) run: `docs/benchmarks/stage2_pass_moon_v045.md`
  - Stage 2 (initial) run: `docs/benchmarks/stage2_pass_moon_2025-10-17.md`
  - Control run: `docs/benchmarks/stage2_pass_moon_control_2025-10-17.md`
  - Delta table & telemetry snapshot: `docs/benchmarks/stage2_pass_comparison.md`

## Key Metrics

| Agent | pass_v045_selective Avg PPH | pass_v1 Avg PPH | Δ (v045_selective − v1) | pass_v045_selective Moon % | pass_v1 Moon % |
|-------|------------------|-----------------|---------------|------------------|----------------|
| baseline_normal | 6.479 | 6.177 | **+0.302** | 0.7% | 0.5% |
| baseline_normal_2 | 6.416 | 5.203 | **+1.213** | 1.0% | 0.5% |
| baseline_hard | 6.450 | 6.422 | **+0.028** | 0.6% | 0.5% |
| baseline_easy | 6.655 | 8.198 | **−1.543** | 0.2% | 0.8% |

Raising the moon-defense threshold trims the Easy seat’s dumping advantage while preserving modest gains for the competitive seats. Compared to the earlier pass_v2 run (see appendix), PPH deltas shrink but remain directionally positive for the target agents; win rates stay within noise.

## Telemetry Highlights

- pass_v045_selective recorded **10,764** pass decisions (avg score **151.11**, avg moon probability **0.361**), with block-shooter objectives on **18.7 %** of passes and **9.3 %** of plays.
- pass_v1 still emits **0** pass telemetry events, confirming the legacy planner lacks the structured instrumentation needed for tuning.
- Play telemetry is available in both runs (`BlockShooter=17,002`, `MyPointsPerHand=165,971` for pass_v045_selective), validating the Stage 2 logging pipeline.

### Block-Shooter Outcomes
- `tools/analyze_block_shooter.py` joins pass telemetry with deal outcomes:
  - pass_v045_selective logged **2,071** block-shooter passes; **2,010** (97.1 %) prevented a moon, **26** failed (other seat shot the moon), **35** resulted in the same seat shooting the moon. Moon rate per hand remains **2.56 %** (105 / 4,096).
  - pass_v1 produced **0** block-shooter passes (legacy planner emits no `pass_decision` events), so success metrics are unavailable.
  - Probability sweep (`docs/stage2_block_threshold_sweep.md`) shows little change between thresholds 0.32–0.45 on the existing dataset but confirms the live run sits in the desired ~19 % activation band; failures concentrate in the >0.6 probability bins.

## Implementation Notes

- Updated first-trick rule handling prevents sloughing Q♠ unless no safe alternatives exist; hearts are permitted only when the hand is point-locked.
- The play planner now filters first-trick candidates when following clubs to align with the rules adjustment.
- `tools/compare_pass_runs.py` can be rerun to refresh the comparison once tuning adjustments land.

## Next Steps

1. Feed the updated pass_v045 vs. pass_v1 data into the Stage 2 acceptance deck (charts + narrative).
2. Tune estimator/heuristic weights targeting the >0.6 probability failure bins while preserving the ~19 % activation rate.
3. Backfill regression tests covering the “void in clubs with only penalty cards” scenario to lock in the rule fix.
