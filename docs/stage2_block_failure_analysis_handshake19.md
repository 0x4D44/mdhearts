# Stage 2 Block Failure Analysis — handshake19 (2025-10-18)

Run `stage2_pass_moon` (1,024 hands × 4 permutations) under the new hard penalties produced **97.3 %** block success (2,050 / 2,106). Eight inverted-moon failures remain (four unique hands).

| Hand | Perm | Seat | Direction | Moon Prob | Penalty | Passed Cards | Shooter |
| ---: | ---: | --- | --- | ---: | ---: | --- | --- |
| 208 | 0 | north | Left | 0.616 | -318.6 | 2♥, 10♥, A♥ | south |
| 208 | 1 | north | Left | 0.616 | -318.6 | 2♥, 10♥, A♥ | south |
| 498 | 3 | east | Left | 0.640 | -1180.5 | 4♥, J♥, K♥ | north |
| 582 | 3 | east | Left | 0.654 | -384.7 | J♥, A♥, 10♠ | north |
| 767 | 0 | north | Left | 0.640 | -7187.5 | 5♣, 6♣, Q♠ | south |
| 767 | 1 | north | Left | 0.640 | -7187.5 | 5♣, 6♣, Q♠ | south |
| 912 | 0 | north | Left | 0.640 | -309.6 | 3♥, 10♥, A♥ | south |
| 912 | 1 | north | Left | 0.640 | -309.6 | 3♥, 10♥, A♥ | south |

## Observations

- **A♥ + single support:** Hands 208 and 912 still ship the Ace with only one Ten+ heart. Penalty magnitudes tripled, yet the optimizer lacks safer alternatives; we now need a hard rule that simply forbids A♥ from leaving unless two additional Ten+ hearts join it.
- **K♥ + minimal backup:** Hand 498 still selects K♥ + J♥ + low despite an enormous penalty (−1,180). Introduce a keep-K override unless the pass contains at least two Ten+ hearts.
- **Fallback to non-heart dumps:** Hand 767 now chooses a pure non-heart dump (clubs + Q♠) with a gigantic penalty because the new rules suppress the prior heart options. We should surface a dedicated regression for this scenario and evaluate whether retaining Q♠ is preferable when penalties exceed a threshold.

## Next Adjustments

1. Convert the ace/king penalties into hard prohibits—skip candidate generation (or assign `f32::INFINITY`) when A♥ or K♥ fail the new support requirements.
2. Capture regression fixtures for hands 208/498/767/912 to assert the updated behavior.
3. Investigate alternative candidate generation for the non-heart dump on hand 767, potentially preferring to keep Q♠ rather than take a 7,000-point liability.
