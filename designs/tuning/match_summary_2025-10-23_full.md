# Mixed-Seat Evaluation — 2025-10-23

Config: MDH_HARD_DETERMINISTIC=1, MDH_HARD_TEST_STEPS=120; mixes NNHH and HHNN; seat=west; n=1000 per window

Artifacts:
- designs/tuning/mixed_nnhh_west_1000_1000.csv
- designs/tuning/mixed_hhnn_west_2000_1000.csv

NNHH west 1000..1999: mean=6.367 sd=6.436 se=0.204 ci95=0.399 (n=1000)
HHNN west 2000..2999: mean=5.871 sd=6.046 se=0.191 ci95=0.375 (n=1000)

Interpretation: Lower mean penalties indicates stronger performance. The two mixes show similar means; additional seats/mixes and non-deterministic runs may be needed to detect small deltas.
