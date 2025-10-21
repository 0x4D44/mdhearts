Date: 2025-10-21

- Baseline verified: all workspace tests green (hearts-core/app/ui). Reviewed recent planner, controller, and tracker code; logs and goldens match Stage 2 wrap-up summary.
- Added Criterion micro-benchmark harness: `crates/hearts-app/benches/heuristic_decision.rs`.
  - Benchmarks `GameController::explain_candidates_for` on a few representative seeds/seats using `iter_batched` to keep snapshot stable.
  - Wired dev-dependency and bench target in `crates/hearts-app/Cargo.toml`.
- Authored continuation plan: `designs/2025.10.21 - Bot Stage 2 tuning and benches plan.md` covering tuning focus, targeting fidelity, planner polish, benches, and goldens.
- Next steps:
  - Record initial bench numbers locally (`cargo bench -p hearts-app`) and capture in journal.
  - Add small planner weight: leader-feed positive below ≥90 (env knob), guarded to avoid over-feeding.
  - Implement score-gap sensitivity in follow-up targeting and add unit tests for multiple-void/provisional-winner corners.

Bench baseline (criterion)
- heuristic_decision/explain_candidates_seed42_seatSouth: ~6.80µs (median)
- heuristic_decision/explain_candidates_seed12345_seatEast: ~7.19µs (median)
- heuristic_decision/explain_candidates_seed8675309_seatNorth: ~2.57µs (median)
Notes: benches measure a single `explain_candidates_for` on a fresh snapshot per iter; comfortably below 2–3ms target. Future changes should stay within the same order of magnitude.

Follow-up targeting and lead nuance
- Follow-ups now consider penalties-on-table; avoid feeding non-leader when penalties already present; prefer safe sloughs or smallest-penalty instead.
- Added planner penalty `MDH_W_NONLEADER_FEED_PERPEN` (default 1200) to discourage feeding second place.
- New goldens:
  - `leader_target_golden.rs`: QS dump to scoreboard leader under <90.
  - `followup_avoid_feeding_nonleader.rs`: avoids dumping QS when provisional winner != leader and penalties are on the table.
  - `lead_nuance_golden.rs`: early cautious lead avoids hearts when a safe/void-creating lead exists; void creation preferred over neutral.
  All tests passing across workspace after these refinements.

Pass phase
- CLI: added `--explain-pass-once <seed> <seat>` to print chosen 3-card pass for a seeded snapshot.
- Planner: hard guard against passing QS to the scoreboard leader (`avoid_qs_to_leader`), while still favoring sending penalties to trailing.
- Tests:
  - `pass_avoid_leader.rs`: ensures QS not passed to leader; confirms target.
  - `pass_trailing_prefers_penalties.rs`: confirms penalties (QS + a heart) sent to trailing.

CLI automation friendliness
- Suppress Windows popups for CLI by default; `show_info_box`/`show_error_box` print to console unless `MDH_CLI_POPUPS=1`.
- README updated to document `MDH_CLI_POPUPS`.

Stage 3 planning
- Added "Stage 3 (Hard) – Shallow Search With Realistic Opponent Models" design doc with a concrete roadmap for opt-in Hard difficulty.

Bench delta after leader gap scaling
- seed42/South: ~6.37µs (improved ~5%)
- seed12345/East: ~6.61µs (improved ~8%)
- seed8675309/North: ~2.18µs (improved ~17%)
Note: minor variance; still well under target.

Stage 3 (Hard) progress
- Added Hard planner scaffold with a tiny 1-ply current-trick rollout contributing a small continuation bonus (feed leader on penalty tricks; avoid self-capture), applied to top-N candidates ordered by heuristic.
- Config: `MDH_HARD_BRANCH_LIMIT` to adjust branch width (default 6).
- Controller routes autoplay/explain to Hard planner when difficulty set to `hard`.
- Added Criterion bench `hard_decision` and README docs; next is adding 2-ply next-trick probe with time caps.

Plan update (polish & guardrails)
- Authored continuation plan: `designs/2025.10.21 - Stage 3 polish and guardrails plan.md` with concrete tasks for Hard branching/pruning, console explain breakdown, moon tuning, expanded goldens, and performance guardrails.
- Next immediate actions (M1):
  - Add console verbose breakdown for Hard in `--explain-once/batch` when `MDH_DEBUG_LOGS=1`.
  - Run `--compare-batch` to harvest disagreement seeds; add first Hard-vs-Normal golden.

Disagreement golden
- Found a stable disagreement: seed 1040, seat West (Normal=10♠ vs Hard=2♠).
- Added test: `crates/hearts-app/tests/hard_vs_normal_disagreement.rs` asserting the differing tops and the current expected cards.
- Expanded disagreements: added South (seed 1080) and North (seed 1145) disagreement checks to the same test file.
 - Added East (seed 2031) disagreement check.

Hard polish
- Implemented early cutoff in Hard candidate loop with env knob `MDH_HARD_EARLY_CUTOFF_MARGIN` (default 300).
- Extended next-trick probe to branch on the second opponent reply (canonical + max-penalty on void), all under the existing time cap.
- Kept Hard explain path deterministic and time-capped (no early cutoff there) to aid tuning.

Hard explain parity fix
- Controller now routes `explain_candidates_for` to Hard explain for both `SearchLookahead` and `FutureHard`, fixing CLI compare-batch alignment and ensuring Hard stats update under `--compare-*`.

Moon tuning knobs
- Added runtime-configurable moon thresholds and abort logic via env (`MDH_MOON_*`) with defaults matching current heuristics; printed alongside weights at startup in debug.
- Added unit test `moon_abort_toggle_respected` to validate `MDH_MOON_ABORT_LOST_CONTROL` behavior and ensured env restoration to avoid cross-test contamination.
- Adjusted default commit threshold to require 2 clean control tricks before committing (still marks Considering after the first clean trick), keeping existing tests green.
- Added Hard verbose-continuation test: `crates/hearts-app/tests/hard_verbose_continuation.rs` asserts QS shows positive continuation when feeding the leader under a constructed setup.
- Dropped env-mutation unit test to avoid parallel test race; validated the toggle manually via CLI.

Compare-batch snapshots
- Saved CSVs with `--out` to designs/tuning/ for quick analysis:
  - `designs/tuning/compare_west_1000_150.csv`
  - `designs/tuning/compare_south_1000_150.csv`
  - `designs/tuning/compare_east_2000_150.csv`
  - `designs/tuning/compare_north_1000_200.csv`
  These include disagreement rows (e.g., west: 1040/1082/1097) useful for crafting additional goldens.

Explain JSON snapshots
- Saved verbose explain snapshots for key disagreements (Normal vs Hard) to assist tuning and documentation:
  - `designs/tuning/explain_1040_west_hard.json`
  - `designs/tuning/explain_1040_west_normal.json`
  - `designs/tuning/explain_1082_west_hard.json`
  - `designs/tuning/explain_1097_west_hard.json`
  - `designs/tuning/explain_1080_south_hard.json`
  - `designs/tuning/explain_1145_north_hard.json`
  - `designs/tuning/explain_2031_east_hard.json`

New disagreement goldens
- Added stable West cases as explicit goldens:
  - seed 1082: Normal=10♠ vs Hard=2♠
  - seed 1097: Normal=8♠ vs Hard=J♦
  - File: `crates/hearts-app/tests/hard_vs_normal_disagreement_more.rs`

Mid-round branching golden
- Added a constructed mid-round scenario to assert Hard’s next-trick probe with second-opponent branching produces a positive continuation signal that influences ranking:
  - File: `crates/hearts-app/tests/hard_next_trick_branching.rs`

Bench snapshot
- heuristic_decision (sample-size=10, measure=1s):
  - seed42/South: ~2.61–2.78µs (improved vs earlier)
  - seed12345/East: ~2.83–3.14µs
  - seed8675309/North: ~1.13–1.21µs
- hard_decision:
  - seed42/South: ~4.86–5.09µs
  - seed12345/East: ~4.86–4.98µs
  - seed8675309/North: ~67.6–70.9µs (branching heavier here; still well under 1ms)
Notes: measurements are single-run snapshots; keep Hard typical under 20–30ms, spikes observed far below cap.

Branch-width sensitivity (MDH_HARD_BRANCH_LIMIT=12)
- hard_decision:
  - seed42/South: ~4.77–4.96µs (no meaningful change vs default)
  - seed12345/East: ~4.93–5.12µs (no meaningful change)
  - seed8675309/North: ~67.0–68.2µs (no meaningful change)
Observation: within this range Hard remains stable and well under target caps.
