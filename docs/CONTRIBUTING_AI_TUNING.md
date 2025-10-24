# Contributing — AI Tuning & Evaluation

This guide describes a safe, reproducible workflow for iterating on the AI (Normal heuristic and Hard continuation/search scaffold) without destabilizing behavior.

## Prerequisites
- Rust stable installed; repo builds and tests pass: `cargo test --all`
- Prefer deterministic Hard for repeatable results:
  - `MDH_HARD_DETERMINISTIC=1`
  - `MDH_HARD_TEST_STEPS=<n>` (e.g., `120`)

## Quick Tools
- Show weights: `cargo run -p hearts-app -- --show-weights`
- Explain once:
  - Normal: `--explain-once <seed> <seat>`
  - Hard: `--explain-once <seed> <seat> hard --hard-verbose`
- Compare batch (Normal vs Hard): `--compare-batch <seat> <seed_start> <count> [--out <csv>] [--only-disagree]`
- Match batch (A vs B): `--match-batch <seat> <seed_start> <count> [difficultyA difficultyB] [--out <csv>]`
- Deterministic sweep helper: `powershell -ExecutionPolicy Bypass -File tools/run_eval.ps1 -Verbose`

## Tuning Workflow (env‑only first)
1. Freeze baseline
   - Run `tools/run_eval.ps1` to generate compare/match CSVs and a summary.
   - Save a few `--explain-once` verbose outputs for seeds of interest.
2. Adjust via environment variables (do not change defaults initially)
   - Normal: `MDH_W_*` (see README Configuration)
   - Hard: `MDH_HARD_CONT_*`, `MDH_HARD_CTRL_*`, optional tie‑break (`MDH_HARD_CONT_BOOST_*`)
3. Re‑evaluate
   - Re‑run `--compare-batch --only-disagree` and/or `tools/run_eval.ps1`.
   - Inspect disagreements and verbose continuation parts.
4. Add/adjust tests
   - Prefer constructed, deterministic goldens that assert clear behavior (e.g., leader feed, endgame, moon relief).
   - Clean up any env vars in tests to avoid cross‑test interference.
5. Promote defaults (only if justified)
   - If match summaries improve and goldens remain stable, carry env changes into code defaults.
   - Update README/docs if knobs are added/changed.
6. Record and index
   - Update `designs/journal/<date> - AI bot journal.md` with a short entry.
   - Update `designs/2025.10.22 - Tuning Artifacts Index.md` if new artifact types were added.

## Acceptance Checklist
- All tests pass: `cargo test --all`
- No unintended regressions in existing goldens
- New goldens are deterministic and minimal
- Evaluation artifacts saved under `designs/tuning/` with brief notes in the journal

## Tips
- Keep `MDH_CLI_POPUPS` unset so runs don’t show message boxes (console only).
- Seeds differ across toolchains; use deterministic mode for reproducibility.
- Keep continuation weights tiny by default; rely on env toggles during experiments.
