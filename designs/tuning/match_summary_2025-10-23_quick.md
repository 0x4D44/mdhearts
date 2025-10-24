Quick Mixed-Seat Evaluation — 2025‑10‑23

Config
- Deterministic: MDH_HARD_DETERMINISTIC=1, MDH_HARD_TEST_STEPS=120
- Mixes: NNHH (Hard seats South+West), HHNN (Hard seats North+East)
- Windows: 1000..1059 (60 seeds), 2000..2059 (60 seeds)
- Commands:
  - mdhearts --match-mixed west 1000 60 nnhh --hard-deterministic --hard-steps 120 --out designs/tuning/mixed_nnhh_west_1000_60_quick.csv
  - mdhearts --match-mixed west 2000 60 hhnn --hard-deterministic --hard-steps 120 --out designs/tuning/mixed_hhnn_west_2000_60_quick.csv

Artifacts
- designs/tuning/mixed_nnhh_west_1000_60_quick.csv
- designs/tuning/mixed_hhnn_west_2000_60_quick.csv

Notes
- This is a small smoke sample to validate CLI + telemetry after the latest code changes. Use larger n (>=1000/seat) for meaningful CI evaluation per plan.

