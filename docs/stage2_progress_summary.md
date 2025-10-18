# Stage 1 & Stage 2 Progress Report (October 17, 2025)

## Completed Work

- **Stage 1 Belief System**
  - Implemented `Belief`, soft likelihood updates, sampler/cache, and controller integration with incremental updates.
  - Added structured telemetry (`mdhearts::belief`) including entropy metrics and belief hashes derived from `Belief::summary_hash`.
  - Documented runtime toggles (`MDH_ENABLE_BELIEF`, `MDH_BELIEF_SOFT_*`, `MDH_BELIEF_VOID_THRESHOLD`) in `docs/benchmarks/README.md`.
  - Added targeted unit coverage (belief void inference, pass planner fallback, controller belief initialisation).

- **Stage 2 Foundations**
  - Created `crates/hearts-core/src/pass/` with direction profiles (`DirectionProfile`) and scoring primitives (`PassScoreInput`, `PassScoreBreakdown`, `score_card`).
  - Introduced `BotFeatures::pass_v2` toggle exposed via `MDH_PASS_V2`, propagated through `PolicyContext`.
  - Prototyped pass_v2 candidate evaluation inside `PassPlanner::choose`:
    - Scores single cards with belief-aware heuristics.
    - Enumerates top combinations, adds void-synergy & direction bonuses.
    - Logs telemetry events at INFO level (`hearts_bot::pass_decision`) with selected cards, component scores, belief hash, and top alternatives.
  - Added tests for pass_v2 behaviour (with and without beliefs) and belief hash stability.
  - Replaced ad-hoc combo loops with shared optimizer (`enumerate_pass_triples`) that returns ranked `PassCandidate`s, includes pruning, synergy weighting, and ensures Queen of Spades coverage; updated pass_v2 to consume the optimizer directly.
  - Added moon estimator scaffolding (`crates/hearts-core/src/moon`): logistic features, probability/objective output, integration into pass scoring (`moon_support`), and telemetry of moon probability/objective.
  - Introduced selective moon-support scaling in `compute_moon_support`: high-urgency passes now boost weighting only for key liabilities (Q♠, high hearts) instead of global penalties.
  - `BotContext` now captures the moon estimate once per decision; pass telemetry and play heuristics consume the derived `Objective::{MyPointsPerHand, BlockShooter}`, with baseline blocking bonuses wired into `PlayPlanner`.
  - Added regression tests ensuring block-shooter objectives boost capture scoring and steer `PlayPlanner::choose` toward heart-taking lines when moon risk spikes.
  - Stage 0 harness exposes pass rankings and moon metrics in `telemetry.jsonl` via new logging toggles (`logging.pass_details`, `logging.moon_details` or `--log-pass-details/--log-moon-details`) that set `MDH_PASS_DETAILS`/`MDH_MOON_DETAILS` for the heuristic bots.
    - Example pass event:
      ```json
      {"seat":"West","direction":"Left","cards":["Card { rank: Five, suit: Hearts }","Card { rank: King, suit: Hearts }","Card { rank: Ace, suit: Hearts }"],"total":111.6856,"moon_probability":0.50947,"moon_objective":"block_shooter","top_scores":[111.6856,102.844864,102.844864]}
      ```
  - Harness post-processing now emits `telemetry_summary.json` / `.md` with pass ranking and moon objective aggregates, appends a **Telemetry Highlights** block to `summary.md`, and the CLI surfaces best-vs-next margins/objective mix. `tools/analyze_telemetry.py` mirrors the logic with `--json/--markdown` for notebook workflows.
  - Completed Stage 2 benchmark run (`bench/stage2_pass_moon.yaml`, 4,096 deals). Observed avg pass score 151.1, moon probability 0.361, and block-shooter activation on ~37% of passes / ~28% of plays; results archived in `docs/benchmarks/stage2_pass_moon_2025-10-17.md`.
  - Follow-up benchmark with the 0.45 block threshold (`bench/out/stage2_pass_moon_v045/`) confirmed an 18.6 % activation rate and ~97 % success; the refined selective scaling run (`bench/out/stage2_pass_moon_v045_selective/`) maintains the 18.7 % rate with 97.1 % success; see `docs/benchmarks/stage2_pass_moon_v045*.md`.
  - Ran pass_v1 control benchmark with identical config (`docs/benchmarks/stage2_pass_moon_control_2025-10-17.md`), highlighting a +0.67 PPH gain for the flagship seat and zero pass telemetry under legacy logic.
  - Added `tools/compare_pass_runs.py` to generate side-by-side Markdown tables for benchmark runs (`docs/benchmarks/stage2_pass_comparison.md`).
- Enhanced `tools/analyze_telemetry.py` with block-shooter ratios and CSV export (`telemetry_summary.csv`) plus a helper `tools/telemetry_to_markdown.py` that turns CSV rows into documentation-ready tables (`docs/benchmarks/stage2_pass_block_ratios.md`).
- Introduced `tools/analyze_block_shooter.py`, joining pass telemetry with deal outcomes: pass_v045 block passes succeed 97.0 % of the time (1,994 / 2,056) while pass_v1 emits none (initial pass_v2 baseline was 6,030 / 6,245).
- Added deck-ready automation:
  - `tools/aggregate_stage2_metrics.py` → consolidated digest (`docs/stage2_metrics_digest.md`).
  - `tools/render_stage2_plots.py` → chart assets under `docs/benchmarks/plots/`.
  - `tools/sweep_block_threshold.py` → what-if analysis on moon estimator thresholds; rerun sweep + benchmark confirm `block_threshold = 0.45` yields an 18.6 % activation rate with 97 % success (`docs/stage2_block_threshold_sweep.md`, `docs/stage2_metrics_digest.md`).
  - `tools/list_block_failures.py` → extracts high-probability block failures (report in `docs/benchmarks/stage2_block_failures.md`) to guide focused tuning.
- Updated `MoonEstimatorConfig::block_threshold` default to **0.45** and validated the change with `bench/out/stage2_pass_moon_v045/`; telemetry now matches the target activation band.
- Trialled urgency-scaled moon-support weighting (`bench/out/stage2_pass_moon_v045_tuned*/`); reduced high-probability failures slightly but regressed PPH, so reverted the heuristic change for now. Artefacts retained for analysis.

## In-Flight / Pending Items

- **Moon Estimation & Objective Switching (Stage 2 Milestones M2.3–M2.4)**
  - Tune block-shooter weighting in `PlayPlanner` and extend coalition heuristics for mid-hand moon defence.
  - Add coalition scoring hooks to `PlayPlanner` and integrate moon objectives into partner handoff logic.

- **Pass Optimizer Refinement (Stage 2 Milestones M2.1–M2.2)**
  - Tune optimizer weights with data-driven thresholds (card pool sizing, moon/void multipliers).
  - Extend regression coverage for optimizer edge cases (e.g., short hands, duplicate scoring ties).

- **Telemetry & Harness Integration**
  - Dashboards still need to ingest the new `telemetry_summary.json` artefact (notebook wiring pending).

- **Benchmarking & Validation (Stage 2 Milestone M2.5)**
  - Run 10k-deal A/B harness comparing pass_v2 with legacy logic; compute PPH deltas & Wilcoxon stats.
  - Capture moon-defense scenarios to verify coalition logic.

## Next Steps (Immediate Focus)

1. Build comparison visuals/tables for pass_v2 vs. pass_v1 (PPH deltas, block-pass success rates, moon activation) to feed the Stage 2 acceptance package.
2. Fold the new pass_v045 results into automated analytics (dashboards/notebooks) and contrast against the initial pass_v2 baseline.
3. Automate parsing of `telemetry_summary.json` / block-shooter CSVs into analytics notebooks (focus: pass ranking distribution, moon objective frequency).
4. Add regression tests for optimizer boundedness and feature flag fallbacks (`MDH_PASS_V2`, `MDH_ENABLE_BELIEF`).
5. Prepare benchmarking scripts/runbooks for Stage 2 acceptance (PPH improvement & moon defense metrics).

_Prepared by: Stage 1/2 implementation agent on 2025-10-17._
