Config
- Deterministic caps: MDH_HARD_DETERMINISTIC=1, MDH_HARD_TEST_STEPS=120
- Determinization: MDH_HARD_DET_ENABLE=1, MDH_HARD_DET_SAMPLE_K=3, MDH_HARD_DET_PROBE_WIDE_LIKE=1, MDH_HARD_DET_NEXT3_ENABLE=1

Window 1000..1499, n=500 per seat/mix
- North: NNHH=6.572, HHNN=6.572, NHNH=6.572, HNHN=6.572
- East:  NNHH=6.444, HHNN=6.444, NHNH=6.444, HNHN=6.444
- South: NNHH=6.618, HHNN=6.618, NHNH=6.618, HNHN=6.618
- West:  NNHH=6.366, HHNN=6.366, NHNH=6.366, HNHN=6.366

Takeaway
- With determinization enabled at K=3 and strict deterministic budgets, aggregate results remain equal across mixes/seats for this window.
- Next: keep relying on the strict flip golden to validate continuation behavior, and consider broader ranges or alternative levers (e.g., small default K for Hard, slightly wider PhaseB top‑K only under Wide, or minimal next3 branching under caps) before proposing any default changes.
