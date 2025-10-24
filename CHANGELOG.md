Changelog

All notable changes to this project will be documented here.

2025-10-22 — AI Improvements, Evaluation, and Handoff
- Heuristic (Normal) planner: endgame polish retained; added non‑QS hearts‑feed golden.
- Hard (FutureHard): deterministic budget, top‑K continuation, tiny continuation signals (env‑tunable) and verbose explain.
- New tests: endgame feed cap, non‑QS hearts feed, moon transition smoke, near‑tie constructed cases.
- Tools: cross‑platform deterministic evaluation helpers (`tools/run_eval.ps1`, `tools/run_eval.sh`).
- Docs: contributor tuning guide, CLI tools polish, artifacts index, Stage 6/7 plans, handoff guide.
- CI: builds/tests on Windows/Linux; PR smoke runs eval helper with tiny ranges.

Notes:
- Env‑gated toggles (tie‑break, probe/pruning) remain off by default to preserve goldens.
- Use the evaluation helpers to reproduce CSVs/summary under `designs/tuning/`.
