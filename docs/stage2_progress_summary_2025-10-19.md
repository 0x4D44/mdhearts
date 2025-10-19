# Stage 2 Progress Summary — 2025-10-19

## Recent Progress
- Hardened the soft-anchor pipeline: the guard now rejects double-queen exports unless the pass carries a premium off-suit liability and Ten+ hearts are exhausted, and `violates_ten_plus_safety`/`violates_support_guard` were relaxed so double-liability queen mixes and mid-support upgrades survive when that is the only safe handoff. New regression coverage (`stage2_hand_29_force_guard_respects_soft_anchor_rules`, `stage2_hand_852_promotes_ten_plus_heart`, `stage2_hand_968_enumerator_returns_liability_mix`) keeps the guard behaviour anchored.
- Rebuilt the fallback enumerator so Ten+-shortage hands inject mixed-liability candidates before the guard synthesiser fires. Single low-heart shipments are now blocked unless all Ten+ hearts are gone, and the forced guard mirrors the same rule (`stage2_hand_8_blocks_single_low_heart_dump`, `stage2_hand_767_enumerator_returns_liability_mix`).
- Landed the targeted guardfix22 clean-up: left-pass combos now require double-heart support when they lean on soft off-suit anchors, Q♠+J♥ mixes are rejected while premium support remains, and the forced guard synthesiser gained liability/stopper-aware fallbacks plus an Ace/King heart filter. New regression cases (`stage2_hand_26_rejects_soft_anchor_liability`, `stage2_hand_212_blocks_queen_ten_mix`, `stage2_hand_599_passes_both_low_hearts`, `stage2_hand_928_forces_strong_spade_anchor`, `stage2_hand_767_forced_prefers_liability_mix`) lock the behaviour in place.
- Ran Stage 2 with guardfix21 (`stage2_pass_moon_v045_guardfix21`): **14,483** pass events, **100 %** success, average best-vs-next margin **12.19**, and **14** self moons (0.34 %). Artefacts live under `bench/out/stage2_pass_moon_v045_guardfix21/`, the failure log has a guardfix21 section, and the run ledger now records the run.
- Followed up with guardfix22 (`stage2_pass_moon_v045_guardfix22`): **14,856** pass events, **100 %** success, average best-vs-next margin **13.61**, and **11** self moons (0.27 %). Artefacts live under `bench/out/stage2_pass_moon_v045_guardfix22/`, the guardfix log is updated, and the run ledger includes the new metrics.
- Re-ran Stage 2 as guardfix22b (`stage2_pass_moon_v045_guardfix22b`): **13,973** pass events, **100 %** success, but the average best-vs-next margin jumped to **17.82** and North logged **57** self moons (1.39 %). Failures concentrate on Q♠ anchors and low-heart doubles (hands 119/310/367/462/464/481/597/607/852/865/887/999/1014).

## Outstanding Gaps
- Self moons now span a wider set of North hands (119, 310, 367, 462, 464, 481, 597, 607, 852, 865, 887, 999, 1014), with telemetry truncation above hand 907 keeping the pass detail for 999/1014 out of the log.
- The baseline_easy lead remains wide at **+2.43 PPH**, so pass scoring is still inflating the margin even after the latest guard tweaks.
- The average best-vs-next margin inflated to **17.82** on guardfix22b, signalling the current liability weights over-reward the chosen pass mix.

## Next Steps
1. Finish the guard pass for the remaining moon hands (96/119/310/367/462/464/481/597/607/852/865/887/999/1014) and rerun Stage 2 to confirm the regression set shrinks.
2. Run a scoring/weight audit (liability vs. moon penalty vs. direction bonuses) to bring the best-vs-next margin back toward ≤ 12 and shrink the baseline_easy delta.
3. After the next guard iteration, capture fresh Stage 2 telemetry (with retention beyond hand 907) and refresh the guardfix log + progress summary.
