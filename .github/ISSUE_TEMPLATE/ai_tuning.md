---
name: AI Tuning Proposal
about: Propose a change to AI weights/heuristics/search and how to validate it
title: "AI Tuning: <short description>"
labels: enhancement, ai
assignees: ''
---

## Summary
Describe the proposed change ( Normal / Hard ) and the intent (e.g., better leader feed, safer endgame, moon alignment ).

## Evidence / Repro
- Deterministic commands/files (paste commands and link artifacts):
  - `tools/run_eval.(ps1|sh)` summary: 
  - Compare CSVs: 
  - Match CSVs: 
  - Verbose explains (`--hard-verbose` with `MDH_DEBUG_LOGS=1`): 

## Impact
- Expected effect on disagreements (counts, examples):
- Expected effect on match averages (per seat):
- Performance considerations (avg/p95 µs):

## Tests
- New/updated goldens (constructed preferred):
- Determinism (env cleanup, step caps):

## Rollout
- Start as env-only (do not change defaults) and re-evaluate.
- Promote defaults only if goldens stable and match summaries improve.

