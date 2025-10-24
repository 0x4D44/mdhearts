Tuning Pass 2 — Mid-trick continuation (deterministic)

Setup
- Tools: `--scan-explain-after`, `--explain-once-after` with `--until-penalties`/`--until-void`, Hard deterministic with step cap.
- Weights baseline vs tuned (env-only):
  - Baseline defaults (documented in search.rs): FEED=60, SELF=80, QS=5, HANDOFF=5, NEXTTRICK_SINGLETON=25.
  - Tuned trial: FEED=70, SELF=90, QS=8, HANDOFF=8 (and a separate run with NEXTTRICK_SINGLETON=40 for a single seed).

Scans
- East penalties: designs/tuning/scan_east_2000_50_until_pen.csv (cont_nonzero > 0 for seed 2000)
- North voids: designs/tuning/scan_north_1100_120_until_void.csv (cont_nonzero > 0 for seeds 1104, 1107)
- West penalties: designs/tuning/scan_west_1000_150_until_pen.csv (cont_nonzero > 0 e.g., 1002, 1010)
- South penalties: designs/tuning/scan_south_1080_120_until_pen.csv (cont_nonzero > 0 e.g., 1095, 1097)

Explains (baseline vs tuned)
- East 2000 until-penalties: no ordering flip; singleton bump raised totals evenly.
- West 1002, 1010 until-penalties: no ordering flip.
- South 1095, 1097 until-penalties: no ordering flip.
- North 1104, 1107 until-void: no ordering flip.

Artifacts (selected)
- explain_after_2000_east_until_pen_out.txt
- explain_after_1104_north_until_void_out.txt
- explain_after_1107_north_until_void_out.txt
- explain_after_1002_west_base.txt / _tuned.txt
- explain_after_1097_south_base.txt / _tuned.txt

Takeaways
- Many mid-trick states exercised NEXTTRICK_SINGLETON and similar small continuation parts across multiple candidates, leading to uniform deltas and no reordering.
- For continuation to change rankings, we need states where only some candidates earn feed bonuses (penalties to leader), self-capture penalties, QS risk, or control handoff — not singleton-next gains shared by all.

Next steps
- Use `--scan-explain-after` results (cont_nonzero > 0) and then inspect verbose explains to filter for states where continuation terms differ across candidates (e.g., only one candidate would feed the leader; only one loses control into a known void). Prioritize such seeds for tuning.
- If ad‑hoc seeds remain elusive, construct minimal scenarios (like existing hard_multi_void_followups) for targeted flipping goldens and calibrate weights conservatively.
- Do not change defaults yet; keep current HardWeights and revisit after collecting discriminative cases.

