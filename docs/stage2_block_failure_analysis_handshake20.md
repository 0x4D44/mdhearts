# Stage 2 Block Failure Analysis — handshake20 (2025-10-18)

Using hard rejections for single-support A♥/K♥ left passes (1,024 hands × 4 permutations) delivers **97.4 %** block success (2,023 / 2,078). Nine inverted-moon failures remain (five unique hands).

| Hand | Perm | Seat | Direction | Moon Prob | Penalty | Passed Cards | Shooter |
| ---: | ---: | --- | --- | ---: | ---: | --- | --- |
| 153 | 3 | south | Left | 0.640 | -18,489.3 | 2♥, K♥, A♥ | north |
| 432 | 2 | east | Left | 0.730 | -24,724.8 | 4♥, J♥, K♥ | south |
| 461 | 2 | north | Left | 0.640 | -14,188.6 | 6♣, Q♠, A♥ | east |
| 498 | 3 | east | Left | 0.640 | -12,602.3 | 4♥, J♥, K♥ | north |
| 681 | 2 | north | Left | 0.730 | -27,822.0 | 5♣, Q♠, A♥ | east |
| 767 | 0 | north | Left | 0.640 | -1,000,000,000.0 | 5♣, 6♣, Q♠ | south |
| 767 | 1 | north | Left | 0.640 | -1,000,000,000.0 | 5♣, 6♣, Q♠ | south |
| 912 | 0 | north | Left | 0.640 | -348.9 | 3♥, 10♥, A♥ | south |
| 912 | 1 | north | Left | 0.640 | -348.9 | 3♥, 10♥, A♥ | south |

## Observations

- **Hard rejections working:** All offending passes now return the fixed `1e9` penalty, steering the optimizer away from single-support A♥/K♥ combinations. The gigantic totals on hands 767 underline that the pass generator still emits fallback combos with no hearts (Q♠ dump) once the premium options are blocked.
- **New failure surfaces:** Hands 153 and 432 show king/ace mixes that exceed the rejection threshold because both lack two extra Ten+ hearts. Additional overrides should prevent these candidates from even being considered.
- **Residual Ace leakage:** Hand 912 still ships A♥ + 10♥ + 3♥ (only one additional Ten+), confirming we must prune the candidate directly instead of relying on large penalties.

## Next Adjustments

1. Instead of returning a large penalty, filter the candidate during enumeration so these combinations never reach scoring (avoid telemetry blow-ups and Q♠-only fallbacks).
2. Extend the pass generator to substitute compliant heart combos (e.g., replace Q♠ dumps with Ten+ hearts) when the hard guard discards the original candidates.
3. Add regression fixtures for hands 153/432/461/681/767/912 so future changes validate the intended behaviour.
