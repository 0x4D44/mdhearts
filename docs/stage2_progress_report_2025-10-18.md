# Stage 2 Progress Report — 2025-10-18

## What changed today
- **Premium guard tightened.** `enumerate_pass_triples` now replaces any high-urgency left pass that includes A♥/K♥ or Q♠ with Ten+ heart alternatives; illegal combinations are discarded if no compliant substitute exists. The helper `choose_index_combinations` enumerates all viable replacements so the optimizer evaluates every legal trio.
- **Regression fixtures landed.** Added unit fixtures for the 11 high-risk hands (75/153/242/432/461/498/511/681/757/767/912) inside `crates/hearts-core/tests/pass_hard_guard.rs`, ensuring the guard continues to block the legacy Q♠/club dumps and Ace-only passes.
- **Benchmark refreshed.** Ran `stage2_pass_moon_v045_handshake17` with belief + telemetry enabled. Block-shooter success is **97.24 %** (1,938 / 1,993). Only nine inverted moons remain: six premium clusters, one heart+club anchor, and two penalised off-suit dumps.
- **Failure tooling upgrade.** `tools/list_block_failures.py` now records the exact passed trio for each failure, letting us inspect the stubborn off-suit anchors (K♠ / A♣) that survive the current guard.
- **Documentation/artefacts updated.** Published `docs/stage2_block_failure_analysis_handshake17.md`, regenerated the failure tables, and refreshed the aggregate CSV/JSON summaries for Stage 2 pass runs.

## Current pain points
- Residual failures now centre on premium-heart clusters (153/432/461/498/567/681/912) plus the heart+club anchor and the off-suit dump (890). Penalties are huge, but better alternatives still rank lower.
- Belief urgency for these hands may need retuning to push the guard into action sooner on left passes.

## Next steps
1. Prototype targeted overrides for hands 153/432/461/498/567/681/757/890/912 so premium hearts are split more reliably and off-suit dumps disappear.
2. Capture belief telemetry for these hands to confirm the defensive urgency thresholds before further tuning.
3. Rerun Windows acceptance (`cargo test --workspace`) and prep the pass_v2 vs pass_v1 benchmark comparison once overrides land.
