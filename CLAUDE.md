# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**mdhearts** is a modern Rust reimplementation of Microsoft Hearts with a sophisticated AI system featuring multiple difficulty levels (Easy, Normal, Hard). The project includes a Win32 Direct2D UI, comprehensive CLI tools for AI tuning and evaluation, and extensive testing infrastructure.

## Build and Test Commands

### Basic Commands
```bash
# Build the project
cargo build --all --release

# Run all tests
cargo test --all --verbose

# Run the main application
cargo run -p hearts-app --bin mdhearts

# Format and lint (must be clean before push)
cargo fmt --all
cargo clippy --workspace -- -D warnings
```

### Benchmarks
```bash
# Benchmark Normal heuristic AI
cargo bench -p hearts-app --bench heuristic_decision

# Benchmark Hard AI
cargo bench -p hearts-app --bench hard_decision
```

### Running Tests
- Core engine tests: `cargo test -p hearts-core`
- App/bot tests: `cargo test -p hearts-app`
- Single test: `cargo test -p hearts-app test_name`

## Architecture

### Workspace Structure
The project uses a Cargo workspace with three crates:

1. **hearts-core** (`crates/hearts-core/`)
   - Deterministic game rules and core logic
   - No platform dependencies or Win32 APIs
   - Pure Rust models: `Card`, `Rank`, `Suit`, `Hand`, `Trick`, `RoundState`, `ScoreBoard`
   - Game state serialization/deserialization for snapshots
   - All game logic must remain deterministic and testable

2. **hearts-ui** (`crates/hearts-ui/`)
   - View models and presentation abstractions
   - Theme metadata and resource definitions
   - No direct Win32/WinUI API calls
   - Bridges between core game state and platform rendering

3. **hearts-app** (`crates/hearts-app/`)
   - Platform entry point with Win32 Direct2D UI (`platform/win32.rs`)
   - AI implementation (`bot/` module)
   - CLI tools for evaluation and tuning
   - Windows-specific interop via `windows` crate
   - All `unsafe` code must be isolated in `platform/` modules with documented invariants

### AI System Architecture

The bot system (`crates/hearts-app/src/bot/`) consists of:

- **`mod.rs`**: Core bot types, difficulty levels (`BotDifficulty`), bot context, and style determination (Cautious, AggressiveMoon, HuntLeader)
- **`play.rs`**: `PlayPlanner` - Normal difficulty heuristic planner with feature-based scoring
- **`search.rs`**: `PlayPlannerHard` - Hard difficulty with shallow search, continuation scoring, and configurable branch limits
- **`pass.rs`**: `PassPlanner` - Card passing logic for all difficulties
- **`tracker.rs`**: `UnseenTracker` - Tracks unseen cards and moon-shooting state
- **`adviser.rs`**: Optional bias system for Hard AI candidate scoring

#### Difficulty Levels
- **Easy** (`BotDifficulty::EasyLegacy`): Legacy simple heuristics (<1μs decisions)
- **Normal** (`BotDifficulty::NormalHeuristic`): Feature-based heuristic planner (5-50μs, default)
- **Hard** (`BotDifficulty::FutureHard`): Shallow search with continuation evaluation (2-15ms deterministic)
- **Search** (`BotDifficulty::SearchLookahead`): Time-capped deeper search, extends Hard with configurable think limits

**Note**: Search difficulty uses the same engine as Hard but with time-based budgets instead of step limits, enabling analysis of think-time vs. strength tradeoffs.

#### Feature Flags (Continue-On-Main Strategy)
To allow ongoing Hard AI development while keeping default behavior stable:
- `MDH_FEATURE_HARD_STAGE12=1` or `--hard-stage12`: Enable all Stage 1/2 Hard logic
- `MDH_FEATURE_HARD_STAGE1=1` or `--hard-stage1`: Enable planner nudges and guards only
- `MDH_FEATURE_HARD_STAGE2=1` or `--hard-stage2`: Enable moon/round-gap follow-ups only
- Default: All flags OFF; CI explicitly enables them for testing

## CLI Tools

The `mdhearts` binary provides extensive CLI commands for AI evaluation:

### Snapshot Management
```bash
# Export a deterministic game state
mdhearts --export-snapshot snapshots/test.json [seed] [seat]

# Import and inspect a snapshot
mdhearts --import-snapshot snapshots/test.json
```

### AI Explanation
```bash
# Explain a single decision
mdhearts --explain-once <seed> <seat> [difficulty]

# Explain across multiple seeds
mdhearts --explain-batch <seat> <seed_start> <count> [difficulty]

# Explain from a snapshot
mdhearts --explain-snapshot <path> <seat>

# Export detailed JSON with candidate scores
mdhearts --explain-json <seed> <seat> <path> [difficulty]
```

### AI Comparison
```bash
# Compare Normal vs Hard for one seed
mdhearts --compare-once <seed> <seat>

# Compare across seeds, output CSV
mdhearts --compare-batch <seat> <seed_start> <count> --out results.csv

# Only show disagreements
mdhearts --compare-batch <seat> <seed_start> <count> --only-disagree
```

### Head-to-Head Evaluation
```bash
# Match two difficulties across seeds
mdhearts --match-batch <seat> <seed_start> <count> [diffA] [diffB] --out results.csv

# Example: Normal vs Hard with deterministic Hard AI
mdhearts --match-batch west 1000 50 normal hard \
  --hard-deterministic --hard-steps 120 \
  --out designs/tuning/match_west.csv

# Mixed-seat evaluation (inline seed ranges)
# <mix> is 4 chars (N,E,S,W) using e|n|h|s for Easy/Normal/Hard/Search
mdhearts --match-mixed <seat> <seed_start> <count> <mix> --out results.csv --stats

# Mixed-seat with seed file
mdhearts --match-mixed-file <seat> <mix> --seeds-file seeds.txt --out results.csv
```

### Performance & Research
```bash
# Quick performance benchmarking
mdhearts --bench-check <difficulty> <seat> <seed_start> <count>

# Export play dataset for research (NDJSON)
mdhearts --export-play-dataset <seat> <seed_start> <count> <difficulty> <out>

# Show Hard AI telemetry
mdhearts --show-hard-telemetry --out telemetry.ndjson
```

### Card Passing Analysis
```bash
# Explain card passing for one seed
mdhearts --explain-pass-once <seed> <seat>

# Batch analyze passing decisions
mdhearts --explain-pass-batch <seat> <seed_start> <count>
```

### Hard AI Flags
Append these to control Hard AI behavior without env vars:
- `--hard-deterministic`: Use step budget instead of wall-clock time
- `--hard-steps <n>`: Step cap for deterministic mode
- `--hard-branch-limit <n>`: Number of top candidates to probe
- `--hard-next-branch-limit <n>`: Next-trick lead candidates to probe
- `--hard-time-cap-ms <ms>`: Wall-clock cap per decision
- `--hard-cutoff <margin>`: Early cutoff margin
- `--hard-phaseb-topk <k>`: Compute continuation only for top-K candidates
- `--hard-cont-boost-gap <n>`: Apply continuation boost within this gap
- `--hard-cont-boost-factor <n>`: Multiplicative boost to continuation
- `--hard-verbose`: Print continuation breakdown (with `MDH_DEBUG_LOGS=1`)

**Additional Hard AI Environment Variables** (see `docs/CLI_TOOLS.md` for complete list):
- Tiering: `MDH_HARD_TIERS_ENABLE=1`, `MDH_HARD_LEVERAGE_THRESH_NARROW`, `MDH_HARD_LEVERAGE_THRESH_NORMAL`
- Belief cache: `MDH_HARD_BELIEF_CACHE_SIZE=128`
- Next-trick probing: `MDH_HARD_NEXT3_ENABLE=1`, `MDH_HARD_PROBE_AB_MARGIN`
- Adviser bias: `MDH_HARD_ADVISER_PLAY=1`
- Promoted defaults: `MDH_HARD_PROMOTE_DEFAULTS=1`

### Evaluation Scripts
```bash
# PowerShell: Run full deterministic evaluation
powershell -ExecutionPolicy Bypass -File tools/run_eval.ps1 -Verbose

# Bash: Same evaluation for Linux/macOS
bash tools/run_eval.sh

# Ultra-fast smoke test (1 seed per seat, for CI)
bash tools/smoke_fast.sh 100

# Search vs Hard sweeps with telemetry verification
powershell -ExecutionPolicy Bypass -File tools/run_search_vs_hard.ps1 -VerifyTimeoutTelemetry

# Mixed-seat sweeps (e.g., shsh, sshh)
powershell -ExecutionPolicy Bypass -File tools/run_search_vs_mixed.ps1 -Mixes shsh -ThinkLimitsMs @(5000,0)
```

**Output Locations:**
- Main eval results: `designs/tuning/`
- Smoke test archives: `designs/tuning/stage1/smoke_release/`
- Summary reports: `designs/tuning/eval_summary_<timestamp>.md`

## Configuration via Environment Variables

### Core Settings
- `MDH_BOT_DIFFICULTY`: `easy`, `normal`, `hard`, `search` (default: `normal`)
- `MDH_DEBUG_LOGS=1`: Enable detailed decision logging to stderr
- `MDH_CLI_POPUPS=1`: Enable Windows message boxes (default: off for automation)

### Hard AI Configuration
- `MDH_HARD_DETERMINISTIC=1`: Enable deterministic step-capped search
- `MDH_HARD_TEST_STEPS=<n>`: Step budget when deterministic
- `MDH_HARD_BRANCH_LIMIT=<n>`: Candidate probe limit (default: 6)
- `MDH_HARD_TIME_CAP_MS=<n>`: Wall-clock timeout per decision (default: 10ms)
- `MDH_HARD_NEXT_BRANCH_LIMIT=<n>`: Next-trick probe branches (default: 3)
- `MDH_HARD_EARLY_CUTOFF_MARGIN=<n>`: Early pruning margin (default: 300)

### AI Tuning Weights
Over 20+ tunable weights for Normal and Hard AI behavior, including:
- `MDH_W_OFFSUIT_BONUS`: Bonus for dumping off-suit penalties (default: 600)
- `MDH_W_NEAR100_SELF_CAPTURE_BASE`: Penalty for self-capture near 100 (default: 1300)
- `MDH_W_HUNT_FEED_PERPEN`: Bonus per penalty fed to leader when hunting (default: 800)
- See `README.md` "AI tuning (env weights)" section for full list

### Moon Shooting Configuration
- `MDH_MOON_COMMIT_MAX_CARDS`: Max cards played to consider moon (default: 20)
- `MDH_MOON_COMMIT_MAX_SCORE`: Max score to consider moon (default: 70)
- `MDH_MOON_COMMIT_MIN_TRICKS`: Min tricks won before commit (default: 2)
- `MDH_MOON_COMMIT_MIN_HEARTS`: Min hearts needed (default: 5)
- `MDH_MOON_ABORT_OTHERS_HEARTS`: Abort threshold (default: 3)

## Development Guidelines

### Coding Standards
- **Rust Edition**: 2024 (requires Rust 1.81+)
- **No warnings**: Code must compile with `#![deny(warnings)]`
- **Safety**: Keep `unsafe` isolated in `hearts-app::platform` modules with invariant docs
- **Testing**: Core engine changes require unit tests in `hearts-core`
- **Error handling**: Use `Result<T>` with descriptive variants, no `unwrap()` in production
- **Documentation**: Public structs/enums must have doc comments

### Code Review Checklist
- API docs updated for public items
- No `unwrap()` in production code paths
- Clippy and fmt pass cleanly
- Tests added for new logic
- Windows-specific code isolated in `platform/` modules

### Git Workflow
- CI runs on `main` branch and all PRs
- CI enforces: build, test, clippy, fmt, and eval smokes on PRs
- PR template: `.github/PULL_REQUEST_TEMPLATE.md`
- For AI changes, include eval results and threshold validation

### Testing Philosophy
- Core game logic: deterministic unit tests in `hearts-core`
- AI behavior: snapshot-based regression tests and seed-based evaluation
- Use `--export-snapshot` to create reproducible test cases
- CI runs ultra-fast smoke tests on PRs (1-2 seeds per seat, aggressive limits)

## Key Directories

- `assets/`: Card atlas (`cards.png`), layout JSON, adviser bias files
- `designs/`: Design docs, tuning plans, eval reports (prefix with `YYYY.MM.DD - `)
- `designs/tuning/`: AI evaluation outputs (CSVs, summaries, smoke archives)
- `docs/`: Setup guides, CLI docs, contributing guides
- `tools/`: Evaluation scripts (PowerShell and Bash)
- `snapshots/`: Exported game state snapshots for testing
- `wrk_docs/`: Component architecture documentation (see index for navigation)
- `installers/`: Inno Setup script and release packaging

## Platform-Specific Notes

### Windows Build
- Requires Windows SDK (see `docs/SETUP_WINDOWS.md`)
- Uses Direct2D for rendering
- Toolchain: `x86_64-pc-windows-msvc`

### CI
- Runs on Ubuntu and Windows (GitHub Actions)
- PR smoke tests use deterministic Hard AI with aggressive limits
- See `.github/workflows/ci.yml` for job definitions

## Common Patterns

### Adding a New AI Feature
1. Implement deterministic logic in `bot/play.rs` or `bot/search.rs`
2. Add env var configuration with defaults
3. Add CLI flags if needed
4. Write unit tests for the logic
5. Create snapshot-based regression tests
6. Run `--compare-batch` to validate changes don't break existing behavior
7. Update relevant docs in `docs/` and design notes in `designs/`

### Tuning AI Parameters
1. Use `--show-weights` to see current active weights
2. Set env vars to override defaults
3. Run `--match-batch` or evaluation scripts to measure impact
4. Document findings in `designs/tuning/`
5. For reproducibility, use `--hard-deterministic --hard-steps <n>`

### Creating Evaluation Snapshots
1. Export: `mdhearts --export-snapshot snapshots/name.json <seed> <seat>`
2. Use in tests or explain commands
3. Check into repo if it's a regression test golden
4. Reference in test documentation

### Building Release Installer (Windows)
1. Build release binary: `cargo build --release`
2. Run Inno Setup: `iscc installers\Hearts.iss`
3. Output: `installers/MDHearts-1.0.1-Setup.exe`

## Important Invariants

1. **Determinism**: Core game logic must be deterministic given the same seed
2. **Separation**: Keep platform code (`unsafe`, Win32 APIs) isolated in `hearts-app::platform`
3. **No warnings**: All code must compile cleanly with `-D warnings`
4. **Testing**: Changes to AI logic require either unit tests or documented eval validation
5. **Feature flags**: New Hard AI features must be behind feature flags when on main branch

## Additional Documentation

- **[ARCHITECTURE.md](ARCHITECTURE.md)**: Comprehensive technical architecture overview
- **[README.md](README.md)**: User-facing quick start and reference
- **docs/CLI_TOOLS.md**: Complete CLI command reference (60+ env vars)
- **docs/CONTRIBUTING_AI_TUNING.md**: AI tuning methodology
- **wrk_docs/2025.11.06 - Architecture Documentation Index.md**: Entry point for component docs
