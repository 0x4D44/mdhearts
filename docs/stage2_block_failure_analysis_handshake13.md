# Stage 2 Block Failure Analysis — handshake13 (2025-10-18)

Running `stage2_pass_moon` after the king/ace support penalties landed (1,024 hands × 4 permutations) produced **97.5 %** success (2,109 / 2,162) for block-shooter passes. The failure list shrank to **5 inverted moons** (across 4 unique hands).

| Hand | Perm | Seat | Direction | Moon Prob | Penalty | Passed Cards | Shooter |
| ---: | ---: | --- | --- | ---: | ---: | --- | --- |
| 32 | 2 | west | Left | 0.680 | -456.6 | 4♥, K♥, 10♣ | east |
| 208 | 0 | north | Left | 0.616 | -115.8 | 2♥, 10♥, K♥ | south |
| 208 | 1 | north | Left | 0.616 | -115.8 | 2♥, 10♥, K♥ | south |
| 498 | 3 | east | Left | 0.640 | -50.0 | 4♥, J♥, K♥ | north |
| 582 | 3 | east | Left | 0.654 | -197.9 | J♥, K♥, 10♠ | north |

## Observations

- **Penalty magnitudes:** The new heuristics drive much steeper penalties (−456 on hand 32), but the optimizer still prefers these hands because alternative triples either keep the K♥ entirely or expose Q♠/premium spades with higher expected loss.
- **Residual patterns:** Every failure still ships the K♥ (and sometimes J♥) while belief marks the left neighbour as the moon favourite. The penalties now ensure one supporting Ten+ heart moves (hands 208 & 498), yet belief indicates the left neighbour already holds Ten+ hearts, so dumping K♥ may still be suboptimal.
- **Off-suit filler edge case:** Hand 582 continues to pair K♥ with an off-suit Ten♠. The new “fewer-than-two hearts” guard generates a large penalty, but there are no dominated alternatives because the hand lacks extra hearts to share.
- **Moon probability bins:** All failures live in the 0.60–0.68 bucket; no ≥0.70 cases remain, confirming the urgency scaling works.

## Next Adjustments

1. **Left-K retention rule:** When the pass already moves a supporting Ten (e.g., 10♥) and the belief estimate still peaks above 0.6, bias toward holding K♥ unless passing two Ten+ hearts. That would redirect hands 208/498 toward lower-risk mixes (e.g., Ten + low + off-suit).
2. **Off-suit mix guard:** Strengthen the “K♥ + off-suit filler” penalty to prefer low-heart splits when the hand has ≥2 low hearts remaining (hand 32 and 582).
3. **Failure fixtures:** Add regression cases mirroring hands 32, 498, 582 so future tweaks confirm when these overrides apply.
4. **Belief debug:** Log the left-seat heart mass for failures to confirm whether the estimator overstates their high-heart count; consider normalising probabilities within the penalty itself.
