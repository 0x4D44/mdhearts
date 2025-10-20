# Stage 2 Progress Summary — 2025-10-19

## Recent Progress
- Re-instated the pre-guardfix22k heuristics so the core regression suite is green again. This keeps `force_guarded_pass` behaviour stable while we regroup on the remaining high-risk seeds.
- Extended the Stage 2 harness to emit a dedicated `pass_details.jsonl` stream; every pass (all 1 024 hands × 4 permutations) now records the selected trio, even beyond the legacy telemetry ceiling.
- Captured a fresh baseline run `stage2_pass_moon_v045_guardfix22l`: **14 254** pass events, **100 %** success, average best-vs-next margin **16.48**, and **18** North self moons (43/76/367/462/464/481/495/607/865/890/941/999 + high-index 941/999 now visible → 0.44 %). Artefacts live under `bench/out/stage2_pass_moon_v045_guardfix22l/`, and the run ledger along with `docs/benchmarks/stage2_block_failures_guardfix19.md` reflect the updated metrics.

## Outstanding Gaps
- North self moons remain concentrated on the historical Q♠ anchors and premium mixes: 43/76/367/462/464/481/495/607/865/890/941/999 (plus permutations 887/913/999 surfaced in the pass log). With pass-detail telemetry we now have complete card mixes for 941/999, removing the earlier visibility gap.
- The average best-vs-next margin sits at **16.48**, and `baseline_easy` still leads by **+2.48 PPH** versus baseline_normal. Liability weighting and direction bonuses continue to overvalue aggressive dumps.
- Guard heuristics are back at the guardfix22j baseline; the unresolved failure set (e.g., triple spade dumps on 495, mixed anchors on 43/76/481, premium heart mixes on 462/464/865/890) still needs targeted rules before we can tighten the pass pipeline again.

## Next Steps
1. Use the new pass-detail logs to design guard/enumerator tweaks that specifically suppress the remaining Q♠ anchors (43/76/481/495) and premium heart mixes (462/464/865/890/887/913/999).
2. Re-introduce guard changes incrementally with fresh regression coverage so `force_guarded_pass` never regresses to `None` on the known Stage 2 fixtures.
3. Kick off the scoring/weight audit (liability vs. moon penalty vs. direction bonus) to drag the best-vs-next margin toward single digits and shrink the `baseline_easy` advantage without reopening the older moon set.
