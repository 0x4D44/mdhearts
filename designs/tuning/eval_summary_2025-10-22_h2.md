H2 Experiment — Mixed-seat NNHH (deterministic)

Setup
- Seat: West and North (separate runs), seeds 1000..1059 (60 hands), mix=nnhh.
- Deterministic: MDH_HARD_DETERMINISTIC=1, MDH_HARD_TEST_STEPS=120.
- Baseline: tiers OFF, nudges OFF, scaling OFF.
- H2: tiers OFF, nudges/scaling ON.
  - MDH_HARD_PLANNER_NUDGES=1
  - MDH_HARD_PLANNER_LEADER_FEED_NUDGE=10
  - MDH_HARD_PLANNER_SELF_CAPTURE_NUDGE=10
  - MDH_HARD_PLANNER_MAX_BASE_FOR_NUDGE=200
  - MDH_HARD_CONT_SCALE_FEED_PERMIL=50
  - MDH_HARD_CONT_SCALE_SELFCAP_PERMIL=30

Artifacts
- West baseline: designs/tuning/mixed_nnhh_west_1000_60_baseline.csv
- West tiers:    designs/tuning/mixed_nnhh_west_1000_60_tiers.csv
- West H2:       designs/tuning/mixed_nnhh_west_1000_60_h2.csv
- North baseline: designs/tuning/mixed_nnhh_north_1000_60_baseline.csv
- North H2:       designs/tuning/mixed_nnhh_north_1000_60_h2.csv

Averages (penalties per hand; lower is better)
- West: baseline=6.28, tiers=6.28, H2=6.28
- North: baseline=6.57, H2=6.57

Notes
- With conservative knobs and only 60 hands/seat, no measurable delta appeared. This is expected given tiny nudges/scaling and limited sample size.

Next Steps
- Increase sample size (≥200 seeds per seat) and include multiple permutations (NNHH, NHNH, HNNH).
- Temporarily widen knobs for sensitivity analysis (e.g., nudge=20, scale_feed=100, scale_self=60), verify improvements, then ratchet down.
- Consider enabling tiering during H2 tests to allow wider PhaseB-K/next-M in Wide tier situations.
- If clear gains emerge without destabilizing goldens, propose small default promotions for Hard-only continuation (leave Normal unchanged), and add a constructed golden capturing the improved decision.
