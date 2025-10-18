# Stage 2 Progress Snapshot — 2025-10-18

## Summary of Recent Work
- Replaced soft penalties with hard rejections for single-support A♥/K♥ left passes. The optimizer now refuses any left pass that sends a premium heart without two additional Ten+ hearts (`HARD_REJECTION_PENALTY`).
- Updated `enumerate_pass_triples` to drop illegal triples before scoring and attempt replacement Ten+ heart candidates; added targeted regression cases ensuring compliant passes remain finite.
- Reran the Stage 2 benchmark multiple times (handshake19–22). Illegal premium dumps are gone, but fallback Q♠/club passes now dominate the remaining ≥ 0.60 failures.
- Captured new telemetry, failure tables, and progress notes for each run (docs/benchmarks/, docs/stage2_block_failure_analysis_*, docs/stage2_pass_progress_summary_2025-10-18.md).

## Current Status
- Latest run handshake22: **97.0 %** block success (1,956 / 2,016). Failures concentrate on Q♠ dump fallbacks (hands 75×2, 242×2, 511×2, 757, 767×2) plus legacy high-risk combinations (153, 432, 461, 498, 681, 912×2).
- Telemetry shows candidate counts dropping slightly (avg 29.0) and best-vs-next gaps widening (~35), indicating the guard is working but producing degenerate alternatives.

## Next Steps
1. Enhance candidate generation to substitute compliant Ten+ heart splits when premium passes are rejected, eliminating Q♠/club fallbacks (hands 75/242/511/767).
2. Add regression fixtures for hands 75/153/242/432/461/498/511/681/757/767/912 to lock in expected behaviour.
3. Emit belief heart-mass diagnostics for moon-triggered passes to validate estimator assumptions before the next tuning cycle.
