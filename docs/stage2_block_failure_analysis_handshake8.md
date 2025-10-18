# Stage 2 Block Failure Analysis — handshake8 (2025-10-18)

The run `stage2_pass_moon_v045_handshake8` (1,024 hands × 4 permutations) incorporates the latest direction-aware moon-liability scaling. Block-shooter success held at **97.66 %** (2,003 / 2,051). The ≥ 0.60 miss count remains at **12 inverted moons**, identical to handshake7, indicating the new multipliers are still too conservative.

| Hand | Perm | Seat | Direction | Moon Prob | Penalty | Passed Cards | Shooter |
| ---: | ---: | --- | --- | ---: | ---: | --- | --- |
| 153 | 3 | south | Left | 0.640 | 0.00 | 2♥, K♥, A♥ | north |
| 223 | 0 | south | Left | 0.693 | 0.00 | Q♥, K♥, A♥ | west |
| 223 | 1 | south | Left | 0.693 | 0.00 | Q♥, K♥, A♥ | west |
| 432 | 2 | east | Left | 0.730 | 0.00 | Q♥, K♥, A♥ | south |
| 498 | 3 | east | Left | 0.640 | 0.00 | 4♥, Q♥, K♥ | north |
| 499 | 2 | west | Left | 0.730 | 5.70 | Q♥, K♥, A♥ | north |
| 615 | 3 | north | Left | 0.631 | 0.00 | J♥, Q♥, A♥ | east |
| 681 | 2 | north | Left | 0.730 | 0.00 | 3♥, Q♥, K♥ | east |
| 767 | 0 | north | Left | 0.640 | 0.00 | 3♥, 4♥, A♥ | south |
| 767 | 1 | north | Left | 0.640 | 0.00 | 3♥, 4♥, A♥ | south |
| 912 | 0 | north | Left | 0.640 | 0.00 | 10♥, J♥, A♥ | south |
| 912 | 1 | north | Left | 0.640 | 0.00 | 10♥, J♥, A♥ | south |

_Penalty_ values reflect the `moon_liability_penalty` for the selected combo; the single non-zero entry (West hand 499) shows the new scaling, but it remains too small to change the decision ordering.

## Patterns

- **Seat distribution:** North 6, South 3, East 2, West 1. South joins North as a primary source of high-risk misses, all on Left passes.
- **Zero-penalty clusters:** South-left hands (153, 223) and East-left hands (432, 498) still select zero-penalty trios despite holding multiple premium hearts. The directional multiplier (×1.25 / ×1.35) is insufficient versus the base scoring delta.
- **Ace-only handoffs remain unchecked:** North hand 767 and South hand 153 pass A♥ with low hearts; no penalty triggers although several Ten+ hearts remain.
- **Triple premium passes untouched:** South 223 and East 432 pass Q♥/K♥/A♥ yet keep the shooter engaged. We likely need heuristics that split premium hearts across opponents when the shooter is on the left.

## Next actions

1. Increase the Left-pass directional factor (or add seat-specific weights) so North/South/East left passes get stronger penalties when fewer than two premium hearts are sent.
2. Add explicit penalties when only Ace♥ is shipped and multiple Ten+ hearts remain.
3. Encode regression fixtures for hands 223 and 432 so penalty regressions are caught automatically.
