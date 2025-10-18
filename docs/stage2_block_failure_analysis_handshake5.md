# Stage 2 Block Failure Analysis — handshake5 run (2025-10-18)

The `stage2_pass_moon_v045_handshake5` benchmark (1,024 hands × 4 perms) produced nine block-shooter failures with estimated probability ≥ 0.60. All nine are inverted-moon outcomes. Seven originate from North (passing Left to South), one from West (Left to South), and one from East (Left to North).

## Failure snapshot

| Hand | Perm | Seat | Direction | Moon Prob | Penalty | Passed Cards | Notes |
| ---: | ---: | --- | --- | ---: | ---: | --- | --- |
| 32 | 2 | West | Left | 0.680 | 0.00 | 4♥, K♥, 10♣ | Keeps Q♥/J♥ (not passed); no penalty triggered |
| 208 | 0 | North | Left | 0.616 | 0.00 | 2♥, 10♥, A♥ | Only one premium heart sent; alternatives passing Q♥/K♥ get non-zero penalty but still ranked lower |
| 208 | 1 | North | Left | 0.616 | 0.00 | 2♥, 10♥, A♥ | Same deal / perm variant |
| 498 | 3 | East | Left | 0.640 | 0.00 | 4♥, J♥, K♥ | Keeps Q♥; mid-heart reinforcement missing |
| 615 | 3 | North | Left | 0.631 | 0.00 | J♥, Q♥, A♥ | Candidate passing K♥ has penalty (≈5.3) but remains below zero-penalty trio |
| 767 | 0 | North | Left | 0.640 | 0.00 | 3♥, 4♥, A♥ | Lacks premium hearts to hand off; only low hearts shipped |
| 767 | 1 | North | Left | 0.640 | 0.00 | 3♥, 4♥, A♥ | Same deal / perm variant |
| 912 | 0 | North | Left | 0.640 | 0.00 | 10♥, J♥, A♥ | Keeps Q♥/K♥; combos passing two premiums carry penalties (~5.2) but total score still lower |
| 912 | 1 | North | Left | 0.640 | 0.00 | 10♥, J♥, A♥ | Same deal / perm variant |

_Penalty_ is the `moon_liability_penalty` recorded for the selected combo.

## Observations

- **Zeros mean the new penalties did not trigger.** Every failure still selects a trio with zero moon penalty, indicating our current multipliers are too conservative whenever at least one premium heart goes out.
- **North→South dominates.** 7/9 cases are North passing Left to South, exactly the scenario highlighted by the earlier inverted-moon review. The remaining two also send high hearts to a neighbour who later shoots.
- **Single-premium handoffs remain attractive.** Hands 208, 498, 912 pass one premium heart alongside low cards. The optimizer penalises alternative combos (note the non-zero values in `top_penalties`), but the delta is not large enough to overcome total-score differences.
- **Low-heart dumps still slip through.** Hands 32, 767 lack penalty because the selected trio only contains one heart ≥ Ten. Additional logic is needed to enforce coverage when the seat holds multiple hearts above Ten but ships only one.

## Next steps

1. **Strengthen mid-tier penalties**: increase the additional cost when a blocker keeps more than one Ten+ heart, even if one premium heart is included in the pass.
2. **Directional scaling**: consider higher multipliers specifically for North/West left passes to South to reflect the observed risk concentration.
3. **Feature-gated tests**: add fixture(s) for the 208/912 scenarios to ensure we regress if future tuning reopens the single-premium loophole.
