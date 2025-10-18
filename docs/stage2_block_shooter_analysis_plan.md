# Stage 2 Block-Shooter Analysis Plan

## Goal
Quantify how often block-shooter objectives succeed (e.g., prevent a moon or capture critical points) and identify tuning opportunities for the moon estimator and defensive heuristics.

## Data Sources
- **Stage 2 selective run**: `bench/out/stage2_pass_moon_v045_selective/telemetry.jsonl` and `deals.jsonl` (current default configuration).
- **Initial Stage 2 run**: `bench/out/stage2_pass_moon/telemetry.jsonl` and `deals.jsonl` (pre-selective weights).
- **Control baseline**: `bench/out/stage2_pass_moon_legacy/`.
- **Failure log**: `docs/benchmarks/stage2_block_failures.md` (generated via `tools/list_block_failures.py`, probability ≥ 0.60).
- **Aggregates**: `bench/out/block_shooter_summary_v045_selective.{json,csv}` plus earlier summaries for comparison.

## Proposed Metrics
1. **Block-shooter success rate**  
   - Condition: telemetry event with `moon_objective="block_shooter"` at pass time.  
   - Success criteria: target seat does *not* shoot the moon in the corresponding deal (need to map telemetry seat to deal outcome).
2. **Moon capture delta**  
   - Count of completed moons per run (from deal logs) for pass_v2 vs. pass_v1.
3. **Penalty redistribution**  
   - Average points taken by the defending seat in block-shooter plays vs. non-block plays.
4. **Moon probability calibration**  
   - Compare estimator probability bins (e.g., 0.3–0.4, 0.4–0.5, …) against actual moon outcomes to evaluate threshold effectiveness.

## Implementation Steps
1. **Telemetry ↔ deal join** *(complete)*  
   - `tools/analyze_block_shooter.py` maps telemetry events to deal outcomes and exports JSON/CSV summaries.
   - `tools/list_block_failures.py` now enumerates high-probability misses for targeted review.
2. **Aggregation**  
   - Compute success/failure counts per estimator probability bin.
   - Summarize moon occurrences per run (pass_v2 vs. pass_v1).
3. **Visualization** *(in progress)*  
   - `tools/render_stage2_plots.py` produces block ratio and success-bin charts. Consider augmenting with failure heatmaps derived from the new failure log.
4. **Regression Tests (follow-up)**  
   - Once metrics are in place, add targeted tests or golden files to ensure future runs preserve or improve block-shooter performance.

## Next Actions
- Use `tools/list_block_failures.py` to cluster failure scenarios (current log shows inverted moons concentrated on West/North passes); reproduce them in targeted simulations.
- Compare selective vs. initial runs in notebooks to quantify improvements by probability bin and seat.
- Feed refined visuals/tables into the Stage 2 deck and acceptance narrative.

## Current Findings
- All recorded high-probability failures (≥ 0.60) are **inverted moon** outcomes.
- Over 70 % of misses occur when West/North issue the block pass; shooter distribution is West 8, South 5, North 5, East 2.
- Many failures feature the blocking seat still holding 2+ high hearts or Q♠, indicating we may need stronger liability boosts in those contexts.

## Proposed Mitigations (Draft)
1. **Directional Liability Boost**  
   - Increase liability weight when the passing direction is Left/Across (i.e., West/North seats) and urgency ≥ 0.6.
   - Add telemetry flag to verify the boost triggers on the recorded failure hands.
2. **High-Heart Handshake Rule**  
   - If a blocker retains ≥ 2 hearts ranked Q or above, apply an extra penalty unless at least one heart is being passed to the perceived shooter seat.
   - Track via telemetry whether the blocker still ends up shooting the inverted moon.
3. **Runner-Up Moon Probability Check**  
   - Compare moon probability of the intended recipient; if near-threshold, consider alternative passes that distribute hearts across multiple opponents.
4. **Validation Strategy**  
   - Re-run selective benchmark, confirm block rate remains ~19 %, success ≥ 97 %.
   - Ensure failure log count drops (target < 12 inverted moons ≥ 0.60) and record seat distribution shift.
- Seat points at failure frequently show the blocking seat taking 26 points, implying insufficient liability penalties when holding high hearts/Q♠.
