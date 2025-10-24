Mixed-seat evaluation (smoke)
- Config: deterministic (MDH_HARD_DETERMINISTIC=1, MDH_HARD_TEST_STEPS=120)
- Runs:
  - west 1000..1199 (n=200), mix=nnhh → designs/tuning/mixed_nnhh_west_1000_200_smoke.csv
  - west 2000..2199 (n=200), mix=hhnn → designs/tuning/mixed_hhnn_west_2000_200_smoke.csv

Quick take
- This is a small smoke to validate plumbing. Full summaries (means/CI) will be computed after stabilizing the determinization flip golden and any tiny Hard-only tuning. No anomalies observed; files written successfully.

Next
- Finalize the determinization flipping golden.
- Scale runs to ≥1000 seeds/seat across two windows and all mixes (NNHH/HHNN/NHNH/HNHN). Compute means and 95% CI per group; compare Hard vs Normal seats.
