# Stage 2 Block Failure Analysis — handshake15 (2025-10-18)

Running `stage2_pass_moon` with the latest king/ace overrides (1,024 hands × 4 permutations) produced **97.5 %** success (2,081 / 2,135). Six inverted-moon failures remain (four unique hands).

| Hand | Perm | Seat | Direction | Moon Prob | Penalty | Passed Cards | Shooter |
| ---: | ---: | --- | --- | ---: | ---: | --- | --- |
| 459 | 2 | north | Left | 0.640 | -291.0 | A♥, 8♠, 5♦ | north |
| 498 | 3 | east | Left | 0.640 | -398.8 | 4♥, J♥, K♥ | north |
| 582 | 3 | east | Left | 0.654 | -354.6 | A♥, 10♠, 2♦ | north |
| 610 | 0 | north | Left | 0.640 | -299.0 | A♥, 10♠, 2♣ | south |
| 610 | 1 | north | Left | 0.640 | -299.0 | A♥, 10♠, 2♣ | south |
| 941 | 3 | east | Left | 0.763 | -541.3 | 2♥, 10♥, K♥ | north |

## Observations

- **Ace leakage:** Three of the failures ship A♥ with off-suit or low support (hands 459, 582, 610). Despite the stronger penalties, the optimizer still chooses these combos because alternatives expose Q♠ or keep large heart clusters that the estimator distrusts.
- **Remaining K♥ case:** Hand 941 remains the only king-centric miss; keeping K♥ raises the total above several alternative liabilities, signalling the need for a hard override when only one premium heart leaves and belief on the left is still high.
- **Belief dependency:** All cases involve left-seat moon probabilities ≥ 0.64 (hand 941 at 0.763). Instrumenting belief heart-mass diagnostics would clarify whether the estimator is overweighting the left neighbour’s premium holdings.

## Next Adjustments

1. **Ace retention rule:** Force A♥ to stay unless two Ten+ hearts accompany it or the pass sheds another premium; this directly targets hands 459/582/610.
2. **K♥ override:** Require at least two Ten+ hearts when K♥ departs during high-belief passes (hand 941), otherwise keep the king.
3. **Fixture coverage:** Add regression fixtures mirroring all six failures so the next round of tuning demonstrates the intended overrides.
4. **Telemetry enrichment:** Emit per-seat heart mass and premium probability snapshots for high-urgency passes to validate estimator assumptions before iterating again.
