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

Hard polish
- Implemented early cutoff in Hard candidate loop with env knob `MDH_HARD_EARLY_CUTOFF_MARGIN` (default 300).
- Extended next-trick probe to branch on the second opponent reply (canonical + max-penalty on void), all under the existing time cap.
- Kept Hard explain path deterministic and time-capped (no early cutoff there) to aid tuning.
