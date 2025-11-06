# CLI Smoke Recipes (Ultra-Fast)

The commands here provide quick, deterministic sanity checks for Stage 1 behavior without long runs.

Ultra-fast per-seat (1 seed)
```
MDH_HARD_DETERMINISTIC=1 MDH_HARD_TEST_STEPS=60 \
  mdhearts --match-mixed <seat> 100 1 <mix> \
  --hard-steps 60 --hard-branch-limit 150 --hard-next-branch-limit 80 --hard-time-cap-ms 5 \
  --stats --out tmp/stage1_ci_<seat>_<mix>_smoke_fast1.csv
```
- Replace `<seat>` with `west|east|south|north` and `<mix>` with `nnhh|hhnn`.
- Outputs can be archived under `designs/tuning/stage1/smoke_release/`.

Helper script
- `tools/smoke_fast.sh [seed_start]` runs smokes across all seats (both NNHH and HHNN) and copies outputs into the archive folder.
- Control the number of seeds with `SMOKE_COUNT` (default 2) and the start with the positional `[seed_start]` (default 100).

Feature flags
- To run new Stage 1/2 logic while continuing on main, enable the runtime flags (default OFF):
  - Env: `MDH_FEATURE_HARD_STAGE12=1` (both), or individually `MDH_FEATURE_HARD_STAGE1=1` / `MDH_FEATURE_HARD_STAGE2=1`.
  - CLI (where supported): `--hard-stage12`, `--hard-stage1`, `--hard-stage2`.
- The smoke helper already enables `MDH_FEATURE_HARD_STAGE12=1`.

Eval helpers
- `tools/run_eval.sh` now honors `MDH_FEATURE_HARD_STAGE12=1` and will pass `--hard-stage12` to supported commands.
- Optional thresholds: `tools/check_smoke_thresholds.sh <dir> [min_lines]` to assert minimum CSV rows (defaults to 2 = header+1 row).

Small compare (Normal vs Hard) — quick disagreements only
```
# Fast limits for Hard explain; flags OFF by default
MDH_HARD_DETERMINISTIC=1 MDH_HARD_TEST_STEPS=60 MDH_HARD_BRANCH_LIMIT=150 MDH_HARD_NEXT_BRANCH_LIMIT=80 MDH_HARD_TIME_CAP_MS=5 \
  mdhearts --compare-batch west 150 5 --only-disagree --out tmp/compare_off_west.csv

# Same, flags ON (both stages)
MDH_FEATURE_HARD_STAGE12=1 MDH_HARD_DETERMINISTIC=1 MDH_HARD_TEST_STEPS=60 MDH_HARD_BRANCH_LIMIT=150 MDH_HARD_NEXT_BRANCH_LIMIT=80 MDH_HARD_TIME_CAP_MS=5 \
  mdhearts --compare-batch west 150 5 --only-disagree --out tmp/compare_on_west.csv
```
See also `tools/compare_small.sh` for a 4-seat convenience wrapper.
