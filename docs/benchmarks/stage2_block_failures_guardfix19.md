# Stage 2 Block Failures — guardfix19

The run `stage2_pass_moon_v045_guardfix19` (left-pass block, 4×1024 hands) completed without any block failures. Two thousand eighty-three block-pass events were logged with a 100% success rate and the average best-vs-next margin fell to 6.10. No partner or self moons were observed in the block scenarios.

| Metric | Value |
| --- | --- |
| Block-pass events | 2,083 |
| Block-pass successes | 2,083 |
| Block-pass failures | 0 |
| Average best vs. next margin | 6.10 |
| Average moon probability | 0.361 |
| Total moons (all seats) | 24 |
| Moon rate (baseline_normal) | 0.59% |

## Self-moon Cases (baseline_normal)

| Hand | Perm | Passed Cards |
| ---: | ---: | --- |
| 26 | 3 | 8♥ J♥ K♣ |
| 76 | 0 | Q♠ 3♥ 9♠ |
| 76 | 1 | Q♠ 3♥ 9♠ |
| 96 | 0 | Q♠ 8♥ 10♥ |
| 96 | 1 | Q♠ 8♥ 10♥ |
| 462 | 3 | J♥ Q♥ K♠ |
| 464 | 2 | Q♠ Q♦ 4♠ |
| 481 | 0 | 2♠ J♠ A♦ |
| 481 | 1 | 2♠ J♠ A♦ |
| 522 | 2 | Q♠ 5♥ 6♣ |
| 534 | 0 | Q♠ 7♥ 6♠ |
| 534 | 1 | Q♠ 7♥ 6♠ |
| 534 | 2 | Q♠ 7♥ 6♠ |
| 599 | 2 | 6♥ 6♠ 10♠ |
| 609 | 0 | Q♠ 6♥ 10♦ |
| 609 | 1 | Q♠ 6♥ 10♦ |
| 778 | 3 | Q♠ 4♥ 2♠ |
| 837 | 2 | 5♥ 10♥ K♣ |
| 839 | 3 | 2♥ 3♠ 5♠ |
| 865 | 3 | Q♠ 3♥ J♥ |
| 941 | 3 | 6♥ 3♠ 4♠ |
| 968 | 2 | N/A |
| 999 | 0 | N/A |
| 999 | 1 | N/A |

> All 24 moons were self-shoots; no opponent moons recorded in guardfix19.

## Stage 2 Block Failures — guardfix19a

Follow-up run `stage2_pass_moon_v045_guardfix19a` (same Stage 2 deck, guard tweaks applied) also recorded zero block failures, but the block-pass volume increased to **16,433** evaluations with an average best-vs-next margin of **15.12**. Self moons dropped to **20** (0.49 % of baseline_normal hands), but the surviving shoots concentrate on mixed premium dumps and new queen-doublet leaks.

| Metric | Value |
| --- | --- |
| Block-pass events | 16,433 |
| Block-pass successes | 16,433 |
| Block-pass failures | 0 |
| Average best vs. next margin | 15.12 |
| Average moon probability | 0.361 |
| Total moons (all seats) | 20 |
| Moon rate (baseline_normal) | 0.49% |

### Self-moon Cases (baseline_normal)

| Hand | Perm | Passed Cards |
| ---: | ---: | --- |
| 29 | 0 | Q♠ Q♥ 2♥ |
| 29 | 1 | Q♠ Q♥ 2♥ |
| 96 | 0 | Q♠ 8♥ 10♥ |
| 96 | 1 | Q♠ 8♥ 10♥ |
| 214 | 2 | 3♥ 2♣ K♣ |
| 462 | 3 | J♥ Q♥ K♠ |
| 481 | 0 | 2♠ J♠ A♦ |
| 481 | 1 | 2♠ J♠ A♦ |
| 534 | 0 | Q♠ 7♥ 6♠ |
| 534 | 1 | Q♠ 7♥ 6♠ |
| 534 | 2 | Q♠ 7♥ 6♠ |
| 599 | 2 | 6♥ 6♠ 10♠ |
| 691 | 2 | Q♠ 6♥ J♥ |
| 839 | 3 | 2♥ 3♠ 5♠ |
| 865 | 3 | Q♠ 3♥ J♥ |
| 893 | 2 | Q♠ 2♥ 5♠ |
| 941 | 3 | 6♥ 3♠ 4♠ |
| 968 | 2 | 2♥ 3♣ A♣ |
| 999 | 0 | 5♣ 7♠ A♠ |
| 999 | 1 | 5♣ 7♠ A♠ |

> All 20 guardfix19a moons were self-shoots; no opponent moons recorded in the follow-up run.

## Stage 2 Block Failures — guardfix19e

Run `stage2_pass_moon_v045_guardfix19e` captures the latest queen-guard adjustments: **14,936** block-pass events, **100 %** success, average best-vs-next margin **13.39**, and **17** self moons (0.41 %).

| Metric | Value |
| --- | --- |
| Block-pass events | 14,936 |
| Block-pass successes | 14,936 |
| Block-pass failures | 0 |
| Average best vs. next margin | 13.39 |
| Average moon probability | 0.361 |
| Total moons (all seats) | 17 |
| Moon rate (baseline_normal) | 0.41% |

| Hand | Perm | Passed Cards |
| ---: | ---: | --- |
| 29 | 0 | Q♠ Q♥ 2♥ |
| 29 | 1 | Q♠ Q♥ 2♥ |
| 96 | 0 | Q♠ 8♥ 10♥ |
| 96 | 1 | Q♠ 8♥ 10♥ |
| 214 | 2 | 3♥ 2♣ K♣ |
| 462 | 3 | J♥ Q♥ K♠ |
| 481 | 0 | 2♠ J♠ A♦ |
| 481 | 1 | 2♠ J♠ A♦ |
| 509 | 2 | 6♣ 9♥ 10♥ |
| 599 | 2 | 6♥ 6♠ 10♠ |
| 691 | 2 | Q♠ 6♥ J♥ |
| 839 | 3 | 2♥ 3♠ 5♠ |
| 865 | 3 | Q♠ 3♥ J♥ |
| 941 | 3 | N/A |
| 968 | 2 | N/A |
| 999 | 0 | N/A |
| 999 | 1 | N/A |

> All 17 guardfix19e moons remain self-shoots; queen soft-anchor protection still allows `{Q♠,Q♥,2♥}` to slip through the fallback path.

## Stage 2 Block Failures — guardfix20

Run `stage2_pass_moon_v045_guardfix20` (left-pass block, 4×1024 hands) sustained **100 %** block-pass success across **14,508** pass events. The average best-vs-next margin held at **13.16**, and telemetry recorded **17** self moons (0.41 % of baseline_normal deals) with no partner or opponent moons.

| Metric | Value |
| --- | --- |
| Block-pass events | 14,508 |
| Block-pass successes | 14,508 |
| Block-pass failures | 0 |
| Average best vs. next margin | 13.16 |
| Average moon probability | 0.361 |
| Total moons (all seats) | 17 |
| Moon rate (baseline_normal) | 0.41% |

| Hand | Perm | Passed Cards |
| ---: | ---: | --- |
| 8 | 3 | 4♥ 2♠ 9♠ |
| 29 | 0 | Q♠ Q♥ 2♥ |
| 29 | 1 | Q♠ Q♥ 2♥ |
| 96 | 0 | Q♠ 8♥ 10♥ |
| 96 | 1 | Q♠ 8♥ 10♥ |
| 214 | 2 | 3♥ 2♣ K♣ |
| 462 | 3 | J♥ Q♥ K♠ |
| 481 | 0 | 2♠ J♠ A♦ |
| 481 | 1 | 2♠ J♠ A♦ |
| 599 | 2 | 6♥ 6♠ 10♠ |
| 691 | 2 | Q♠ 6♥ J♥ |
| 839 | 3 | 2♥ 3♠ 5♠ |
| 852 | 3 | 4♥ J♠ K♦ |
| 865 | 3 | Q♠ 3♥ J♥ |
| 968 | 2 | N/A |
| 999 | 0 | N/A |
| 999 | 1 | N/A |

> All guardfix20 moons were self shoots; the failure set now includes the new `{4♥, J♠, K♦}` soft anchor and a lingering low-heart triple on hand 8.

## Stage 2 Block Failures — guardfix21

Run `stage2_pass_moon_v045_guardfix21` kept the streak of **100 %** block-pass success while logging **14,483** pass events. The average best-vs-next margin dropped to **12.19**, moon probability stayed flat at **0.361**, and **14** self moons (0.34 %) were recorded — no partner or opponent moons were observed.

| Metric | Value |
| --- | --- |
| Block-pass events | 14,483 |
| Block-pass successes | 14,483 |
| Block-pass failures | 0 |
| Average best vs. next margin | 12.19 |
| Average moon probability | 0.361 |
| Total moons (all seats) | 14 |
| Moon rate (baseline_normal) | 0.34% |

| Hand | Perm | Passed Cards |
| ---: | ---: | --- |
| 8 | 3 | 4♥ 2♠ 9♠ |
| 26 | 3 | 8♥ J♥ K♣ |
| 96 | 0 | Q♠ 8♥ 10♥ |
| 96 | 1 | Q♠ 8♥ 10♥ |
| 119 | 0 | K♣ 3♥ J♥ |
| 119 | 1 | K♣ 3♥ J♥ |
| 459 | 2 | 8♠ 8♥ 10♥ |
| 462 | 3 | J♥ Q♥ K♠ |
| 592 | 0 | Q♣ 4♥ J♥ |
| 592 | 1 | Q♣ 4♥ J♥ |
| 599 | 2 | 6♥ 6♠ 10♠ |
| 852 | 3 | A♥ K♥ 4♥ |
| 865 | 3 | Q♠ 3♥ J♥ |
| 941 | 3 | N/A |

> Guardfix21 removed the previous `{Q♠,Q♥}` soft-anchor leak but newly surfaced early-shoot failures on hands 26/119 (double-low heart anchors) and left-pass triples on 8/852 that still strand all high hearts.

## Stage 2 Block Failures — guardfix22

Run `stage2_pass_moon_v045_guardfix22` continued the 100 % success streak with **14,856** block-pass evaluations. The average best-vs-next margin eased slightly to **13.61**, moon probability remained **0.361**, and self moons fell to **11** (0.27 %). No partner or opponent moons were recorded.

| Metric | Value |
| --- | --- |
| Block-pass events | 14,856 |
| Block-pass successes | 14,856 |
| Block-pass failures | 0 |
| Average best vs. next margin | 13.61 |
| Average moon probability | 0.361 |
| Total moons (all seats) | 11 |
| Moon rate (baseline_normal) | 0.27% |

| Hand | Perm | Passed Cards |
| ---: | ---: | --- |
| 26 | 3 | 8♥ J♥ K♣ |
| 96 | 0 | Q♠ 8♥ 10♥ |
| 96 | 1 | Q♠ 8♥ 10♥ |
| 212 | 0 | Q♠ J♥ 5♠ |
| 212 | 1 | Q♠ J♥ 5♠ |
| 420 | 0 | 5♥ 4♠ K♠ |
| 420 | 1 | 5♥ 4♠ K♠ |
| 599 | 2 | 6♥ 6♠ A♠ |
| 928 | 0 | 9♥ J♦ A♣ |
| 928 | 1 | 9♥ J♦ A♣ |
| 941 | 3 | N/A |

> Guardfix22 replaces the earlier single-low-heart leaks, but new problem sets have surfaced: double-heart anchors without premium liability (26/420/599) and mixed premium dumps (212/928) still strand key hearts or abuse low spades. Hand 941 remains unsolved—the forced guard still refuses to ship any safe mix.

## Stage 2 Block Failures — guardfix22b

Run `stage2_pass_moon_v045_guardfix22b` (latest guard tweaks) recorded **13,973** pass events with **100 %** success, but the average best-vs-next margin ballooned to **17.82** and self moons spiked to **57** (1.39 %). All recorded moons were self-shoots from the North seat.

| Hand | Perm | Passed Cards |
| ---: | ---: | --- |
| 119 | 0 | K♣, 3♥, J♥ |
| 119 | 1 | K♣, 3♥, J♥ |
| 310 | 2 | Q♠, A♣, 5♠ |
| 367 | 2 | Q♠, 10♦, 7♠ |
| 462 | 3 | J♥, Q♥, K♠ |
| 464 | 2 | Q♠, Q♦, 4♠ |
| 481 | 0 | 2♠, J♠, A♦ |
| 481 | 1 | 2♠, J♠, A♦ |
| 597 | 3 | A♥, 6♥, 3♠ |
| 607 | 2 | Q♠, 5♣, 8♦ |
| 852 | 3 | A♥, K♥, 4♥ |
| 865 | 3 | Q♠, 3♥, J♥ |
| 887 | 3 | Q♠, 2♠, 10♦ |
| 999 | 0 | N/A (telemetry truncated beyond hand 907) |
| 999 | 1 | N/A (telemetry truncated beyond hand 907) |
| 1014 | 3 | N/A (telemetry truncated beyond hand 907) |

> The new guard paths cured the previous low-heart dumps on hand 420 but reintroduced several historical moon patterns (119/310/481/852). Telemetry retention only covers hands ≤ 907, so the exact pass mixes for 999/1014 were not captured; the deals still ended in self moons for North.
