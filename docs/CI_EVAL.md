# CI Eval (manual)

This repository includes a manual GitHub Actions workflow to run the feature‑flag eval helpers end‑to‑end and upload artifacts.

How to run
- In GitHub → Actions → "Eval (manual)", click "Run workflow".
- Optional inputs (defaults shown):
  - `seed_start`: 200
  - `smoke_count`: 2 (`SMOKE_COUNT`)
  - `small_count`: 3 (`SMALL_COUNT`)
  - `med_count`: 6 (`MED_COUNT`)

What it does
- Builds release binary.
- Runs `tools/eval_all.sh` with the provided inputs (flags ON via `MDH_FEATURE_HARD_STAGE12=1`).
- Indexes compare outputs (`tools/index_compare.sh`).
- Uploads artifacts:
  - `designs/tuning/stage1/smoke_release/*.csv` and `INDEX.md`
  - `designs/tuning/stage1/compare_release/*.csv` and `INDEX.md`
  - `designs/tuning/eval_summaries/*.md`

Notes
- The eval workflow is manual (`workflow_dispatch`) and keeps counts modest by default to run quickly.
- For scheduled/nightly runs, prefer `tools/nightly_eval.sh` executed outside GitHub or via a separate scheduler.
