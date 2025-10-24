2025-10-21 — Addendum (23:55)

Summary
- Verified repo state; full test suite green (cargo test).
- Confirmed CLI popups are opt-in via `MDH_CLI_POPUPS`; default output remains console to avoid desktop dialogs.
- Saved continuation roadmap: `designs/2025.10.21 - Hard AI Continuation Plan v2.md` covering tuning, determinism tests, time-cap stress, adaptive thresholds refinement (gated), benches guardrails, CLI/docs polish, and match-batch evaluation.

Next
- Collect explain logs with `MDH_DEBUG_LOGS=1 MDH_HARD_VERBOSE_CONT=1` for a small set of known disagreements.
- Trial tiny deltas (+5) to QS risk and control handoff weights via env; re-run compare-batch and verify goldens remain stable.
- If consistently beneficial, propose promoting defaults and update docs/goldens accordingly.

Tuning pass 1 (deterministic, step-capped)
- Commands
  - Explain JSON (baseline):
    - `mdhearts --explain-json 1040 west designs/tuning/explain_1040_west_hard_new_baseline.json hard --hard-deterministic --hard-steps 80`
    - `mdhearts --explain-json 1145 north designs/tuning/explain_1145_north_hard_new_baseline.json hard --hard-deterministic --hard-steps 80`
    - `mdhearts --explain-json 1082 west designs/tuning/explain_1082_west_hard_new_baseline.json hard --hard-deterministic --hard-steps 80`
  - Explain JSON (tuned env): set `MDH_HARD_QS_RISK_PER=10`, `MDH_HARD_CTRL_HANDOFF_PEN=10` and repeat to `*_tuned.json` files.
  - Compare (baseline vs tuned):
    - `--compare-batch west 1000 100 --only-disagree --out designs/tuning/compare_west_1000_100_new.csv --hard-deterministic --hard-steps 80`
    - with env deltas → `designs/tuning/compare_west_1000_100_new_tuned.csv`.
- Artifacts
  - designs/tuning/explain_1040_west_hard_new_baseline.json
  - designs/tuning/explain_1145_north_hard_new_baseline.json
  - designs/tuning/explain_1082_west_hard_new_baseline.json
  - designs/tuning/explain_1040_west_hard_new_tuned.json
  - designs/tuning/explain_1145_north_hard_new_tuned.json
  - designs/tuning/explain_1082_west_hard_new_tuned.json
  - designs/tuning/compare_west_1000_100_new.csv
  - designs/tuning/compare_west_1000_100_new_tuned.csv
- Outcome
  - Disagreement rows unchanged for West 1000..1099 (seeds 1040, 1082, 1097). Tiny +5 deltas to QS risk and control handoff did not flip these specific cases.
  - Verbose continuation parts for these openings were 0; next pass should include mid-trick scenarios or seeds where QS risk/hand-off apply.
- Next
  - Expand compare-batch across other seats/ranges (e.g., East 2000..2099, North 1100..1219) and gather explain JSON for a few non-opening, follow-up decisions where continuation signals are active.
  - Trial small deltas to `MDH_HARD_CONT_FEED_PERPEN` and `MDH_HARD_CONT_SELF_CAPTURE_PERPEN` (+5 each) and re-check for any flips/regressions.

Tuning pass 2 (broader ranges)
- Compare CSVs (baseline):
  - designs/tuning/compare_east_2000_100_new.csv (rows: 4)
  - designs/tuning/compare_north_1100_120_new.csv (rows: 6)
- Explain JSONs (baseline, hard deterministic):
  - designs/tuning/explain_2031_east_hard_new_baseline.json
  - designs/tuning/explain_2044_east_hard_new_baseline.json
  - designs/tuning/explain_1145_north_hard_new_baseline.json
  - designs/tuning/explain_1162_north_hard_new_baseline.json
- Compare CSVs (tuned feed/self caps: +5 each):
  - designs/tuning/compare_east_2000_100_new_tuned.csv (rows unchanged)
  - designs/tuning/compare_north_1100_120_new_tuned.csv (rows unchanged)
- Observation
  - Disagreement counts unchanged with small `cont_feed_perpen` and `cont_self_capture_perpen` increases. Selected seeds still appear to be early trick or cases where these continuation parts don’t activate strongly.
- Next
  - Target mid-trick follow-up scenarios more explicitly (consider snapshot-based explains if available) to exercise continuation signals (feed/self/QS risk/hand-off).
  - If snapshot tooling is limited, widen ranges and harvest 2–3 additional disagreements per seat; focus explain on cases showing penalties on table or known voids.
  - Hold off on promoting default changes until we see clear flips or better separation in explain deltas without regressions.
Explain-after examples
- mdhearts.exe --explain-once-after 2031 east 10 hard --hard-deterministic --hard-steps 120
- mdhearts.exe --explain-once-after 1145 north 6 hard --hard-deterministic --hard-steps 120

New tools
- Added --explain-once-after with --until-penalties/--until-void to target mid-trick states.
- Added --scan-explain-after to sweep seed ranges and output CSV with continuation activity counts.

Artifacts
- explain_after_2000_east_until_pen_out.txt (shows non-zero continuation parts, e.g., 25)
- scan_east_2000_50_until_pen.csv (includes seeds where cont_nonzero>0; e.g., 2000)
- explain_after_1104_north_until_void_out.txt, explain_after_1107_north_until_void_out.txt (non-zero continuation parts)

Next
- Use scan outputs to pick 2–3 mid-trick seeds per seat with cont_nonzero>0; run tuned env deltas (feed/self/QS/handoff) and evaluate flips.
- If consistent wins, propose promoting tiny default deltas in HardWeights; expand goldens to include these constructed mid-trick cases.
Artifacts (cont.)
- scan_west_1000_150_until_pen.csv, scan_south_1080_120_until_pen.csv (more cont-active seeds)
- explain_after_1002_west_base.txt vs _tuned.txt (no flip with FEED/SELF/QS/HANDOFF +10/+10/+3/+3)
- explain_after_1097_south_base.txt vs _tuned.txt (no flip with same deltas)
- designs/tuning/tuning_pass_2_summary.md saved

Next addendum
- Prioritize states where continuation differs across candidates (feed/self/QS/handoff only on a subset) to allow reordering; avoid uniform singleton bonuses.


Seed scan to tuned pass summary
- Shortlisted discriminative seeds from scans (0 < cont_nonzero < legal): east 2012/2022/2027; west 1002/1003/1008; south 1081/1086/1088; north 1104/1122/1131.
- Baseline vs stronger deltas (FEED 85, SELF 100, QS 12, HANDOFF 12) did not flip top choices; scores shifted but base gaps dominated.
- Drafted continuation tie-break proposal to gently boost continuation only when base gap is small: designs/2025.10.21 - Hard continuation tie-break proposal.md.

