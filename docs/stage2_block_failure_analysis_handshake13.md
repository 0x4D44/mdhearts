# Stage 2 Block Failure Analysis — handshake13 (2025-10-18)

Run `stage2_pass_moon_v045_handshake13` (1,024 hands × 4 permutations) exercises the updated premium-heart guard with fallback heart substitution. Block-shooter passes succeeded **97.06 %** of the time (1,916 / 1,974). The ≥ 0.60 failure list is still the familiar **16 inverted moons**, and the failing trios match handshake12 because the baseline heuristic in `hearts-bot` continues to bypass the shared optimizer guard.

| Hand | Perm | Seat | Direction | Moon Prob | Penalty | Passed Cards | Shooter |
| ---: | ---: | --- | --- | ---: | ---: | --- | --- |
| 75 | 0 | west | Left | 0.640 | 0.0 | K♠, A♥, Q♥ | north |
| 75 | 1 | west | Left | 0.640 | 0.0 | K♠, A♥, Q♥ | north |
| 153 | 3 | south | Left | 0.640 | -18 489.3 | 10♥, A♥, K♥ | north |
| 242 | 0 | north | Left | 0.640 | 0.0 | K♠, A♥, Q♥ | east |
| 242 | 1 | north | Left | 0.640 | 0.0 | K♠, A♥, Q♥ | east |
| 432 | 2 | east | Left | 0.730 | -24 724.8 | Q♥, A♥, K♥ | south |
| 461 | 2 | north | Left | 0.640 | -14 188.6 | 10♥, Q♥, K♥ | east |
| 498 | 3 | east | Left | 0.640 | -12 602.3 | J♥, Q♥, K♥ | north |
| 511 | 0 | south | Left | 0.640 | 0.0 | K♠, K♥, Q♥ | west |
| 511 | 1 | south | Left | 0.640 | 0.0 | K♠, K♥, Q♥ | west |
| 681 | 2 | north | Left | 0.730 | -27 822.0 | Q♥, A♥, K♥ | east |
| 757 | 3 | north | Left | 0.680 | 0.0 | A♥, K♥, A♣ | east |
| 767 | 0 | north | Left | 0.640 | 0.0 | A♥, K♥, 9♥ | south |
| 767 | 1 | north | Left | 0.640 | 0.0 | A♥, K♥, 9♥ | south |

## Observations

- **Guard working in isolation.** Unit tests and the optimizer confirm that `enumerate_pass_triples` now upgrades low-support passes to include a third heart even when only two Ten+ hearts are available.
- **Integration gap.** The harness still records the legacy trios because the production heuristic path in `hearts-bot` does not yet consume the guarded optimizer (see `crates/hearts-bot/src/policy/heuristic.rs`). Until the optimizer drives the live selection, the fallback logic cannot influence real passes.
- **Penalty magnitudes unchanged.** Hands passing through the shared optimizer (153/432/461/498/681) still carry five-digit negative totals, demonstrating that the scoring path reacts correctly once invoked.

## Next actions

1. Wire `pass_v2` / heuristic selection through `enumerate_pass_triples` so the hard guard and fallback substitution affect live passes.
2. After integration, re-run the Stage 2 benchmark to verify that the K♠/A♣ anchors disappear from the failure table.
3. Keep the failure snapshots (`docs/benchmarks/stage2_block_failures_handshake13.md`) handy as the regression target when the integration lands.
