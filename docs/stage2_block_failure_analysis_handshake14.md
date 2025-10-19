# Stage 2 Block Failure Analysis — handshake14 (2025-10-18)

Run `stage2_pass_moon_v045_handshake14` (1,024 hands × 4 permutations) introduces the relaxed low-score fallback inside `enumerate_pass_triples`, eliminating the guard-induced legacy fallback. Block-shooter passes succeeded **97.35 %** of the time (1,987 / 2,041). Only **8 inverted moons** remain, and all offending trios now ship three hearts—no K♠/A♣ anchors survive the guard.

| Hand | Perm | Seat | Direction | Moon Prob | Penalty | Passed Cards | Shooter |
| ---: | ---: | --- | --- | ---: | ---: | --- | --- |
| 32 | 2 | west | Left | 0.680 | -124.5 | 4♥, Q♥, 10♣ | east |
| 153 | 3 | south | Left | 0.640 | -18 489.3 | 10♥, A♥, K♥ | north |
| 432 | 2 | east | Left | 0.730 | -24 724.8 | Q♥, A♥, K♥ | south |
| 461 | 2 | north | Left | 0.640 | -14 188.6 | 10♥, Q♥, K♥ | east |
| 498 | 3 | east | Left | 0.640 | -12 602.3 | J♥, Q♥, K♥ | north |
| 681 | 2 | north | Left | 0.730 | -27 822.0 | Q♥, A♥, K♥ | east |
| 767 | 0 | north | Left | 0.640 | -224.4 | 3♥, 4♥, 5♥ | south |
| 767 | 1 | north | Left | 0.640 | -224.4 | 3♥, 4♥, 5♥ | south |

## Observations

- **Guard respected everywhere.** Every remaining failure sends three hearts; premium hearts only travel together when multiple Ten+ cards exist, and the penalty keeps their totals deeply negative.
- **Legacy anchors removed.** Hands 75/242/511/757 are no longer in the failure table—falling back to pass_v1 is no longer necessary, and Q♠-anchored dumps are eliminated.
- **New top miss (Hand 32).** West still ships 4♥/Q♥/10♣ with a mild penalty despite the guard. Belief judged the moon risk moderate (0.68), so follow-up tuning should examine whether this combination deserves additional scaling (e.g., emphasizing premium heart splits even without A/K).

## Next actions

1. Inspect the residual failures (especially Hand 32) to decide whether additional moon-liability weighting is needed for mid-heart-only passes.
2. Expand regression coverage for the new low-penalty case (hand 32) to make sure future scoring changes keep the guard intact.
3. Schedule the Windows acceptance sweep once telemetry/docs land, then prepare A/B benchmarks against pass_v1 for publication.
