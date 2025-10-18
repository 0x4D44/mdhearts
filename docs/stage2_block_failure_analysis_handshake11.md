# Stage 2 Block Failure Analysis — handshake11 (2025-10-18)

Run `stage2_pass_moon_v045_handshake11` (1,024 hands × 4 perms) integrates belief-weighted directional penalties and the Ace-only guard. Block-shooter passes succeeded **98.1 %** of the time (2,191 / 2,233). The ≥ 0.60 failure list remains at **6 inverted moons**, identical to handshake10 but with much larger negative totals for the selected combos, indicating the optimizer now views them as significantly worse.

| Hand | Perm | Seat | Direction | Moon Prob | Penalty | Passed Cards | Shooter |
| ---: | ---: | --- | --- | ---: | ---: | --- | --- |
| 153 | 3 | south | Left | 0.640 | -35.8 | 2♥, K♥, A♥ | north |
| 615 | 3 | north | Left | 0.631 | -38.5 | J♥, Q♥, A♥ | east |
| 767 | 0 | north | Left | 0.640 | -61.8 | 3♥, 4♥, A♥ | south |
| 767 | 1 | north | Left | 0.640 | -61.8 | 3♥, 4♥, A♥ | south |
| 912 | 0 | north | Left | 0.640 | -22.0 | 10♥, J♥, A♥ | south |
| 912 | 1 | north | Left | 0.640 | -22.0 | 10♥, J♥, A♥ | south |

## Key takeaways

- **Penalty magnitude:** All remaining misses carry large negative totals (−22 to −62). The new scoring shifts the optimizer toward safer options, but these deals lack better candidates (e.g., only low hearts remain), so the fallback still chooses them.
- **Seat focus:** Failures are concentrated on North (4) with one South case. East/West no longer appear after the shooter-aware scaling.
- **Next focus areas:** Need targeted heuristics for: (a) splitting K♥/A♥ when belief mass indicates the left neighbour is the likely shooter (hand 153), and (b) avoiding “Ace + small hearts” when only low cards remain (hands 767/912). Consider combining penalties with rule-based overrides (e.g., forced premium split when shooter pressure exceeds a threshold).

With the failure count plateauing at 6, the next iteration should inject explicit overrides or alternative candidate generation for these edge scenarios.
