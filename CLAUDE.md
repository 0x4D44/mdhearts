# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**mdhearts** is a Rust reimplementation of Microsoft Hearts with sophisticated AI (4 difficulty levels), Win32/Direct2D UI, and extensive CLI tools for AI evaluation. Uses a 3-crate Cargo workspace architecture.

## Build and Test Commands

```bash
# Build
cargo build --all --release

# Run all tests
cargo test --all --verbose

# Run single test
cargo test -p hearts-app test_name

# Format and lint (must pass before push)
cargo fmt --all
cargo clippy --workspace -- -D warnings

# Run the game
cargo run -p hearts-app --bin mdhearts

# Benchmarks
cargo bench -p hearts-app --bench heuristic_decision  # Normal AI
cargo bench -p hearts-app --bench hard_decision       # Hard AI
```

## Architecture

### Workspace Crates

1. **hearts-core** (`crates/hearts-core/`) - Pure game logic
   - Deterministic Hearts rules, no platform dependencies
   - Types: `Card`, `Rank`, `Suit`, `Hand`, `Trick`, `RoundState`, `ScoreBoard`
   - All game logic must remain deterministic and testable

2. **hearts-ui** (`crates/hearts-ui/`) - Presentation layer
   - View models, theme metadata, resource definitions
   - No direct Win32/WinUI API calls

3. **hearts-app** (`crates/hearts-app/`) - Platform + AI + CLI
   - Win32/Direct2D UI in `platform/` module
   - AI implementation in `bot/` module
   - CLI tools for evaluation and tuning
   - All `unsafe` code isolated in `platform/` with documented invariants

### AI System

The bot system (`crates/hearts-app/src/bot/`) has 4 difficulty levels:

| Difficulty | Module | Strategy | Decision Time |
|------------|--------|----------|---------------|
| Easy | `mod.rs` | Legacy heuristics | <1μs |
| Normal | `play.rs` | Feature-based heuristic | 5-50μs |
| Hard | `search.rs` | Shallow search + continuation | 2-15ms |
| Search | `search_deep.rs` + `endgame.rs` | Deep alpha-beta + perfect endgame | 0.5-2s |

Key bot modules:
- **`tracker.rs`**: Tracks unseen cards, void inference, belief-state sampling
- **`pass.rs`**: Card passing logic for all difficulties
- **`adviser.rs`**: Optional external bias system

## Key Invariants

1. **Determinism**: Core game logic must be deterministic given the same seed
2. **Separation**: Keep `unsafe` and Win32 APIs isolated in `hearts-app::platform`
3. **No warnings**: All code must compile cleanly with `-D warnings`
4. **Testing**: AI logic changes require unit tests or documented eval validation
5. **Feature flags**: New Hard AI features should be behind flags on main branch

## CLI Tools (Quick Reference)

```bash
# Export/import game state snapshots
mdhearts --export-snapshot snapshots/test.json [seed] [seat]
mdhearts --import-snapshot snapshots/test.json

# Explain AI decisions
mdhearts --explain-once <seed> <seat> [difficulty]
mdhearts --explain-snapshot <path> <seat>

# Compare Normal vs Hard
mdhearts --compare-batch <seat> <seed_start> <count> --out results.csv

# Head-to-head evaluation
mdhearts --match-batch <seat> <seed_start> <count> [diffA] [diffB] --out results.csv
mdhearts --match-mixed <seat> <seed_start> <count> <mix> --out results.csv  # mix: nnhh, hhnn, etc.

# Show current AI weights
mdhearts --show-weights
```

**For complete CLI reference**: See `docs/CLI_TOOLS.md` (60+ env vars, all flags documented)

## Evaluation Scripts

```bash
# Full deterministic evaluation
powershell -ExecutionPolicy Bypass -File tools/run_eval.ps1 -Verbose
bash tools/run_eval.sh

# Ultra-fast CI smoke test
bash tools/smoke_fast.sh 100
```

Outputs go to `designs/tuning/`. Use `--hard-deterministic --hard-steps <n>` for reproducible results.

## Development Workflow

### Adding/Modifying AI Features
1. Implement logic in appropriate bot module
2. Add env var configuration with defaults if needed
3. Write unit tests for the logic
4. Create snapshot-based regression test if behavior-critical
5. Run `--compare-batch` to validate no unintended regressions
6. For Hard AI: use feature flags (`MDH_FEATURE_HARD_STAGE1`, etc.) on main

### Creating Regression Tests from Bugs
1. Export snapshot: `mdhearts --export-snapshot snapshots/bug_name.json <seed> <seat>`
2. Use `--explain-snapshot` to analyze the decision
3. Write test that loads snapshot and asserts correct behavior
4. Fix the bug, verify test passes

### Debugging AI Decisions
```bash
# See detailed decision breakdown
MDH_DEBUG_LOGS=1 mdhearts --explain-once <seed> <seat> hard --hard-verbose

# Export full candidate analysis to JSON
mdhearts --explain-json <seed> <seat> output.json hard
```

## Key Directories

- `assets/`: Card atlas, layout JSON, adviser bias files
- `designs/`: Design docs and tuning reports (prefix: `YYYY.MM.DD - `)
- `docs/`: Setup guides, CLI reference, contributing guides
- `tools/`: Evaluation scripts (PowerShell and Bash)
- `snapshots/`: Game state snapshots for testing
- `wrk_docs/`: Component architecture documentation

## Configuration

### Core Environment Variables
```bash
MDH_BOT_DIFFICULTY=normal          # easy | normal | hard | search
MDH_DEBUG_LOGS=1                   # Enable detailed AI logging
MDH_HARD_DETERMINISTIC=1           # Use step budget instead of wall-clock
MDH_HARD_TEST_STEPS=120            # Step budget when deterministic
```

### Feature Flags (Continue-on-Main)
```bash
MDH_FEATURE_HARD_STAGE12=1         # Enable all Stage 1/2 Hard logic
MDH_FEATURE_HARD_STAGE1=1          # Planner nudges + guards only
MDH_FEATURE_HARD_STAGE2=1          # Moon/round-gap follow-ups only
```

## Documentation

- **[ARCHITECTURE.md](ARCHITECTURE.md)**: Comprehensive technical overview
- **[README.md](README.md)**: User-facing quick start
- **docs/CLI_TOOLS.md**: Complete CLI and env var reference
- **docs/CONTRIBUTING_AI_TUNING.md**: AI tuning methodology
- **wrk_docs/**: Component architecture docs (see index file)
