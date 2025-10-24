Setup
- Deterministic: MDH_HARD_DETERMINISTIC=1, MDH_HARD_TEST_STEPS=120.
- Hard env nudges (not defaults):
  - MDH_HARD_WIDE_PERMIL_BOOST_FEED=300
  - MDH_HARD_WIDE_PERMIL_BOOST_SELFCAP=120
  - MDH_HARD_PLANNER_LEADER_FEED_NUDGE=5
  - MDH_HARD_PLANNER_MAX_BASE_FOR_NUDGE=500

Runs (seat=West, seeds 1000..1299, n=300)
- NNHH West(Hard) avg_pen ≈ 6.387
- HHNN West(Normal) avg_pen ≈ 6.387

Takeaway
- As expected for a small deterministic slice, these tiny nudges do not move the aggregate materially in this window.
- Proceed with strict flip golden and consider slightly stronger but still capped nudges if goldens remain stable.
