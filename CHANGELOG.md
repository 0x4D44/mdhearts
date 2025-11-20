Changelog

All notable changes to this project will be documented here.
2025-11-18 - Core Rules & AI Reliability
- **Rules**: First-trick guard now lets a club-void follower shed hearts/QS only when no safe cards remain; regression tests cover both branches.
- **Tracker**: Passing no longer removes cards from `UnseenTracker`, keeping unseen counts accurate until cards are actually played.
- **Telemetry**: `TelemetrySink::push` enforces retention caps immediately, preventing unbounded memory use; added retention unit test.
- **Controller/Core**: Added `RoundState::legal_cards`/`can_play_card` helpers and wired `GameController::legal_moves` to them, eliminating per-card round clones.
- **Tests**: Reactivated Stage1 guard tests (`hard_guard_round_leader_saturated_blocks_feed`, `hard_flat_scores_uses_round_leader_penalties_gt0`) so planner nudges stay covered in CI.

2025-11-16 — Critical Bug Fixes
- **Search Deep**: Fixed recursion bug in search_opponent (was evaluating instead of recursing)
- **Search Deep**: Replaced unsafe alpha-beta bounds (i32::MIN/MAX) with safe bounds (-100k/+100k)
- **Search Deep**: Added saturating arithmetic for aspiration windows to prevent overflow
- **Search Deep**: Replaced panic with graceful error for empty legal moves
- **Search Deep**: Added player-to-move to position hash for correct transposition table lookups
- **Endgame Solver**: Fixed penalty tracking (was computing but not accumulating penalties)
- **Endgame Solver**: Corrected minimax perspective logic (now properly determines next player)
- **Endgame Solver**: Added perspective to memoization keys (position + seat)
- **Endgame Solver**: Added timeout mechanism to prevent long-running solves
- **Endgame Solver**: Integrated belief-state sampling for imperfect information
- **Endgame Solver**: Added moon shooting detection and scoring
- **Win32 Platform**: Fixed memory leak by adding WM_NCDESTROY handler to deallocate AppState

Phase 3 (2025-11) — Endgame Perfect Play
- New module `bot/endgame.rs`: Minimax solver with memoization for endgame positions
- Solves positions with ≤13 cards per player (configurable, default 7 for Hard, 13 for Search)
- Perfect play guarantee when all cards known or sampled from belief state
- Integrated into SearchLookahead difficulty for ultra-strong endgame
- Environment variable `MDH_ENDGAME_SOLVER_ENABLED` (default: enabled)
- Environment variable `MDH_ENDGAME_MAX_CARDS` to control activation threshold
- Environment variable `MDH_ENDGAME_USE_SAMPLING` for belief-state integration (default: enabled)

Phase 2 (2025-11) — Deep Search with Alpha-Beta Pruning
- New module `bot/search_deep.rs`: Multi-ply search with alpha-beta pruning
- Transposition table (10M positions for Search difficulty)
- Iterative deepening up to 10 plies (SearchLookahead difficulty)
- Killer move heuristic for move ordering
- Aspiration windows for faster search
- Time-based search budgets (default 2000ms for Search difficulty)
- Belief-state sampling integration (100 samples for Search)
- Environment variable `MDH_DEEP_SEARCH_MAX_DEPTH` (default: 10)
- Environment variable `MDH_DEEP_SEARCH_TT_SIZE_MB` (default: 10)
- Environment variable `MDH_DEEP_SEARCH_TIME_MS` (default: 2000)

Phase 1 (2025-11) — Belief-State Sampling for Imperfect Information
- Enhanced `UnseenTracker` with `sample_world()` for opponent hand sampling
- Integrated belief-state sampling into Hard and Search difficulties
- Search difficulty uses 100 belief samples for robust evaluation
- Samples respect known voids and card constraints
- Foundation for proper imperfect information handling in deep search

2025-10-22 — AI Improvements, Evaluation, and Handoff
- Heuristic (Normal) planner: endgame polish retained; added non‑QS hearts‑feed golden.
- Hard (FutureHard): deterministic budget, top‑K continuation, tiny continuation signals (env‑tunable) and verbose explain.
- New tests: endgame feed cap, non‑QS hearts feed, moon transition smoke, near‑tie constructed cases.
- Tools: cross‑platform deterministic evaluation helpers (`tools/run_eval.ps1`, `tools/run_eval.sh`).
- Docs: contributor tuning guide, CLI tools polish, artifacts index, Stage 6/7 plans, handoff guide.
- CI: builds/tests on Windows/Linux; PR smoke runs eval helper with tiny ranges.

Notes:
- Env‑gated toggles (tie‑break, probe/pruning) remain off by default to preserve goldens.
- Use the evaluation helpers to reproduce CSVs/summary under `designs/tuning/`.

