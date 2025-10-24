2025-10-23 — Implementation kickoff per Hard Advantage plan

Summary
- Reviewed designs/2025.10.23 - Hard Advantage Implementation Plan.md.
- Repository already contains most Phase 1 (M1) features: leverage tiers, Wide-tier continuation boosts (env), probe widening (choose-only), Hard-only planner nudges, extensive tests/CLI.
- Started implementation by improving telemetry/weights surfacing to aid tuning.

Changes
- crates/hearts-app/src/bot/search.rs: debug_hard_weights_string now prints Wide-tier permille boost envs:
  - MDH_HARD_WIDE_PERMIL_BOOST_FEED
  - MDH_HARD_WIDE_PERMIL_BOOST_SELFCAP
- crates/hearts-app/tests/weights_surface_smoke.rs: extended smoke test to assert these fields appear when env is set.

Validation
- Ran cargo test --all: all tests passed.

Next
- If we push further per M1: consider minor Stats additions (optional) and re-run mixed-seat evaluations.
- If M1 deltas remain < +1.0 pts/hand, proceed to M2 (determinized rollouts scaffold) behind env gates.

Addendum: M2 scaffold (determinization)
- Added env-gated multi-sample wrapper around current-trick rollout (no default behavior change):
  - MDH_HARD_DET_ENABLE, MDH_HARD_DET_SAMPLE_K, MDH_HARD_DET_TIME_MS (surfaced in weights string).
- Refactored rollout into core + wrapper (and parts variant) to support averaging.
- Extended CLI hard-stats to print cont_cap and wide-boost fields under MDH_DEBUG_LOGS=1.
- All tests still pass.

Evaluation snapshot (deterministic)
- Commands:
  - mdhearts --match-mixed west 1000 1000 nnhh --hard-deterministic --hard-steps 120 --out designs/tuning/mixed_nnhh_west_1000_1000.csv
  - mdhearts --match-mixed west 2000 1000 hhnn --hard-deterministic --hard-steps 120 --out designs/tuning/mixed_hhnn_west_2000_1000.csv
- Summary written to designs/tuning/match_summary_2025-10-23_full.md.
- Observation: means are close across mixes; no clear aggregate advantage yet at these conservative defaults. Next, either widen determinization (next-trick probe per sample) or run broader seat/mix permutations.
