# Stage 2 Block Failure Analysis — handshake16 (2025-10-18)

The latest `stage2_pass_moon` run after the ace retention updates (1,024 hands × 4 permutations) produced **97.5 %** block success (1,985 / 2,036). Only four inverted-moon failures remain (two unique hands).

| Hand | Perm | Seat | Direction | Moon Prob | Penalty | Passed Cards | Shooter |
| ---: | ---: | --- | --- | ---: | ---: | --- | --- |
| 208 | 0 | north | Left | 0.616 | -318.6 | 2♥, 10♥, A♥ | south |
| 208 | 1 | north | Left | 0.616 | -318.6 | 2♥, 10♥, A♥ | south |
| 498 | 3 | east | Left | 0.640 | -398.8 | 4♥, J♥, K♥ | north |
| 582 | 3 | east | Left | 0.654 | -384.7 | J♥, A♥, 10♠ | north |

## Observations

- **Ace-focused leaks:** Hand 582 still ships A♥ with an off-suit Ten♠ despite the new guard. The optimizer favours this mix because alternative triples expose Q♠ or keep multiple premium hearts; a hard “keep A♥ unless two Ten+ hearts depart” override will remove it.
- **Two-heart passes:** Hands 208 double with {A♥,10♥} plus a small heart. The belief model rates the left neighbour highly, yet keeping A♥ or K♥ forces the optimizer toward even larger liabilities. A deterministic rule to require three Ten+ hearts (or keep A♥ otherwise) would address both permutations.
- **K♥ case:** Hand 498 still passes K♥ with only J♥ support. Penalty totals are enormous (−399), but candidate scarcity keeps this option preferred—suggesting we should force the pass to grab a second Ten+ heart or retain the king.

## Next Adjustments

1. **Ace retention override:** Keep A♥ unless the pass includes at least two Ten+ hearts (or another premium) so hands 208/582 fall back to safer combinations.
2. **King support requirement:** Require K♥ passes to include two Ten+ hearts or keep the king (hand 498) when belief on the left is high.
3. **Fixture expansion:** Add regression fixtures for hands 208 and 582 to verify these overrides, alongside the existing king tests.
4. **Telemetry diagnostic:** Emit belief heart-mass snapshots for moon-triggered passes to confirm estimator accuracy before further tuning.
