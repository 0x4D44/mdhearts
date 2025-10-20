# Stage 2 Progress Report — 20 October 2025

## Overview
- Restored the guard heuristic baseline to the guardfix22j behaviour to keep the pass pipeline stable while diagnostics continue.
- Added deterministic pass-level logging to the Stage 2 harness (`pass_details.jsonl`), providing full visibility into every pass combination, including hands beyond the legacy telemetry ceiling.
- Executed a fresh Stage 2 benchmark (`stage2_pass_moon_v045_guardfix22l`) with the new instrumentation and refreshed documentation to reflect the latest metrics and outstanding problem hands.

## Stage 2 Telemetry (guardfix22l)
- Runs: 1,024 hands × 4 permutations (seed 20251017).
- Pass events: **14 254**; success rate: **100 %**.
- Average best-vs-next margin: **16.48** (previous guardfix22j: 16.39).
- Baseline moon rate (normal): **0.44 %** (18 North self moons; no partner/opponent moons).
- Self-moon seeds (North): 43/76/367/462/464/481/495/607/865/890/941/999 plus high-index permutations 887/913/999 (complete pass mixes now logged).
- Baseline_easy still leads by **+2.48 PPH** over baseline_normal.

## Guard / Enumerator Status
- Guard heuristics are back at the guardfix22j baseline; `force_guarded_pass` behaviour matches the last known green state and all targeted regression fixtures pass.
- Outstanding moon scenarios continue to stem from Q♠ anchors (43/76/481/495), premium heart dumps (462/464/865/890/887/913/999), and spade-heavy voids (495/941/999).

## Telemetry & Tooling Improvements
- `hearts-bench` now records `pass_details.jsonl` alongside the existing telemetry, enabling targeted analysis of any pass without re-running the benchmark.
- Documentation updated:
  - `docs/stage2_progress_summary_2025-10-19.md` now reflects the new instrumentation and guardfix22l metrics.
  - `docs/benchmarks/stage2_block_failures_guardfix19.md` includes a guardfix22l section with the latest self-moon table.
  - `docs/benchmarks/stage2_pass_moon_runs.csv` logs the guardfix22l run for historical comparisons.

## Testing Status
- `cargo test --package hearts-core` — ✅
- `cargo test --package hearts-bench` — ✅
- Full Stage 2 benchmark (`stage2_pass_moon_v045_guardfix22l`) — ✅ (artefacts under `bench/out/stage2_pass_moon_v045_guardfix22l/`).

## Remaining Gaps
1. **Liability mix regressions**: Hands 43/76/481/495 continue to ship Q♠ plus weak support; hand 495 in particular still prefers triple spade dumps.
2. **Premium heart anchors**: Hands 462/464/865/890/887/913/999 retain high-heart mixes that strand liability coverage.
3. **Margin & scoring**: Average best-vs-next margin remains >16, and baseline_easy’s +2.48 PPH advantage indicates direction/penalty weights still overvalue aggressive passes.

## Next Steps
1. Use the new pass-detail snapshots to design focused guard and enumerator tweaks for the residual Q♠ anchors and premium heart mixes (starting with 43/76/481/495 and 462/464/865/890).
2. Re-introduce guard changes incrementally with dedicated regression coverage for each affected seed to keep `force_guarded_pass` stable and avoid `None` fallbacks.
3. Begin the scoring/weight audit (liability vs. moon penalty vs. direction bonus) to push average margins into single digits and close the baseline_easy performance gap.
4. Once the next guard iteration lands, repeat the Stage 2 benchmark and update documentation/run logs to track progress against these targets.
