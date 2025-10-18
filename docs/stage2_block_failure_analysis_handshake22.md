# Stage 2 Block Failure Analysis — handshake22 (2025-10-18)

Filtering unsupported premiums during candidate generation (1,024 hands × 4 permutations) yields **97.0 %** block success (1,956 / 2,016). Failures now cluster around Q♠/club dumps and legacy acceptance outliers.

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

## Observations

- **No invalid combos scored:** All illegal single-support passes are removed before evaluation, so penalties remain finite. Zero totals indicate pure off-suit dumps (e.g., Q♠ + low clubs) that still lose despite the guard.
- **New targets:** We now need to prioritise heart-sharing alternatives in the candidate pool so hands like 75/242/511 replace Q♠ dumps with Ten+ hearts.
- **Legacy high penalties remain:** Hands 153/432/498/681/912 still expose the underlying lack of safe heart distribution; future work should inject deterministic “keep A♥/K♥” fallbacks or alternative low-heart splits.

## Next Adjustments

1. Augment candidate generation to insert substitute heart combinations when the guard removes the original pass, avoiding Q♠-only fallbacks.
2. Add regression fixtures for the listed hands to ensure we spot regressions instantly.
3. Capture belief telemetry (premium mass per seat) so we can verify estimator assumptions before re-tuning the fallback logic.
