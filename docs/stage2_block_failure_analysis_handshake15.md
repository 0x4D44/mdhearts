# Stage 2 Block Failure Analysis — handshake15 (2025-10-18)

Run `stage2_pass_moon_v045_handshake15` (1,024 hands × 4 permutations) reflects the queen-heart guard and low-heart substitution updates. Block-shooter passes succeeded **97.22 %** of the time (2,065 / 2,124). The ≥ 0.60 failure list has expanded slightly (12 inverted moons) as the optimizer now prefers all-heart trios, exposing remaining moon-liability tuning gaps rather than off-suit fallbacks.

| Hand | Perm | Seat | Direction | Moon Prob | Penalty | Passed Cards | Shooter |
| ---: | ---: | --- | --- | ---: | ---: | --- | --- |
| 153 | 3 | south | Left | 0.640 | -18 489.3 | 10♥, K♥, A♥ | north |
| 432 | 2 | east | Left | 0.730 | -24 724.8 | Q♥, K♥, A♥ | south |
| 461 | 2 | north | Left | 0.640 | -14 188.6 | 10♥, Q♥, K♥ | east |
| 498 | 3 | east | Left | 0.640 | -12 602.3 | J♥, Q♥, K♥ | north |
| 567 | 2 | west | Left | 0.794 | -24 751.9 | Q♥, K♥, A♥ | north |
| 681 | 2 | north | Left | 0.730 | -27 822.0 | Q♥, K♥, A♥ | east |
| 757 | 3 | north | Left | 0.680 | 0.0 | A♥, K♥, A♣ | east |
| 890 | 0 | west | Left | 0.640 | -224.2 | J♠, 7♦, 6♣ | south |
| 890 | 1 | west | Left | 0.640 | -224.2 | J♠, 7♦, 6♣ | south |
| 890 | 2 | west | Left | 0.640 | -224.2 | J♠, 7♦, 6♣ | north |
| 912 | 0 | north | Left | 0.640 | -348.9 | 10♥, J♥, A♥ | south |
| 912 | 1 | north | Left | 0.640 | -348.9 | 10♥, J♥, A♥ | south |

## Observations

- **Premium guard holding.** All failures except the 890 cluster now pass three hearts; queen/ace dumps without Ten+ support are gone. Hand 757 still mixes A♣ with A♥/K♥ because the substitution logic cannot introduce a third Ten+ heart when only two exist.
- **New exposure (Hand 890).** West occasionally sends a spade/diamond/club trio with minimal penalty. The guard does not intervene because no premium hearts are included, suggesting we need a supplementary rule for high-urgency off-suit passes when strong hearts remain.
- **High-penalty cases unchanged.** Hands 153/432/461/498/567/681/912 continue to incur massive moon penalties; further tuning should investigate whether any of these still need deterministic overrides versus additional weighting.

## Next actions

1. Add regression fixtures for hands 567, 757, and 890 to cement the current behaviour and target future tuning.
2. Extend `moon_liability_penalty` to punish off-suit dumps (hand 890) when Ten+ hearts remain unpassed.
3. Experiment with explicit overrides for the final premium clusters (153/432/461/498/681/912), potentially splitting high hearts across opponents once urgency crosses 0.70.
