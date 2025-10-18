# Stage 2 Block Failure Analysis — handshake10 (2025-10-18)

Benchmark `stage2_pass_moon_v045_handshake10` (1,024 hands × 4 perms) with shooter-aware directional penalties delivered **97.9 %** block success (2,168 / 2,213). Only **6** ≥ 0.60 inverted-moon failures remain — down from 12 in handshake8/9 and 20 in the original selective run.

| Hand | Perm | Seat | Direction | Moon Prob | Penalty | Passed Cards | Shooter |
| ---: | ---: | --- | --- | ---: | ---: | --- | --- |
| 153 | 3 | south | Left | 0.640 | -35.8 | 2♥, K♥, A♥ | north |
| 615 | 3 | north | Left | 0.631 | -38.5 | J♥, Q♥, A♥ | east |
| 767 | 0 | north | Left | 0.640 | -61.8 | 3♥, 4♥, A♥ | south |
| 767 | 1 | north | Left | 0.640 | -61.8 | 3♥, 4♥, A♥ | south |
| 912 | 0 | north | Left | 0.640 | -22.0 | 10♥, J♥, A♥ | south |
| 912 | 1 | north | Left | 0.640 | -22.0 | 10♥, J♥, A♥ | south |

_Penalty_ is the `moon_liability_penalty` for the selected pass. Strongly negative totals reflect that the new penalties substantially reduce the overall score; however, these combinations still surfaced due to relative ordering in the optimizer (compared with even worse alternatives).

## Observations

- **Majority on North:** 4 of 6 misses are North→South Left passes; South has 1, East 0. North’s remaining failures are “Ace + low hearts” handoffs where no premium hearts remain after the pass.
- **Penalty effectiveness:** All failing combos now incur sizeable penalties (|penalty| ≥ 22). We’re no longer seeing zero-penalty misses. Further tuning should compare penalty magnitudes against alternative combo totals to ensure the optimizer actually prefers safer passes.
- **Special cases:** Hand 153 (South) still offloads K♥ + A♥ to the left. Even with the new seat/shooter scaling, the optimizer prefers this trio because alternative cards open voids. We may need a rule that splits premium hearts when the shooter projection on the left is high.

## Follow-up ideas

1. **Forced premium split:** When the Left neighbour’s shooter pressure exceeds a threshold, require at least one premium heart to travel to another seat (or reduce score sharply).
2. **Ace-only guard:** Add a dedicated penalty when the pass includes Ace♥ but no other premium hearts while ≥2 Ten+ hearts remain in hand (hands 767/912).
3. **Optimizer weighting check:** Review `PassWeights` to ensure liability reductions and void bonuses do not overpower the new penalties in these specific configurations.
