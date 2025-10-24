Release Notes — AI Improvements (2025‑10‑22)

Highlights
- Heuristic (Normal): endgame behavior polished; added non‑QS hearts‑feed golden.
- Hard (FutureHard): deterministic budget + top‑K continuation with tiny, env‑tunable continuation signals; verbose explain and JSON.
- Evaluation: cross‑platform deterministic helpers (PowerShell/Bash); disagreement and match CSVs; bench summaries.
- Tests: endgame feed cap, hearts feed (non‑QS), moon transition smoke, near‑tie constructed mid‑trick cases.
- Docs: contributor guide, artifacts index, Stage 6/7 plans, handoff guide; CLI docs updated.
- CI: GitHub Actions for build/test on Windows/Linux + PR eval smoke.

Upgrade Notes
- No default AI weight changes; env‑gated toggles (tie‑break, probe/pruning, continuation extras) remain OFF by default to keep goldens stable.
- Use eval helpers (`tools/run_eval.ps1` or `tools/run_eval.sh`) to reproduce CSVs; see `designs/2025.10.22 - Tuning Artifacts Index.md`.

Acknowledgements
- Thanks to contributors for ideas and reviews during Stage 5–7.
