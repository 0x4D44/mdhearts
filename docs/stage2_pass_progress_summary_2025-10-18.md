# Stage 2 Passing Progress Summary — 2025-10-18

## Recent Progress
- **Belief-weighted penalties:** The pass optimizer now scales liability using `left_shooter_pressure`, applying larger hits when premium hearts move toward a high-risk left neighbour. Coverage, remainder, and new directional boosters are tied to shooter projections.
- **Edge-case guards:** Added explicit penalties for triple-premium dumps, Ace-only left passes, mid-heart-only handoffs, and king/ace low-support or off-suit mixes. Regression tests cover each scenario (`left_pass_multiple_premiums_penalised`, `left_pass_mid_hearts_penalised`, `left_pass_ace_only_penalised`, `penalty_triggers_when_passing_single_premium`, `left_pass_king_low_support_penalised`, `left_pass_king_with_support_penalised`, `left_pass_king_offsuit_penalised_when_low_hearts_available`, `left_pass_ace_with_low_hearts_penalised`, `left_pass_single_premium_penalised_under_pressure`).
- **Benchmark results:** Latest run `stage2_pass_moon` (**handshake23**) delivered **97.0 %** block success (2 042 / 2 105) with the same failure set (75×2, 153, 242×2, 432, 461, 498, 511×2, 681, 757, 767×2, 912×2). Premium guards hold, but Q♠/club fallbacks still dominate the residual list.
- **Documentation sync:** Added `docs/benchmarks/stage2_block_failures_handshake23.md`, `docs/benchmarks/stage2_pass_moon_v045_handshake23.md`, `docs/stage2_block_failure_analysis_handshake23.md`, and refreshed the CSV/JSON aggregates.

## Next Steps
1. Filter A♥/K♥ single-support passes out of the candidate set and supply compliant substitutes to avoid the Q♠ dump fallback (hands 153/432/498/767/912).
2. Add regression fixtures for hands 153/432/461/498/681/767/912 and ensure telemetry captures belief heart-mass for these scenarios.
3. Investigate alternative pass generation when premium options are rejected so the optimizer retains viable heart-sharing candidates instead of pure off-suit dumps.
