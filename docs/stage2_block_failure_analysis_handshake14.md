# Stage 2 Block Failure Analysis — handshake14 (2025-10-18)

The latest `stage2_pass_moon` run with the king/ace retention tweaks (1,024 hands × 4 permutations) produced **97.5 %** block success (2,118 / 2,172). Six inverted-moon failures remained (four unique hands).

| Hand | Perm | Seat | Direction | Moon Prob | Penalty | Passed Cards | Shooter |
| ---: | ---: | --- | --- | ---: | ---: | --- | --- |
| 208 | 0 | north | Left | 0.616 | -245.4 | 2♥, 10♥, K♥ | south |
| 208 | 1 | north | Left | 0.616 | -245.4 | 2♥, 10♥, K♥ | south |
| 216 | 2 | west | Left | 0.693 | -450.5 | 7♥, 10♥, K♥ | west |
| 498 | 3 | east | Left | 0.640 | -169.7 | 4♥, J♥, K♥ | north |
| 582 | 3 | east | Left | 0.654 | -296.2 | J♥, K♥, 10♠ | north |
| 941 | 3 | east | Left | 0.763 | -347.8 | 2♥, 10♥, K♥ | north |

## Observations

- **King retention:** Four of the six failures still pass K♥ alongside a Ten (hands 208, 216, 941). The new penalties increase the negative totals (−245 to −451), but the optimizer prefers these combinations over keeping the king, indicating a need for a hard override when only a single premium heart departs.
- **Off-suit filler:** Hand 582 continues to pair K♥ with an off-suit Ten♠; despite the harsher guard, the lack of extra medium hearts keeps this combo at the top.
- **High-urgency case:** Hand 941 reintroduces a ≥0.75 failure (0.763) with K♥+10♥+2♥, highlighting that the urgency scaling alone isn’t sufficient once the left seat is already projected to hold multiple premiums.
- **Probability bins:** No failures appear below 0.6—penalties successfully suppress lower-confidence passes.

## Next Adjustments

1. **Hard keep rule for single-premium passes:** When a left pass contains exactly one premium heart (K♥) plus at most one Ten+ support card and belief exceeds ~0.6, bias the optimizer toward keeping the king (or force evaluation of alternatives without it).
2. **Off-suit escalation:** Multiply the off-suit penalty when moon probability ≥0.65 and at least two low hearts remain, so hands like 582 defer to heart-heavy mixes even if they appear costly.
3. **Regression fixtures:** Capture scenario records for 216 and 941 (in addition to 208/582) to validate future overrides.
4. **Telemetry enrichment:** Emit belief heart-mass deltas into the pass decision logs to confirm whether the estimator is overpredicting the left neighbour’s premium count.
