Title: Hard budget/branching evaluation (deterministic)

Config
- Seeds: 1000..1499 (n=500) per seat
- Mixed seats: nnhh and hhnn
- Deterministic: MDH_HARD_DETERMINISTIC=1, MDH_HARD_TEST_STEPS=120
- Hard knobs (env):
  - MDH_HARD_TIME_CAP_MS=10
  - MDH_HARD_BRANCH_LIMIT=10
  - MDH_HARD_NEXT_BRANCH_LIMIT=8
  - MDH_HARD_PHASEB_TOPK=8
  - MDH_HARD_CONT_CAP=400
  - MDH_HARD_NEXT3_ENABLE=1

CLI example
```
mdhearts --match-mixed north 1000 500 nnhh --hard-deterministic --hard-steps 120
mdhearts --match-mixed north 1000 500 hhnn --hard-deterministic --hard-steps 120
```

Results (mean penalties per hand ±95% CI)
- mix=nnhh: Hard 6.492 ± 0.402, Normal 6.508 ± 0.408, delta (N−H) = +0.016 (N=1000 each)
- mix=hhnn: Hard 6.508 ± 0.408, Normal 6.492 ± 0.402, delta (N−H) = −0.016 (N=1000 each)

Takeaway
- Increasing Hard’s time/branch budgets within tight caps improves compute time utilization (see benches) but does not create a measurable advantage over Normal in aggregate with current conservative continuation weights. Net delta ~0 across mixes.

Next steps (proposal)
- Keep the above budget/branch settings, and layer small, Wide-tier-only strength increases:
  - Slightly raise Wide-tier continuation boosts per penalty (feed/self-cap) and allow a small next-probe widening.
  - Add a tiny Hard-only current-trick leader-target nudge under Wide tier when not capturing and penalties>0.
- Re-run mixed-seat evaluation (≥1000 seeds/seat) and promote defaults only if delta ≥ +1.0 in (Normal − Hard).

