2025-10-24 — Hard polish and docs/telemetry

Changes
- Added tiny Normal-tier next3 gate telemetry:
  - Stats now include `next3_tiny_hits` for the last Hard decision.
  - CLI prints it under `MDH_DEBUG_LOGS=1` in explain paths.
- Documented a simple “Hard defaults gate” recipe:
  - How to enable deterministic determinization (choose-only) for Hard via env.
  - Added examples in README and docs/CLI_TOOLS.md.
- Clarified limits precedence between global and Wide-tier-only envs in docs/CLI_TOOLS.md.

Next up
- Add a CLI smoke test for `--match-mixed-file` with a tiny seeds file.
- Consider slightly stronger Wide-tier-only continuation boosts (choose-only) and evaluate via mixed-seat runs.
- If aggregate advantage remains ≈0, proceed to determinization tuning and/or deeper selective branching (still budget-capped).

Notes
- Popups remain off by default (MDH_CLI_POPUPS unset); CLI prints to console.
- All tests previously green; rerun full suite after next changes.

Evaluation (mixed-seat, deterministic)
- West seat, n=1000 per window; mixes NNHH (1000..1999) and HHNN (2000..2999).
- Baseline (no det default): CSVs under designs/tuning/ mixed_west_w1000_1000_nnhh_baseline.csv and ..._w2000_1000_hhnn_baseline.csv
- Determinization ON (choose-only K=3): CSVs ..._detk3.csv
- Trial Wide-tier boosts (feed=250‰, self=150‰, cap=250): CSVs ..._trial_boost.csv
- Summary markdown: designs/tuning/mixed_summary_2025-10-24_west.md (means + 95% CI). Early result: deltas remain ~0 within CI.

Expanded all-seats runs (n=1000/seat)
- Seats: west/north/east/south; mixes NNHH/HHNN/NHNH/HNNH; windows 1000..1999 and 2000..2999.
- Baseline, detk3, and trial_boost CSVs saved under designs/tuning/ with names mixed_{seat}_{start}_{count}_{mix}_{group}.csv.
- Summary: designs/tuning/mixed_summary_2025-10-24_allseats_n1000.md. Observation: overall means hover near 0 with CI including 0 across groups; no clear Hard>Normal gap yet under conservative knobs.

Trial with planner nudge (choose-only)
- Env: WIDE topK=12 nextM=6, boosts feed=300‰ self=180‰, cont_cap=250, detK=3, plus MDH_HARD_PLANNER_NUDGES=1 with leader-feed=10/pen and guard=200.
- CSVs: mixed_{seat}_{start}_{count}_{mix}_trial_nudge.csv for all seats/mixes.
- Updated summary (same file): includes trial_nudge. Observation: still near 0 with CI including 0; nudge remains conservative by design.

Stronger Wide-tier deepening (choose-only)
- Env: WIDE topK=14 nextM=7, boosts feed=350‰ self=180‰, cont_cap=250, detK=3.
- CSVs: mixed_{seat}_{start}_{count}_{mix}_trial_boost2.csv for all seats/mixes.
- Summary updated: includes trial_boost2 in designs/tuning/mixed_summary_2025-10-24_allseats_n1000.md. Observation: Overall mean remains ~0 with CI spanning 0 — conservative caps likely dampening aggregate movement.

Boost 3 + Nudge 2 trials
- Boost 3: feed boost=450‰ (else as above). CSVs: ..._trial_boost3.csv. Overall still ~0 with CI spanning 0.
- Nudge 2: planner leader-feed nudge=15/pen (guard=200) with Wide topK=14 nextM=7, feed boost=350‰. CSVs: ..._trial_nudge2.csv. Overall still ~0 with CI spanning 0.

Conclusion (today)
- Under current conservative caps and shallow continuation, aggregate improvements remain below detection in deterministic mixed-seat averages. Behavior-level goldens are in place; telemetry is available. Next lever would be modestly increasing continuation cap (e.g., 300–350) or adding a limited endgame micro-solver (≤3 cards) under strict budgets to create clearer signal — both guarded by tests and eval summaries.

Cap trials
- cap=300 (Wide: topK=14, nextM=7, feed boost=350‰): CSVs ..._trial_cap300.csv — Overall mean ~0 with CI spanning 0.
- cap=350 (Wide: topK=14, nextM=7, feed boost=450‰): CSVs ..._trial_cap350.csv — Overall mean ~0 with CI spanning 0.

Next recommendations
- Consider small, targeted endgame micro-solver (≤3 cards) for choose-only under strict budget, plus maintaining cont_cap ≤ 350. Keep explain deterministic and extend goldens to cover micro-solver cases.

Endgame micro-solver (env ON, tiny bonus)
- Env: DP_ENABLE=1, MAX_CARDS=3, BONUS=5; Wide: topK=14 nextM=7; feed boost=350‰; cont_cap=300; detK=3.
- CSVs: mixed_{seat}_{start}_{count}_{mix}_trial_endgame.csv for all seats/mixes.
- Summary updated (same combined file). Observation: Overall mean remains ~0 with CI spanning 0. A larger, still-capped micro-solver scaling or simple late-trick DP may be needed to see aggregate movement.

Endgame micro-solver BONUS=10 trial
- Env: DP_ENABLE=1, BONUS=10 (others as above). CSVs: ..._trial_endgame10.csv for all seats/mixes.
- Quick summary: Overall mean ~0 with CI ~0; deterministic setup suggests the current bonus magnitude under the chosen cap doesn’t shift aggregates.

---

Hard endgame micro-solver implementation
- Implemented tiny DP in crates/hearts-app/src/bot/search.rs:micro_endgame_bonus (choose-only, env-gated):
  - Triggers when all seats have = MDH_HARD_ENDGAME_MAX_CARDS (default 3).
  - Deterministically plays out up to that many remaining tricks using the existing void-aware follow-up policy and scores each trick with small next2 continuation weights (feed-to-leader vs self-capture).
  - Contribution is indirectly clamped by MDH_HARD_CONT_CAP in the caller.
- Telemetry: added ENDGAME_DP_COUNT (thread-local) and Stats.endgame_dp_hits; choose records per-decision hits.
- Docs: added docs/CLI_TOOLS_ADDENDUM.md to document the new env knobs (CLI_TOOLS.md has an encoding issue).
- Tests: full suite green; existing hard_endgame_dp_smoke.rs covers the enabled path. Next step is a constructed =3-card golden that flips a near-tie in favor of feeding the leader.


DP mixed-seat smoke (West)
- NNHH 1000..1099 and HHNN 2000..2099 with DP enabled (deterministic).
- CSV: designs/tuning/mixed_west_1000_100_nnhh_dp.csv, designs/tuning/mixed_west_2000_100_hhnn_dp.csv
- Summary: designs/tuning/mixed_summary_2025-10-24_west_dp_smoke.md
- Result: mean deltas both ~0 (as expected under tiny, capped continuation).


DP mixed-seat matrix (West) deterministic:
- Runs: starts 1000/2000; mixes NNHH/HHNN/NHNH/HNNH; n=300 each; DP enabled.
- Summary: designs/tuning/mixed_summary_2025-10-24_west_dp_matrix.md
- Result: means 0 across all cells under conservative continuation/caps.


DP full-seat matrix (deterministic):
- Seats: west/north/east/south; windows: 1000 & 2000; mixes: NNHH/HHNN/NHNH/HNNH; n=300 each.
- Summary: designs/tuning/mixed_summary_2025-10-24_allseats_dp_matrix.md
- Result: means all 0 under conservative endgame continuation caps.


DP eval (cap=350, next2_feed=60), deterministic:
- All seats, windows 1000/2000, mixes NNHH/HHNN/NHNH/HNNH, n=200 each.
- Summary: designs/tuning/mixed_summary_2025-10-24_allseats_dp_cap350_n2f60.md
- Result: means all 0. Current DP influence remains too conservative to show aggregate gain.

Next steps (plan captured)
- Wrote: designs/2025.10.24 - Hard Advantage Next Steps (DP + Flip).md
- Focus: craft robust ≤3-card flipping golden; then small, endgame-only tuning with guardrails; then full-seat matrices (n≥1000/seat) and CI-style summaries.


DP stats sample: stats run (west 1000..1049 nnhh): avg_pen=6.1 avg_scanned=0 sum_dp_hits=0


DP stats (A/B): match-batch (west 1000..1099 normal vs hard): mean_delta=0 avg_scanned=0 sum_dp_hits=0


Robust flipping golden plan added:
- designs/2025.10.24 - Robust DP Flip Golden Plan.md
- Next: reduce seed 2120 (west) to a minimal =3-card RoundState and aim to flip under current caps; if needed, apply a narrow endgame-only cap in the test.


DP flip tools added:
- --compare-dp-once and --export-endgame; docs updated in CLI_TOOLS_ADDENDUM.md.
- Built minimal w2120 RoundState test (ignored) and enabled boosted seed 2120 flip test.
- Next: iterate minimal w2120 to flip under current caps or narrowly scoped endgame cap, then enable.

