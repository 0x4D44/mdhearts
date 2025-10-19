# Stage 2 Progress Summary — 2025-10-19

## Recent Progress
- Hardened the guard substitution pipeline so we always surface a candidate before touching the legacy heuristic. Low-scoring, non-liability trios are tracked separately and only promoted when the main pool is empty (`crates/hearts-core/src/pass/optimizer.rs`).
- Added explicit synthesis paths for unsupported A♥/K♥ hands and premium-heart triples. `force_guarded_pass` now generates safe Ten+ splits and filters out `HARD_REJECTION` combos, while new helpers keep us from dumping all hearts in high-urgency spots (`crates/hearts-core/src/pass/optimizer.rs`).
- Captured deterministic Stage 2 fixtures that assert the forced guard retains stoppers on the critical seeds (75/511/757/767). See `stage2_force_guarded_pass_preserves_stoppers` inside `crates/hearts-core/tests/pass_hard_guard.rs`.
- Ran `stage2_pass_moon_v045_guardfix` with belief + pass telemetry enabled. Block-shooter success improved to **97.33 %** (2,149 / 2,208) and every `pass_v2_fallback` warning disappeared. Outputs live under `bench/out/stage2_pass_moon_v045_guardfix/`, with the failure table in `docs/benchmarks/stage2_block_failures_handshake20.md` and aggregate stats in `docs/benchmarks/stage2_block_shooter_guardfix.{csv,json}`.

## Outstanding Gaps
- Inverted-moon failures are now limited to eight unique hands: 153, 432, 461, 498, 567, 681, 767 (×2 perms), and 912 (×2). All except 767 still pass three premium hearts; the new guard keeps A♥/K♥ home on the previously problematic hands (75/511/890) but we need stronger heuristics to break up premium triples when the belief urgency is ≥ 0.6.
- Hand 767 now sends `{5♥, 4♥, 3♥}` via the synthesized fallback—better than dumping A♥, but still concedes the moon. We need an alternate plan (e.g., hybrid Ten+ mix with off-suit) that pressures the shooter without emptying mid hearts.
- Direction-aware penalties should be reviewed: hands 432/567/681 still score premium triples highly despite the larger liability adjustments. We likely need a deterministic override ahead of scoring for “three premiums” when support exists.

## Next Steps
1. Implement targeted overrides for the remaining failure hands so premium triples are replaced by Ten+/mid heart support (focus on 432/461/498/567/681/912); keep the belief urgency thresholds aligned with the new guard.
2. Explore alternate synth strategies for 767 (e.g., promote Ten♥ + mid hearts even if it keeps a single premium) and add regression coverage once the preferred trio is identified.
3. Re-run the Stage 2 benchmark after the overrides, refresh the telemetry tables, and update the deck/analysis docs with the new moon-blocking metrics.
