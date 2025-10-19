# Stage 2 Block Failure Analysis — handshake17 (2025-10-18)

Run `stage2_pass_moon_v045_handshake17` (1,024 hands × 4 permutations) applies the queen-heart guard, low-heart substitution, and new off-suit penalties. Block-shooter passes succeeded **97.24 %** of the time (1,938 / 1,993). The ≥ 0.60 failure list now sits at **9 inverted moons**: six premium-heart clusters, one heart+club anchor, and two off-suit dumps that still outrank guarded alternatives.

| Hand | Perm | Seat | Direction | Moon Prob | Penalty | Passed Cards | Shooter |
| ---: | ---: | --- | --- | ---: | ---: | --- | --- |
| 153 | 3 | south | Left | 0.640 | -18 489.3 | 10♥, K♥, A♥ | north |
| 432 | 2 | east | Left | 0.730 | -24 724.8 | Q♥, K♥, A♥ | south |
| 461 | 2 | north | Left | 0.640 | -14 188.6 | 10♥, Q♥, K♥ | east |
| 498 | 3 | east | Left | 0.640 | -12 602.3 | J♥, Q♥, K♥ | north |
| 567 | 2 | west | Left | 0.794 | -24 751.9 | Q♥, K♥, A♥ | north |
| 681 | 2 | north | Left | 0.730 | -27 822.0 | Q♥, K♥, A♥ | east |
| 757 | 3 | north | Left | 0.680 | 0.0 | A♥, K♥, A♣ | east |
| 890 | 0 | west | Left | 0.640 | -10 678.4 | J♠, 7♦, 6♣ | south |
| 890 | 1 | west | Left | 0.640 | -10 678.4 | J♠, 7♦, 6♣ | south |
| 912 | 0 | north | Left | 0.640 | -348.9 | 10♥, J♥, A♥ | south |
| 912 | 1 | north | Left | 0.640 | -348.9 | 10♥, J♥, A♥ | south |

## Observations

- **Premium guard intact.** Every failure now either ships three premium hearts or (hand 757) pairs A♥/K♥ with A♣ when no third Ten+ heart exists. The penalties keep these totals deeply negative, but alternative combinations remain even worse.
- **Off-suit penalty active.** Hands 890 still surface but carry an additional −10.6k penalty, meaning the optimizer regards them as extremely poor. Further tuning or forced substitution is required to eliminate them entirely.
- **Penalty plateau.** High-penalty clusters (153/432/461/498/567/681/912) persist with five-digit hits, indicating that deterministic overrides or belief-side adjustments may be needed to change their ordering.

## Next actions

1. Prototype targeted overrides for hands 890 and 757 (e.g., force a Ten+ heart when ≥2 remain, or split premium hearts across opponents) to replace the current off-suit anchor.
2. Investigate belief estimates for the premium clusters to ensure defensive urgency reflects the shooter risk; consider pushing the guard threshold above 0.60 for left passes.
3. Update regression coverage once overrides land, then regenerate the benchmark to confirm the inverted-moon count drops below 5.
