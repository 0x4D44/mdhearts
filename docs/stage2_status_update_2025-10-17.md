# Stage 2 Status Update — 2025-10-17

## Recent Progress
- Completed Stage 2 vs. Stage 1 control benchmarks (`bench/stage2_pass_moon.yaml` with `pass_v2` and `pass_v1`) and captured telemetry + Markdown summaries.
- Ran the follow-up benchmarks with `MoonEstimatorConfig::block_threshold=0.45` (initial `bench/out/stage2_pass_moon_v045/` and current selective run `bench/out/stage2_pass_moon_v045_selective/`), confirming the target block ratio and refreshing docs/plots.
- Added regression coverage for first-trick edge cases to ensure the refined penalty safeguards remain stable.
- Extended analytics tooling:
  - Added high-probability failure log (`docs/benchmarks/stage2_block_failures.md`): all remaining misses are inverted moons, concentrated on West/North passes.
  - `tools/analyze_telemetry.py` now outputs block-shooter ratios and CSV rows.
  - `tools/compare_pass_runs.py` and `tools/telemetry_to_markdown.py` provide Markdown-ready comparison tables.
  - `tools/analyze_block_shooter.py` correlates block passes with deal results (pass_v045_selective: 2,010 / 2,071 block passes prevented a moon; pass_v1 emits none).
- Updated documentation (`README.md`, benchmark notes, Stage 2 summaries) with cross-platform test guidance and telemetry workflows, plus new block-failure drill-down (`tools/list_block_failures.py`).

## Test Matrix
- `cargo test -p hearts-core`
- `cargo test --workspace --exclude hearts-app --exclude hearts-ui` *(WSL-safe command; Win32 crates still require running on a Windows host)*

## Key Findings
- Pass_v045_selective retains positive PPH for the competitive seats (+0.03 to +1.21) while still cutting the Easy bot’s dumping edge (−1.54 PPH vs. pass_v1).
- Block-shooter objectives now trigger on ~18.7 % of passes / ~9.3 % of plays, preventing moons 97.1 % of the time; legacy pass emits no pass telemetry.
- Rule adjustments allow hearts only when truly forced and keep Q♠ forbidden even without off-suit outs.

## Next Steps
1. Integrate telemetry CSV outputs into analytics dashboards / acceptance deck (charts for PPH, block ratios).
2. Tune block-shooter weights by correlating telemetry events with deal outcomes (see `bench/out/block_shooter_summary_v045_selective.(csv|json)`), targeting inverted-moon failures (20 cases ≥ 0.60 probability with shooters West 8, South 5, North 5, East 2; see `docs/benchmarks/stage2_block_failures.md`).
3. Design and test targeted heuristics (directional liability + high-heart passes); note the first prototype (`bench/out/stage2_pass_moon_v045_directional/`) showed no improvement, so iterate before the next benchmark rerun.
3. Add regression tests for optimizer bounds and flag fallbacks (`MDH_PASS_V2`, `MDH_ENABLE_BELIEF`).
4. Run full workspace tests (including `hearts-app`) on a Windows host to verify Win32 components once available.
