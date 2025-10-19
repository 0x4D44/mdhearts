# Stage 2 Block Failure Analysis — handshake12 (2025-10-18)

Run `stage2_pass_moon_v045_handshake12` (1,024 hands × 4 permutations) applies the new hard guard that forces three Ten+ hearts whenever a left pass ships a premium heart or Q♠ under high moon urgency. Block-shooter passes succeeded **97.03 %** of the time (2,061 / 2,124). The ≥ 0.60 failure list remains at **16 inverted moons**, unchanged from the earlier handshake23 sample, but the affected combinations now accrue much larger negative totals when the guard fires.

| Hand | Perm | Seat | Direction | Moon Prob | Penalty | Passed Cards | Shooter |
| ---: | ---: | --- | --- | ---: | ---: | --- | --- |
| 75 | 0 | west | Left | 0.640 | 0.0 | K♠, A♥, Q♥ | north |
| 75 | 1 | west | Left | 0.640 | 0.0 | K♠, A♥, Q♥ | north |
| 153 | 3 | south | Left | 0.640 | -18 489.3 | 2♥, K♥, A♥ | north |
| 242 | 0 | north | Left | 0.640 | 0.0 | K♠, A♥, Q♥ | east |
| 242 | 1 | north | Left | 0.640 | 0.0 | K♠, A♥, Q♥ | east |
| 432 | 2 | east | Left | 0.730 | -24 724.8 | 4♥, J♥, K♥ | south |
| 461 | 2 | north | Left | 0.640 | -14 188.6 | 6♣, Q♠, A♥ | east |
| 498 | 3 | east | Left | 0.640 | -12 602.3 | 4♥, Q♥, K♥ | north |
| 511 | 0 | south | Left | 0.640 | 0.0 | K♠, K♥, Q♥ | west |
| 511 | 1 | south | Left | 0.640 | 0.0 | K♠, K♥, Q♥ | west |
| 681 | 2 | north | Left | 0.730 | -27 822.0 | 5♣, Q♠, A♥ | east |
| 757 | 3 | north | Left | 0.680 | 0.0 | A♥, K♥, A♣ | east |
| 767 | 0 | north | Left | 0.640 | 0.0 | A♥, K♥, 9♥ | south |
| 767 | 1 | north | Left | 0.640 | 0.0 | A♥, K♥, 9♥ | south |
| 912 | 0 | north | Left | 0.640 | -348.9 | 10♥, J♥, A♥ | south |
| 912 | 1 | north | Left | 0.640 | -348.9 | 10♥, J♥, A♥ | south |

## Key takeaways

- **Guard impact is visible.** Hands that previously slid through with negligible penalties (153, 432, 461, 498, 681) now receive five-digit liability deductions, confirming the hard rejection path is firing when sufficient Ten+ replacements exist.
- **Fallback splits persist.** Zero-penalty misses (75, 242, 511, 757, 767) still ship premium hearts with off-suit anchors (K♠ or A♣) because the guard never triggers—either belief urgency falls just under the threshold or the hand lacks a third Ten+ heart. These remain prime candidates for explicit substitution or tighter urgency gating.
- **Failure mix unchanged.** All 16 misses are inverted moons, matching handshake23. Shooter seats continue to cluster on North/East, so upcoming tuning should focus on high-heart redistribution toward those neighbours.

## Follow-up items

1. Extend `PremiumSupportResolution::Replacements` to synthesise Ten+ heart splits even when only two suitable cards exist, so the optimizer can still prefer partial heart support over Q♠ dumps.
2. Evaluate whether the hard guard thresholds should depend on moon probability rather than the coarse urgency cut (currently ≥ 0.60) to catch hand 75–style passes earlier.
3. Convert the new regression fixtures (hands 75/153/242/432/461/498/511/681/757/767/912) into targeted scenario tests that guard against future substitutions reintroducing the zero-penalty cases.
