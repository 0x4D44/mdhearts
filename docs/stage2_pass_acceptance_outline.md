# Stage 2 Passing & Moon Objectives — Deck Outline (Draft)

## Slide 1 — Context & Goals
- Recap Stage 2 objectives: direction-aware passing, moon estimator, telemetry, PPH improvement.
- Note baseline issues observed in Stage 1 (point dumping, lack of visibility).

## Slide 2 — Implementation Highlights
- Direction profile + optimizer integration (`pass_v2`), belief-aware scoring.
- Moon estimator thresholds and block-shooter objective switching.
- Telemetry instrumentation: INFO events, harness flags, summary artefacts.

## Slide 3 — Benchmark Configuration
- `bench/stage2_pass_moon.yaml`: 1,024 hands × 4 perms.
- Env flags: `MDH_PASS_V2=1`, `MDH_ENABLE_BELIEF=1`, logging toggles.
- Compare with control run (`MDH_PASS_V2=0`).

## Slide 4 — PPH Outcomes
- Table of pass_v2 vs. pass_v1 averages (use `docs/benchmarks/stage2_pass_comparison.md`).
- Highlight gains for competitive seats (+0.67 to +1.69 PPH) and Easy bot regression.

## Slide 5 — Moon Objective Telemetry
- Pass vs. play block-shooter ratios (0.572 vs. 0; play ~0.28).
- Show histogram or simple chart derived from `telemetry_summary.csv`.

## Slide 6 — Rule & Planner Safeguards
- First-trick Q♠ prevention with void/penalty logic.
- Candidate filtering and empty-legal fallback to stabilise simulations.

## Slide 7 — Telemetry & Tooling
- Summaries: `telemetry_summary.(json|md|csv)`, `tools/compare_pass_runs.py`.
- New CSV output for dashboards; mention `docs/stage2_pass_summary_2025-10-17.md`.

## Slide 8 — Next Steps
- Tune block-shooter weights via telemetry analysis.
- Add regression coverage (first-trick edge cases, optimizer bounds).
- Integrate metrics into analytics dashboards / final Stage 2 sign-off process.
