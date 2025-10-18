# Stage 2 Block Failure Analysis — handshake23 (2025-10-18)

After introducing premium-support substitution in candidate generation (1,024 hands × 4 permutations), overall block success is **97.0 %** (2,042 / 2,105). The remaining ≥ 0.60 failures are identical to handshake22, indicating further work must tackle the Q♠ dump fallbacks and legacy premium splits.

| Hand | Perm | Seat | Direction | Moon Prob | Penalty | Passed Cards | Shooter |
| ---: | ---: | --- | --- | ---: | ---: | --- | --- |
| 75 | 0 | west | Left | 0.640 | 0.0 | 6♣, 7♣, Q♠ | north |
| 75 | 1 | west | Left | 0.640 | 0.0 | 6♣, 7♣, Q♠ | north |
| 153 | 3 | south | Left | 0.640 | -18,489.3 | 2♥, K♥, A♥ | north |
| 242 | 0 | north | Left | 0.640 | 0.0 | 7♣, 8♣, Q♠ | east |
| 242 | 1 | north | Left | 0.640 | 0.0 | 7♣, 8♣, Q♠ | east |
| 432 | 2 | east | Left | 0.730 | -24,724.8 | 4♥, J♥, K♥ | south |
| 461 | 2 | north | Left | 0.640 | -14,188.6 | 6♣, Q♠, A♥ | east |
| 498 | 3 | east | Left | 0.640 | -12,602.3 | 4♥, J♥, K♥ | north |
| 511 | 0 | south | Left | 0.640 | 0.0 | 4♣, 5♣, Q♠ | west |
| 511 | 1 | south | Left | 0.640 | 0.0 | 4♣, 5♣, Q♠ | west |
| 681 | 2 | north | Left | 0.730 | -27,822.0 | 5♣, Q♠, A♥ | east |
| 757 | 3 | north | Left | 0.680 | 0.0 | 6♣, 7♣, Q♠ | east |
| 767 | 0 | north | Left | 0.640 | 0.0 | 5♣, 6♣, Q♠ | south |
| 767 | 1 | north | Left | 0.640 | 0.0 | 5♣, 6♣, Q♠ | south |
| 912 | 0 | north | Left | 0.640 | -348.9 | 3♥, 10♥, A♥ | south |
| 912 | 1 | north | Left | 0.640 | -348.9 | 3♥, 10♥, A♥ | south |

## Notes
- Illegal premium dumps are gone, but zero-penalty Q♠ fallbacks persist (hands 75/242/511/757/767). The substitution logic must promote alternative Ten+ heart splits instead of retaining pure off-suit dumps.
- Premium mixes with significant negative totals (hands 153/432/498/681/912) still require deterministic “keep A♥/K♥” overrides or tuned weights once compliant substitutes exist.

## Action Items
1. Enhance `enforce_premium_support_rule` to synthesise compliant heart combinations (e.g., replace Q♠ dumps with Ten+ hearts) rather than returning the original zero-penalty triple.
2. Add regression fixtures for the hand list above (see `docs/stage2_regression_plan.md`).
3. Re-run the benchmark after each fixture and substitution improvement to validate progress.
