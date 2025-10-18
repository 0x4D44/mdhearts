# Stage 2 Regression Plan — Remaining Hands

## Target Hands (telemetry handshake22)
- 75×2 — West Q♠ dump (6♣, 7♣, Q♠)
- 153 — South 2♥/K♥/A♥
- 242×2 — North Q♠ dump (7♣, 8♣, Q♠)
- 432 — East 4♥/J♥/K♥
- 461 — North 6♣/Q♠/A♥
- 498 — East 4♥/J♥/K♥
- 511×2 — South Q♠ dump (4♣, 5♣, Q♠)
- 681 — North 5♣/Q♠/A♥
- 757 — North Q♠ dump (6♣, 7♣, Q♠)
- 767×2 — North Q♠ dump (5♣, 6♣, Q♠)
- 912×2 — North 3♥/10♥/A♥

## Fixture Strategy (each hand)
- Create a dedicated fixture under `tests/pass_hard_guard.rs` with minimal card sets reproducing the telemetry combination.
- Ensure `enumerate_pass_triples` returns compliant heart splits (or retains the premium heart) and does **not** contain the original failing combo.
- Reuse a helper to build `PassScoreInput` for each fixture to keep tests concise.

## Next Steps
1. Create test fixtures under `crates/hearts-core/tests/pass_hard_guard.rs` covering the above combos; assert compliant alternatives are preferred and illegal passes are absent.
2. Update docs after tests pass to record the new regression coverage.
3. Re-run the Stage 2 benchmark (handshake23) once fixtures and filtering are in place.
