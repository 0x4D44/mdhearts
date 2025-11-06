# Eval Summaries

This folder contains dated markdown reports produced by `tools/eval_all.sh` (and `tools/nightly_eval.sh`).

Each summary captures:
- Seed start and counts
- Smoke threshold status
- Small/medium compare status and basic counts

To generate:
```
# Quick local run with small counts
SMOKE_COUNT=2 SMALL_COUNT=3 MED_COUNT=6 tools/eval_all.sh 200
```
