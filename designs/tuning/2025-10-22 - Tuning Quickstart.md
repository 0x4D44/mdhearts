Tuning Quickstart (Hard and Normal)

Goals
- Compare Normal vs Hard decisions deterministically and harvest disagreements for goldens.
- Explain and inspect continuation parts (Hard) without environment variables where possible.

Basics
- Show active weights:
  - mdhearts --show-weights
- Single snapshot explain (Normal by default):
  - mdhearts --explain-once <seed> <seat> [difficulty]

Hard flags (pass to supported commands)
- --hard-deterministic — deterministic, step-capped scanning
- --hard-steps <n> — step cap when deterministic
- --hard-phaseb-topk <k> — probe continuation only for top‑K
- --hard-branch-limit <n>, --hard-next-branch-limit <n>
- --hard-time-cap-ms <ms>, --hard-cutoff <margin>
- --hard-cont-boost-gap <n>, --hard-cont-boost-factor <n>

Examples
- Deterministic compare for one seed/seat and print flags/stats:
  - mdhearts --compare-once 1040 west --hard-deterministic --hard-steps 80
- Deterministic batch; write only disagreements to CSV:
  - mdhearts --compare-batch west 1000 50 --only-disagree --hard-deterministic --hard-steps 80 --out designs/tuning/compare_west_1000_50.csv
- Hard verbose explain (when MDH_DEBUG_LOGS=1):
  - set MDH_DEBUG_LOGS=1 then mdhearts --explain-once <seed> <seat> hard --hard-deterministic --hard-steps 80

Promoting goldens
- Prefer deterministic mode (flags above) for stability.
- Use disagreeing seeds to craft tests that assert inequality (Normal != Hard) rather than exact cards, unless you intend to lock exact choices.

