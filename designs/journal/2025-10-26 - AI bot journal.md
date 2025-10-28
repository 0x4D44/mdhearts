# AI Bot Journal
## 2025-10-26
- Investigated --export-endgame; current JSON omits trick history, scores, and tracker hints, causing difficulty reproducing default-weight DP flips.
- Authored designs/2025.10.26 - Endgame Snapshot Enhancements.md outlining schema upgrades (current trick plays, completed tricks, scores, optional tracker state) plus loader/helper plan.
- Next: implement richer export + loader, then reattempt enabling a default-weight DP flip golden; follow with deterministic mixed-seat smoke.
- Captured default-weight DP flip (seed 2263/W) using new export/rehydrate helper and enabled golden: crates/hearts-app/tests/hard_endgame_dp_golden.rs:1.
- Ran deterministic mixed-seat smoke (NNHH, 50 seeds) to confirm telemetry output; CSV: designs/tuning/mixed_smoke_2025-10-26.csv.
- Larger deterministic mixed-seat runs (n=1000) with default weights recorded: designs/tuning/mixed_west_1000_1000_nnhh_dp_default.csv and designs/tuning/mixed_west_2000_1000_hhnn_dp_default.csv (means still near parity).
- Added controller-based DP flip goldens for seed 2263 (West) and 2325 (South) using run_flip_assert; no default flips yet for North/East in 2000..2399 range.
- Mixed-seat deterministic evals (n=1000) for north/east/south recorded under designs/tuning/mixed_{seat}_1000_1000_nnhh_dp_default.csv; mean penalties remain ~6-7 vs Normal, still showing negligible Hard advantage.
- Extended deterministic mixed-seat runs to mixes HHNN/HNNH/NHNH for all seats (means: West 6.367, North 6.969, East 6.19, South 6.474); still near parity vs Normal.
- Found default-weight flip for 1383 East via controller replay; added to hard_endgame_dp_golden.rs with env-guarded mutex.
- Computed 95% confidence intervals for mixed-seat runs: West 6.37 +/- 0.40, North 6.97 +/- 0.43, East 6.19 +/- 0.39, South 6.47 +/- 0.39 penalties; still overlapping Normal.
- North flip hunt: seeds 800-1599 & 2000-2399 show Hard vs Normal disagreements (see compare_north_* CSVs) but DP toggle leaves Hard choice unchanged; no DP-specific north golden yet. Exported endgame_1373_north.json for targeted tuning later.
- Tweak trial (cont_feed=90/cont_self=110) increased mean penalties (west n=200) to 6.66; reverted to defaults.
- Implemented adviser bias scaffolding for Hard planner: added bot::adviser module, wired optional bias application into Hard candidate scoring, added default bias JSON at assets/adviser/play.json, dataset export test verifies bias fields, and CLI docs plus README now document --export-play-dataset and adviser toggles.
- Stage 1: implemented leader-feed planner nudge with unique-leader/hearts-broken/near-100 guards, tracked `planner_nudge_hits` in Hard stats/CLI, added `hard_wide_tier_feed_nudge` regression tests and expanded docs/CSV outputs for the new metrics.
