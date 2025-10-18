# Stage 2 Progress Snapshot — 2025-10-17

## Highlights
- Completed Stage 2 vs. Stage 1 control benchmarks (`bench/stage2_pass_moon.yaml`) with telemetry, Markdown, CSV, and comparison artefacts.
- Added first-trick regression coverage to lock in the refined penalty safeguards (no Q♠ dumps, hearts only when forced).
- Enhanced telemetry tooling:
  - `tools/analyze_telemetry.py` → JSON/Markdown/CSV summaries (block ratios, averages).
  - `tools/compare_pass_runs.py` / `tools/telemetry_to_markdown.py` → deck-ready tables.
  - `tools/analyze_block_shooter.py` → block-pass success metrics (pass_v045_selective: 2,010 / 2,071 = **97.1 %** success).
- Generated Stage 2 chart assets via `tools/render_stage2_plots.py` (outputs in `docs/benchmarks/plots/`) and refreshed deck Slide 5 visuals.
- Tuned `MoonEstimatorConfig::block_threshold` to **0.45**; the selective moon-scaling benchmark (`bench/out/stage2_pass_moon_v045_selective/`) confirms an **18.7 %** pass block ratio with **97.1 %** success (see `docs/stage2_metrics_digest.md`).
- Updated pass-vs-control comparison (`docs/benchmarks/stage2_pass_comparison.md`): Easy seat drops **1.54 PPH**, competitive seats retain modest gains (+0.03 to +1.21 PPH) under the tighter block trigger.
- Captured the run catalogue in `docs/benchmarks/stage2_pass_moon_v045.md` (initial), `docs/benchmarks/stage2_pass_moon_v045_selective.md` (current default), and the legacy control report.
- Added high-probability failure log via `tools/list_block_failures.py` (`docs/benchmarks/stage2_block_failures.md`) to pinpoint remaining moon threats.
- Directional liability experiment (`bench/out/stage2_pass_moon_v045_directional/`) produced no improvement; reverting to selective baseline while pursuing targeted rules.
- Observations: all current high-probability failures are inverted moons; West/North planners account for 16/20 cases, guiding directional liability tweaks and high-heart handoff heuristics before the next run.
- Documented acceptance deck outline (`docs/stage2_pass_deck.md`) and block-shooter analysis plan (`docs/stage2_block_shooter_analysis_plan.md`).
- Test coverage refreshed: `cargo test -p hearts-core` and `cargo test --workspace --exclude hearts-app --exclude hearts-ui` (WSL-safe command).

## Next Steps
1. Validate the plotting pipeline and metrics digest across additional variants (belief-off, legacy control refresh) now that the 0.45 threshold run is in place; adjust colour palettes for accessibility as needed.
2. Tune moon estimator + block-shooter weights using the success-rate bins; quantify moon prevention vs. estimator thresholds.
3. Hook the `tools/aggregate_stage2_metrics.py` / `docs/stage2_metrics_digest.md` outputs into dashboards and automated deck export.
4. Extend automation/notebooks to ingest `telemetry_summary.json` + block summaries for dashboards.
5. Run the full workspace suite on a Windows host to validate `hearts-app` / `hearts-ui`.
