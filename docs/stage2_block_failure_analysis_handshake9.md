# Stage 2 Block Failure Analysis — handshake9 (2025-10-18)

The latest benchmark `stage2_pass_moon_v045_handshake9` (1,024 hands × 4 permutations) applies stronger Left-pass penalties for premium heart handoffs. Block-shooter passes succeeded **97.97 %** of the time (2,168 / 2,213), up from 97.66 % in handshake8. Only **7** ≥ 0.60 inverted-moon failures remain — a 42 % reduction.

| Hand | Perm | Seat | Direction | Moon Prob | Penalty | Passed Cards | Shooter |
| ---: | ---: | --- | --- | ---: | ---: | --- | --- |
| 153 | 3 | south | Left | 0.640 | 0.30 | 2♥, K♥, A♥ | north |
| 498 | 3 | east | Left | 0.640 | 7.10 | 4♥, Q♥, K♥ | north |
| 615 | 3 | north | Left | 0.631 | -13.10 | J♥, Q♥, A♥ | east |
| 767 | 0 | north | Left | 0.640 | -6.80 | 3♥, 4♥, A♥ | south |
| 767 | 1 | north | Left | 0.640 | -6.80 | 3♥, 4♥, A♥ | south |
| 912 | 0 | north | Left | 0.640 | -4.70 | 10♥, J♥, A♥ | south |
| 912 | 1 | north | Left | 0.640 | -4.70 | 10♥, J♥, A♥ | south |

_Penalty_ column shows the selected combo’s `moon_liability_penalty`. Negative figures indicate the underlying scoring still prefers these trios despite the new adds; their magnitude is much lower than in prior runs, revealing the penalties are biting.

## Seat distribution & trends

- North now accounts for 5 of 7 misses (down from 6/12). South contributes 1, East 1, West 0.
- South hand 153 still keeps two premium hearts with zero penalty; we need a dedicated rule for “K♥+A♥ to the left” when the belief suggests the left seat is the likely shooter.
- East hand 498 carries a modest penalty (≈7.1) yet still wins because alternative combos score worse. Consider boosting the premium-handoff cost when multiple players to the left hold high moon probabilities.

## Planned follow-ups

1. Tie directional penalties to projected shooter seat (e.g., belief mass on left neighbour) so cases like hand 153 receive stronger discouragement.
2. Introduce an explicit penalty when three premium hearts are passed in a single Left handoff, regardless of remaining hearts.
3. Add regression fixtures for hands 153 and 498 once the next tuning pass lands, to lock in the reduced failure count.
