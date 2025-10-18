# Stage 2 Progress Report — 2025-10-17 (Evening)

## Progress Summary
- **Baseline & selective benchmarks:** Stage 2 pass_v2 logic with the 0.45 moon threshold (`bench/out/stage2_pass_moon_v045_selective/`) delivers an 18.7 % block-pass rate with 97.1 % success while keeping competitive-seat PPH gains (+0.03 to +1.21) and suppressing the Easy bot (−1.54 PPH vs. legacy).
- **Telemetry & tooling:** Structured pass/play telemetry, summaries (`telemetry_summary.{json,md,csv}`), and deck-ready tables/digests (`docs/stage2_metrics_digest.md`, `docs/benchmarks/stage2_pass_block_ratios.md`, `docs/benchmarks/stage2_pass_comparison.md`) are up to date for the selective run.
- **Block-shooter analytics:** `tools/analyze_block_shooter.py` + `tools/list_block_failures.py` expose all ≥0.60 probability failures; `docs/benchmarks/stage2_block_failures.md` shows 20 inverted moons concentrated on West/North passes (shooters: West 8, South 5, North 5, East 2).
- **Directional prototype experiment:** A targeted liability boost for West/North + left/across passes (`bench/out/stage2_pass_moon_v045_directional/`) maintained block success (~97.1 %) but did not reduce failure count; doc archived at `docs/benchmarks/stage2_pass_moon_v045_directional.md`.
- **Documentation sync:** Stage 2 deck, summaries, analysis plan, and status updates reference the selective baseline, failure log, and ongoing tuning focus.
- **West/North handshake guard:** pass_v2 now subtracts a moon-liability penalty when block passes retain Q♠, ≥ 2 high hearts, or multiple mid hearts (Ten+); the optimizer pool always evaluates at least one high-heart trio; regression tests cover the handshake behaviour (`handshake_penalty_applies_when_high_hearts_are_kept`, `handshake_prefers_passing_high_heart_combo`).
- **Handshake benchmark:** Latest run `stage2_pass_moon_v045_handshake11` (1,024 hands × 4 perms) lands 2,233 block passes at 98.1 % success (2,191 hits); only six ≥ 0.60 inverted-moon failures remain (`docs/benchmarks/stage2_block_failures_handshake11.md`, analysis in `docs/stage2_block_failure_analysis_handshake11.md`).

## Open Issues / Risks
- **Inverted moon failures:** Remaining high-risk misses are still inverted moons (6 cases ≥ 0.60) concentrated on North (4) and South (1) Left passes; penalties now heavily disfavour these trios but no better safe combo exists, so targeted overrides remain necessary.
- **Lack of automation:** Telemetry CSV/JSON outputs aren’t yet wired into notebooks/dashboards, so manual regeneration is required for each variant.
- **Regression coverage gaps:** No automated tests yet for optimizer bounds or feature-flag fallbacks.
- **Windows validation pending:** Full workspace tests (`cargo test --workspace`) have not been rerun on the Windows host since the selective tuning.

## Next Steps
1. **Targeted heuristic iteration:** Implement and benchmark refined rules (e.g., explicit high-heart handoffs, partner coordination checks) aimed at the remaining inverted-moon failures in `stage2_block_failures_handshake11.md` and the scenarios outlined in `docs/stage2_block_failure_analysis_handshake11.md`.
2. **Analytics automation:** Ingest `telemetry_summary.csv`, block summaries, and failure logs into notebooks/dashboards to streamline future deck updates.
3. **Regression testing:** Add unit/regression coverage for pass optimizer bounds and feature-flag fallbacks (`MDH_PASS_V2`, `MDH_ENABLE_BELIEF`).
4. **Windows acceptance run:** Once heuristics stabilise, run `cargo test --workspace` (including `hearts-app`) on the Windows host and capture results.
