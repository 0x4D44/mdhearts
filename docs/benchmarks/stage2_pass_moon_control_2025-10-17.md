# Stage 2 Control Benchmark (pass_v1) — 2025-10-17

## Run Details

- Config: `bench/stage2_pass_moon.yaml`
- Environment: `MDH_PASS_V2=0`, `MDH_ENABLE_BELIEF=1`, `--log-pass-details`, `--log-moon-details`
- Deals: 1,024 hands × 4 permutations (4,096 deals)
- Outputs: `bench/out/stage2_pass_moon_legacy/`

## Tournament Summary (Δ vs. baseline `baseline_normal`)

| Agent | Avg PPH | Δ vs baseline | Win % | Moon % | Avg ms/decision | p-value |
|-------|---------|---------------|-------|--------|------------------|---------|
| baseline_normal | 6.177 | +0.000 | 33.0% | 0.5% | 0.12 | 1.000 |
| baseline_easy | 8.198 | +2.021 | 28.4% | 0.8% | 0.05 | <0.001 |
| baseline_hard | 6.422 | +0.245 | 32.5% | 0.5% | 0.12 | 0.200 |
| baseline_normal_2 | 5.203 | -0.974 | 42.5% | 0.5% | 0.11 | <0.001 |

The legacy pass logic yields large PPH variance between agents and elevates the Easy bot’s score by ~2.0 points per hand relative to the baseline seat, indicating frequent point dumps from teammates.

## Telemetry Highlights

- Pass events captured: **0** — legacy planner does not emit the structured pass telemetry, confirming why Stage 2 logging work was necessary.
- Play objective counts: `BlockShooter=51,110`, `MyPointsPerHand=129,087` (unchanged from Stage 2 run because the moon estimator still drives play-style toggles even when pass_v2 is off).

## Comparison vs. pass_v2 Run

| Agent | Δ Avg PPH (pass_v2 − pass_v1) | Notes |
|-------|------------------------------|-------|
| baseline_normal | **+0.674** | Stage 2 pass logic recovers ~0.67 PPH for the flagship seat. |
| baseline_easy | **−2.903** | Easy bot loses dumping privileges against the more disciplined pass_v2, reducing its ability to offload penalties. |
| baseline_hard | **+0.542** | Hard seat benefits from improved coordination. |
| baseline_normal_2 | **+1.687** | Second normal seat gains the most, mirroring the control’s point losses. |

Moon-shot frequency doubles (0.5% → 1.0–1.6%) for the stronger seats when pass_v2 + block-shooter heuristics are active, reflecting more aggressive moon defence and opportunistic runs.

## Takeaways

- The legacy pass logic prevents telemetry capture and enables extreme point transfers; Stage 2 pass_v2 both surfaces the data needed for tuning and reins in the outlier agent behaviour.
- Even without additional tuning, pass_v2 delivers positive PPH deltas for the competitive seats while shrinking the easy agent’s advantage, moving us toward the “fairness” goals described in the Stage 2 plan.
- Follow-up: integrate these metrics into the notebooks and prepare a concise before/after write-up for acceptance review.
