Scope and config
- Deterministic: MDH_HARD_DETERMINISTIC=1, MDH_HARD_TEST_STEPS=120.
- One round per seed via CLI match harness.

West seat (n=1000 per window)
- Seeds 1000..1999: NNHH/HHNN/NHNH/HNHN all average ~6.367 penalties for West.
- Seeds 2000..2999: NNHH/HHNN/NHNH/HNHN all average ~5.871 penalties for West.
- CSVs under designs/tuning/ mixed_*_west_*_1000_det.csv.

Other seats (n=500 for seeds 1000..1499)
- North: NNHH=HHNN ~6.572.
- East: NNHH/HHNN/NHNH/HNHN ~6.444.
- South: NNHH/HHNN/NHNH/HNHN ~6.618.
- CSVs under designs/tuning/ mixed_*_{north,east,south}_1000_500_det.csv.

Takeaway
- As expected with conservative Hard defaults and single‑round per seed evaluation, Hard ≈ Normal in aggregate across mixes and seats for these windows.
- Proceed to strengthen constructed flipping golden and evaluate modest Hard‑only tuning (Wide‑tier continuation boost under cap; tiny guarded leader‑feed nudge), then re‑run at larger scale.
