## 2025-10-22 — Hard vs Normal tuning experiments

Setup
- Deterministic Hard (steps=120), mixed-seat harness `--match-mixed` with mix=NNHH (N/E=Normal, S/W=Hard).
- One round per seed, 150 seeds per seat; averaged hard seats (S,W) vs normal seats (N,E).

Baseline
- hard_avg=6.48 normal_avg=6.52 (delta=+0.04 normal over hard; essentially a tie)

Experiment A (tiny boosts)
- Weights: FEED_PER=70 SELF_PER=90 TOPK=8 GAP=60 FACT=2 SINGLETON=35 HANDOFF=20
- Result: hard_avg=6.48 normal_avg=6.52 (delta=+0.04; unchanged)

Experiment B (stronger boosts + cap)
- Weights: FEED_PER=80 SELF_PER=100 TOPK=10 GAP=100 FACT=3 HEARTS_PER=1 HANDOFF=30 CAP=300
- Result: hard_avg=6.48 normal_avg=6.52 (delta=+0.04; unchanged)

Notes
- With current single-round/seed harness and conservative continuation integration, these env-only boosts did not shift the aggregate averages in this slice.
- Hard continuation remains tiny relative to base scores; improvements likely require either:
  1) stronger continuation influence in planner scoring (beyond current tiny parts), or
  2) deeper/more selective lookahead under strict budgets, or
  3) scenario-weighted eval (more hands where continuation matters), not uniform random seeds.

Next ideas
- Add a second-round continuation probe (already scaffolded) with slightly wider branch caps only when leverages are high (near-penalty tricks, leader targeting opportunities).
- Incorporate a small planner-level bias that mirrors continuation effects (e.g., leader-feed on penalty tricks) to reinforce direction.
- Curate evaluation seeds emphasizing mid-trick decisions and endgame, then re-run mixed-seat comparisons.
