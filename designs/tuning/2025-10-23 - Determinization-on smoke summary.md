Config
- Deterministic caps: MDH_HARD_DETERMINISTIC=1, MDH_HARD_TEST_STEPS=120
- Determinization: MDH_HARD_DET_ENABLE=1, MDH_HARD_DET_SAMPLE_K=5, MDH_HARD_DET_PROBE_WIDE_LIKE=1, MDH_HARD_DET_NEXT3_ENABLE=1

Seat West (n=300, seeds 1000..1299)
- NNHH ≈ 6.387, HHNN ≈ 6.387

Other seats (n=300)
- Seeds 1000..1299:
  - North: NNHH ≈ 6.693, HHNN ≈ 6.693
  - East: NNHH ≈ 6.170, HHNN ≈ 6.170
  - South: NNHH ≈ 6.750, HHNN ≈ 6.750
- Seeds 2000..2299:
  - North: NNHH ≈ 7.967, HHNN ≈ 7.967
  - East: NNHH ≈ 6.750, HHNN ≈ 6.750
  - South: NNHH ≈ 6.377, HHNN ≈ 6.377

Takeaway
- With determinization enabled (K=5) under conservative continuation weights and strict caps, Hard ≈ Normal on these small slices.
- Next, finalize the strict flip golden and consider slightly stronger, still-capped Hard-only nudges for targeted windows before any default promotions.
