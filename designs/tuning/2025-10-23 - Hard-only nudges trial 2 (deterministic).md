Config
- Deterministic caps: MDH_HARD_DETERMINISTIC=1, MDH_HARD_TEST_STEPS=120
- Hard-only env nudges (not defaults):
  - MDH_HARD_WIDE_PERMIL_BOOST_FEED=700
  - MDH_HARD_WIDE_PERMIL_BOOST_SELFCAP=300
  - MDH_HARD_PLANNER_LEADER_FEED_NUDGE=12
  - MDH_HARD_PLANNER_MAX_BASE_FOR_NUDGE=800
  - MDH_HARD_PHASEB_TOPK=8

Run
- Seat=West, seeds 1000..1499 (n=500)
- NNHH West(Hard) avg_pen = 6.366
- HHNN West(Normal) avg_pen = 6.366

Takeaway
- Even with stronger (but still capped) Wide-tier continuation boosts and a small planner leader-feed nudge, aggregate difference remains negligible on this window.
- Next: rely on the strict flip golden (now enabled) to guard continuation behavior; consider broader ranges or alternative levers (e.g., deterministic K defaults, deeper selective probe) if we seek a measurable +1–2 pts/hand gap.
