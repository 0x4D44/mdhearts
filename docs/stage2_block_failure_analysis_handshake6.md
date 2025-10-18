# Stage 2 Block Failure Analysis — handshake6 (2025-10-18)

Benchmark `stage2_pass_moon_v045_handshake6` (1,024 hands × 4 perms) recorded **12** block-shooter misses with estimated probability ≥ 0.60. The success rate stays high at 2024 / 2072 (97.68 %), but the failure mix shifted after the latest mid/high-heart penalty tuning.

## Snapshot of ≥ 0.60 failures

| Hand | Perm | Seat | Direction | Moon Prob | Penalty | Passed Cards | Moon Shooter |
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

> _Penalty_ is the `moon_liability_penalty` on the chosen candidate. Non-zero entries indicate the new heuristics fired but the combo still ranked highest.

## Observations

- **Penalty fire rate improved but still limited.** West hand 499 now shows a non-zero penalty, but most failures still pick zero-penalty trios. Hands 223/432/615/912 demonstrate that single-premium or triple-premium passes can remain penalty-free when the optimizer’s base scoring dominates.
- **Seat distribution broadened.** North still accounts for half the failures (6 / 12), but South now contributes 3, and East 2. West has a single failure with a small (≈5.7) penalty.
- **Triples of premium hearts still rank first.** Several failures (223, 432, 499) pass Q♥/K♥/A♥ yet retain strong moon odds. Our penalties do not differentiate between sending all premiums together versus splitting them—additional heuristics may need to encourage splitting when the shooter is likely to the left.
- **Low-heart‐plus‑Ace combos persist.** Hands 153, 767, 912 show that dumping A♥ with low hearts still yields zero penalty, even when higher hearts remain in hand. The mid-heart logic should probably consider Ace-only handoffs as insufficient when combined with multiple Ten+ hearts retained.

## Recommendations

1. **Increase “share” penalties for keeping multiple Ten+ hearts when only one premium moves**, especially for Left passes to South/North.
2. **Introduce directional scaling for South/East seats**, since South now shows three high-probability misses after the latest tuning.
3. **Add targeted regression fixtures** for the 223 and 912 patterns to ensure future adjustments don’t revert the penalties to zero.
