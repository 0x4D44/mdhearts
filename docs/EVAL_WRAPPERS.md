# Eval Wrappers

This repo includes small helpers to run quick, deterministic checks around the feature‑gated Stage 1/2 logic.

Quick smoke (flags ON)
- `tools/smoke_fast.sh [seed_start]` with `SMOKE_COUNT` (default 2). Generates CSVs under `designs/tuning/stage1/smoke_release/` and updates the index.

Compare (Normal vs Hard, disagreements only)
- Small: `tools/compare_small.sh <seed_start> <count>` (fast Hard limits; OFF vs ON flags; writes CSVs to `designs/tuning/stage1/compare_release/`).
- Medium: `tools/compare_medium.sh <seed_start> <count>` (same outputs).
- Index: `tools/index_compare.sh designs/tuning/stage1/compare_release` → updates `INDEX.md`.

All‑in‑one
- `tools/eval_all.sh [seed_start]` (envs: `SMOKE_COUNT`, `SMALL_COUNT`, `MED_COUNT`).
  - Runs smokes, checks thresholds (header+rows), runs small+medium compares, writes a dated summary to `designs/tuning/eval_summaries/`.
 - Optional wrapper: `tools/nightly_eval.sh [seed_start]` sets modest counts and calls `eval_all.sh`.

Feature Flags
- Enable both Stage 1/2 via env: `MDH_FEATURE_HARD_STAGE12=1`, or individually `MDH_FEATURE_HARD_STAGE1=1`, `MDH_FEATURE_HARD_STAGE2=1`.
- CLI: add `--hard-stage12`, `--hard-stage1`, `--hard-stage2` to supported commands.
