# mdhearts Code Review - October 15, 2025

## Scope & Method
- Reviewed Rust crates `hearts-core`, `hearts-ui`, and `hearts-app`, focusing on gameplay flow, CLI tooling, AI controller, RL environment, and Windows integration.
- Evaluated Python RL tooling (`python/hearts_rl`), orchestration scripts, dataset loaders, and trainers.
- Spot-checked documentation (`docs/`, top-level READMEs) and build metadata to verify alignment with implementation.

## Critical Findings

1. **Self-play reward export labels all steps with post-hoc data (Critical)**  
   `crates/hearts-app/src/cli.rs:967-1041` batches every action into `pending_experiences`, then assigns rewards after the round finishes. At that point `match_state` reflects the end of the game, so:
   - `compute_step_reward` reads final trick counts, yielding zero for shaped/per-trick modes regardless of the actual step.
   - `prev_hand_size` and `prev_tricks` captured earlier no longer match live state, so even a corrected reward routine would mis-compute deltas.  
   Impact: RL datasets contain only duplicated terminal rewards, making PPO/BC fine-tuning ineffective and hiding regressions.  
   Recommendation: Emit rewards immediately after each action (or snapshot per-seat penalty totals) while the state still reflects that move. Add regression tests that assert shaped/per-trick modes produce non-zero feedback during a scripted round.

2. **Reward shaping helper never fires (Critical)**  
   `crates/hearts-app/src/rl/rewards.rs:92-122` inspects `round.current_trick()` to penalize winning a point-bearing trick. `RoundState::complete_trick` (`crates/hearts-core/src/model/round.rs:125-138`) swaps in a fresh `Trick`, so `trick.is_complete()` is always false by the time rewards are computed. Shaped mode therefore returns zero even if the export bug above is fixed.  
   Recommendation: Fetch penalties from the most recent entry in `trick_history()` or surface the completed trick before it is replaced. Add a unit test that covers capturing the Queen of Spades and validates a negative shaped reward.

## High Findings

1. **RL environment attributes terminal reward to the wrong seat (High)**  
   In `crates/hearts-app/src/rl/env.rs:231-244`, `step_play` sets `self.current_seat = winner` before assembling the `Step` result. When the final trick completes, the terminal reward now appears under the next leader instead of the actor that just moved. Advantage computation and training loops that assume seat stability will mislabel data.  
   Recommendation: Capture the acting seat before mutating `self.current_seat`; use that seat for rewards and done flags. Add coverage that simulates a hand and asserts each seat’s terminal reward matches its penalty total.

2. **CLI commands invoke modal Windows message boxes (High)**  
   `crates/hearts-app/src/cli.rs:258-305` calls `show_info_box`/`show_error_box` for success paths (`--export-snapshot`, `--import-snapshot`, help). These raise blocking dialogs that halt automation (CI, orchestrator scripts).  
   Recommendation: Gate message boxes behind an opt-in flag (default off) or confine them to GUI launches. Maintain stdout/stderr output for scripting and add a smoke test ensuring snapshot commands exit headlessly.

## Medium Findings

1. **Per-trick reward double counts history**  
   `crates/hearts-app/src/rl/rewards.rs:71-110` uses cumulative penalties when a trick is completed. Even with per-step state captured correctly, earlier penalties will be re-applied on subsequent tricks. Track per-seat deltas (e.g., store the last known penalty total) before subtracting.
2. **Status text contains mojibake separators**  
   `crates/hearts-app/src/controller.rs:140` renders `Round N ??? Passing: ...` because the literal uses a non-ASCII glyph. Replace with ASCII (for example `" | "`) so HUD text and logs remain legible.
3. **Python orchestrator emits garbled characters**  
   `python/hearts_rl/orchestrator.py:99,314` prints strings with replacement characters (`�`, `?`). The same data leaks into `python/README.md`’s tree diagram. Rewrite these literals to plain ASCII so console logs and docs stay readable.

## Low Observations

- `GameController::legal_moves` (`crates/hearts-app/src/controller.rs:152-160`) clones `RoundState` per candidate card; acceptable at 13 cards but worth profiling once RL/debug tooling drives tighter loops.
- Asset manifest fallback (`crates/hearts-ui/src/resource.rs:24-54`) logs errors via `eprintln!` but offers no programmatic signal. Consider surfacing the failure to the caller/UI if theme customization becomes important.

## Testing & Coverage Gaps

- No automated test exercises the CLI self-play export path; add a headless test that runs `mdhearts eval 1 --self-play --collect-rl temp.jsonl --reward-mode shaped` and verifies non-zero shaped rewards once fixes land.
- RL environment tests (`crates/hearts-app/src/rl/env.rs:300-372`) focus on invariants but do not validate reward semantics. Extend them to check per-seat rewards against known penalty tables.
- Python PPO trainer lacks an integration test that loads a tiny dataset and confirms the loss decreases/advantages vary. A deterministic fixture would catch future reward regressions quickly.

## Tooling & Workflow Notes

- `docs/CLI_TOOLS.md` currently promises message boxes for snapshot commands; update after gating dialogs so docs match behavior.
- `python/README.md`’s project-structure section needs regeneration without Unicode artifacts (use `tree /f` or a manual list).
- Consider adding a pre-commit hook or CI check for stray non-ASCII characters in source/docs to avoid recurring mojibake.

## Suggested Next Steps

1. Repair reward capture: restructure `run_self_play_eval` to emit per-step rewards in-place, adjust `RewardComputer` to use deltas, and add regression tests (Rust + Python) to prove shaped/per-trick feedback is non-zero and stable.
2. Fix `HeartsEnv` seat bookkeeping and add a property test that validates terminal rewards per seat on a seeded game.
3. Gate/remove CLI message boxes, align documentation, and introduce a headless CLI test to prevent regressions.
4. Clean up mojibake in controller status text, orchestrator logs, and README; enforce ASCII-only output.
5. After the above, regenerate a fresh RL dataset and run PPO/BC training to confirm loss curves behave sensibly (non-zero advantage variance, meaningful policy updates).
