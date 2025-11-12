# mdhearts

Modern Rust implementation of Microsoft Hearts featuring sophisticated multi-difficulty AI, native Win32/Direct2D UI, and comprehensive evaluation tools.

## Overview

**mdhearts** is a production-quality Hearts card game implementation built in Rust, showcasing:

- **🎮 Native Windows UI** - Hardware-accelerated Direct2D rendering with smooth animations
- **🤖 Sophisticated AI** - 4 difficulty levels (Easy, Normal, Hard, Search) with 60+ tunable parameters
- **⚙️ Deterministic Gameplay** - Same seed → identical game, enabling reproducible testing
- **📊 Comprehensive Tooling** - 20+ CLI commands for AI evaluation and research
- **🏗️ Clean Architecture** - 3-tier design separating game logic, presentation, and platform layers

**Key Metrics:**
- ~8,000 lines of Rust code across 3 workspace crates
- <15ms AI decisions (Hard difficulty, deterministic mode)
- 60 FPS rendering with animations
- 74 test files with extensive coverage
- 14 evaluation scripts for reproducible AI research

📚 **[Read the Architecture Documentation](ARCHITECTURE.md)** for a comprehensive technical overview.

**CI Status:** See GitHub Actions workflow in `.github/workflows/ci.yml` (builds/tests on Windows/Linux; PR eval smoke).

## Quick Start

### Prerequisites
1. Install Rust stable toolchain (`x86_64-pc-windows-msvc` for Windows)
2. Follow `docs/SETUP_WINDOWS.md` for Win32 build prerequisites
3. Card assets are in `assets/` directory (`cards.png` atlas + `cards.json` layout)

### Building and Running
```bash
# Build and run the game
cargo run -p hearts-app --bin mdhearts

# Build release version
cargo build --release

# Run tests
cargo test --all

# Run benchmarks
cargo bench -p hearts-app
```

## Architecture

mdhearts uses a clean 3-tier Cargo workspace architecture:

### Workspace Crates

**1. hearts-core** - Pure game logic
- Deterministic Hearts rules implementation
- ~2,500 lines of portable Rust (zero unsafe code)
- Key types: `Card`, `Hand`, `Trick`, `RoundState`, `ScoreBoard`
- Snapshot serialization for testing

**2. hearts-ui** - Presentation layer
- Asset and theme management
- Platform-agnostic abstractions
- ~160 lines (foundational, awaiting full integration)

**3. hearts-app** - Application layer
- Win32/Direct2D native UI (~5,700 lines)
- AI system (~3,700 lines)
- CLI evaluation tools
- Controller orchestrating all layers

### AI System

The bot system implements 4 difficulty levels:

| Difficulty | Strategy | Decision Time | Strength |
|------------|----------|---------------|----------|
| **Easy** | Legacy heuristics | <1 μs | Beginner |
| **Normal** (default) | Feature-based heuristic | 5-50 μs | Intermediate |
| **Hard** | Shallow search + continuation | 2-15 ms | Advanced |
| **Search** | Deep search (future) | TBD | Expert |

**Normal AI** uses ~15 weighted features with single-trick simulation. **Hard AI** uses shallow search with top-K candidate selection, continuation scoring, belief-guided opponent modeling, and optional perfect endgame solving.

📚 **For complete architecture details, see [ARCHITECTURE.md](ARCHITECTURE.md)**

Additional component documentation:
- hearts-core: `wrk_docs/2025.11.06 - Architecture - hearts-core crate.md`
- AI Bot System: `wrk_docs/2025.11.06 - Architecture - AI Bot System.md`
- Platform Layer: `wrk_docs/2025.11.06 - Architecture - Platform Layer.md`
- Controller: `wrk_docs/2025.11.06 - Architecture - Controller and Orchestration.md`
- CLI Tools: `wrk_docs/2025.11.06 - Architecture - CLI and Evaluation System.md`
- Build System: `wrk_docs/2025.11.06 - Architecture - Build System and Tooling.md`
- Documentation Index: `wrk_docs/2025.11.06 - Architecture Documentation Index.md`

## CLI Commands

### Snapshot Management
- `--export-snapshot <path> [seed] [seat]` - Export game state to JSON for reproducible testing
- `--import-snapshot <path>` - Load and inspect a saved game state

### AI Explanation & Analysis
- `--explain-once <seed> <seat> [difficulty]` - Analyze a single AI decision with candidate scores
- `--explain-batch <seat> <seed_start> <count> [difficulty]` - Batch analyze decisions across multiple seeds
- `--explain-snapshot <path> <seat>` - Analyze decision from a saved snapshot
- `--explain-json <seed> <seat> <path> [difficulty]` - Export detailed JSON with candidates, weights, and stats
- `--explain-pass-once <seed> <seat>` - Show the 3 cards selected for passing
- `--explain-pass-batch <seat> <seed_start> <count>` - Batch analyze card passing decisions

### AI Comparison & Evaluation
- `--compare-once <seed> <seat>` - Compare Normal vs Hard for single decision
- `--compare-batch <seat> <seed_start> <count> [--out <path>] [--only-disagree]` - Batch compare with CSV output
- `--match-batch <seat> <seed_start> <count> [diffA diffB] [--out <path>]` - Head-to-head simulation with penalty CSV
- `--match-mixed <seat> <seed_start> <count> <mix> [--out <path>] [--telemetry-out <path>] [--stats] [Hard flags]` - Mixed-seat evaluation with inline seed ranges. `<mix>` is a 4-char string (N,E,S,W) using `e|n|h|s` for Easy/Normal/Hard/Search seats.
- `--match-mixed-file <seat> <mix> --seeds-file <path> [--out <path>] [--telemetry-out <path>]` - Mixed-seat tournament evaluation using a saved seed list (same `e|n|h|s` mix syntax).

### Performance & Telemetry
- `--bench-check <difficulty> <seat> <seed_start> <count>` - Quick performance stats (avg and p95 µs)
- `--show-weights [--out <path>]` - Display active Normal/Hard AI weights
- `--show-hard-telemetry [--out <path>]` - Export Hard AI decision telemetry to NDJSON
- `--export-play-dataset <seat> <seed_start> <count> <difficulty> <out>` - Stream NDJSON for research

## Testing & Evaluation

### Quick Smoke Test
Ultra-fast 1-seed deterministic smoke test for CI validation:
```bash
# Linux/macOS
tools/smoke_fast.sh [seed_start]  # defaults to 100

# Windows (PowerShell)
cargo build -p hearts-app --bin mdhearts --release
$env:MDH_HARD_DETERMINISTIC = "1"
$env:MDH_HARD_TEST_STEPS = "60"
.\target\release\mdhearts --match-mixed west 100 1 nnhh `
  --hard-steps 60 --hard-branch-limit 150 --hard-next-branch-limit 80 `
  --hard-time-cap-ms 5 --stats --out tmp/smoke.csv
```

Outputs archived under `designs/tuning/stage1/smoke_release/`.

### Feature Flags (Continue-on-Main Strategy)
Allow ongoing Hard AI development while keeping default behavior stable:

```bash
# Enable all Stage 1/2 features
MDH_FEATURE_HARD_STAGE12=1        # or --hard-stage12

# Enable individually
MDH_FEATURE_HARD_STAGE1=1         # Planner nudges + guards
MDH_FEATURE_HARD_STAGE2=1         # Moon/round-gap follow-ups
```

**Default:** All flags OFF; CI explicitly enables them.

### Deterministic Evaluation
For reproducible AI evaluation across seed ranges:

```bash
# PowerShell
powershell -ExecutionPolicy Bypass -File tools/run_eval.ps1 -Verbose

# Bash
bash tools/run_eval.sh
```

**Outputs:** Timestamped CSVs under `designs/tuning/` + summary markdown.
**See also:** `docs/EVAL_WRAPPERS.md`, `docs/CI_EVAL.md`

Additional scripted sweeps:

- `tools/run_search_vs_hard.ps1` – runs Search vs Hard (mirrored seats) across multiple think limits, captures match/compare CSVs, and can force telemetry smokes via `-VerifyTimeoutTelemetry`.
- 	ools/run_search_vs_mixed.ps1 – automates mixed-seat batches (e.g., shsh, sshh) across think limits with optional seed files, smoke counts, -TelemetryOut for per-seat telemetry, and -TelemetrySmoke for timeout verification.

### Contributing
- PR template: `.github/PULL_REQUEST_TEMPLATE.md`
- PR validation: `docs/CLI_TOOLS_SMOKE.md` and CI ultra-smoke job
- AI tuning guide: `docs/CONTRIBUTING_AI_TUNING.md`

## Configuration

### Bot Difficulty
```bash
MDH_BOT_DIFFICULTY=normal  # easy | normal (default) | hard | search
```

### Hard AI Flags (CLI)
Control Hard AI behavior without environment variables:
- `--hard-deterministic` - Use step-based budget instead of wall-clock time
- `--hard-steps <n>` - Step budget for deterministic mode
- `--hard-branch-limit <n>` - Top-K candidates to evaluate (default: 6)
- `--hard-next-branch-limit <n>` - Next-trick leads to probe (default: 3)
- `--hard-time-cap-ms <ms>` - Wall-clock timeout per decision (default: 10)
- `--hard-cutoff <margin>` - Early cutoff margin (default: 300)
- `--hard-phaseb-topk <k>` - Compute continuation for top-K only
- `--hard-verbose` - Print continuation breakdown (with `MDH_DEBUG_LOGS=1`)

### Hard AI Environment Variables

**Search Control:**
```bash
MDH_HARD_DETERMINISTIC=1      # Use step-based budget
MDH_HARD_TEST_STEPS=120       # Step budget when deterministic
MDH_HARD_BRANCH_LIMIT=6       # Top-K candidates
MDH_HARD_TIME_CAP_MS=10       # Wall-clock timeout (ms)
```

**Continuation Scoring Weights (tiny adjustments):**
```bash
MDH_HARD_CONT_FEED_PERPEN=60          # Bonus per penalty to leader
MDH_HARD_CONT_SELF_CAPTURE_PERPEN=80  # Penalty for self-capture
MDH_HARD_NEXTTRICK_SINGLETON=25       # Singleton bonus
```

**Advanced:**
- `MDH_HARD_TIERS_ENABLE=1` - Leverage-based adaptive search depth
- `MDH_HARD_BELIEF_CACHE_SIZE=128` - Belief cache capacity
- `MDH_HARD_ADVISER_PLAY=1` - External bias injection
- See `docs/CLI_TOOLS.md` for complete list (30+ variables)

### Normal AI Tuning Weights
```bash
MDH_W_OFFSUIT_BONUS=600          # Bonus for void dumping
MDH_W_NEAR100_SELF_CAPTURE_BASE=1300  # Near-100 urgency
MDH_W_HUNT_FEED_PERPEN=800       # Leader feeding bonus
MDH_W_LEADER_FEED_BASE=120       # Base leader feed
```

See ARCHITECTURE.md for complete weight documentation (9+ normal, 30+ hard).

### Moon Shooting Parameters
```bash
MDH_MOON_COMMIT_MAX_SCORE=70   # Max score to attempt moon
MDH_MOON_COMMIT_MIN_HEARTS=5   # Min hearts needed
MDH_MOON_COMMIT_MIN_CONTROL=3  # Min high hearts (≥10)
MDH_MOON_ABORT_OTHERS_HEARTS=3 # Abort if opponents collect N hearts
```

### Debug & Logging
```bash
MDH_DEBUG_LOGS=1      # Detailed AI decision output
MDH_CLI_POPUPS=1      # Enable Windows message boxes
```

## Benchmarks & Performance

### Running Benchmarks
```bash
# Normal AI performance
cargo bench -p hearts-app --bench heuristic_decision

# Hard AI performance
cargo bench -p hearts-app --bench hard_decision
```

**Target Performance:**
- Normal AI: <100μs worst-case (typically 5-50μs)
- Hard AI: <30ms typical (2-15ms with 120 steps deterministic)

### Performance Characteristics
- Normal AI: ~20,000 decisions/second
- Hard AI: ~100-200 decisions/second (deterministic mode)
- Rendering: 60 FPS with animations
- Startup: <1 second cold start


## Packaging

### Building Release Installer
```bash
# 1. Build release binary
cargo build --release

# 2. Run Inno Setup compiler (Windows)
iscc installers\Hearts.iss
```

**Output:** `installers/MDHearts-1.0.1-Setup.exe` (~2-3 MB)

**Installer features:**
- 64-bit Windows executable
- Desktop shortcut (optional)
- Start menu entry
- Uninstaller
- LZMA2 ultra compression

## Documentation

### User Documentation
- **[ARCHITECTURE.md](ARCHITECTURE.md)** - Complete technical architecture overview
- **[README.md](README.md)** (this file) - Quick start and reference
- **docs/CLI_TOOLS.md** - Complete CLI command reference
- **docs/CONTRIBUTING_AI_TUNING.md** - AI tuning guide
- **docs/SETUP_WINDOWS.md** - Build prerequisites

### Component Documentation
Comprehensive architecture analysis in `wrk_docs/`:
- hearts-core crate (game engine)
- hearts-ui crate (presentation)
- AI Bot System (4 difficulty levels)
- Platform Layer (Win32/Direct2D)
- Controller & Orchestration
- CLI & Evaluation System
- Build System & Tooling
- Documentation Index (start here for detailed docs)

### Design Documentation
162 design documents tracking development decisions in `designs/`:
- Stage plans (Stage 1-7 for Hard AI development)
- Tuning reports and evaluation summaries
- Architecture decision records
- Feature specifications
- See `designs/INDEX.md` for complete index

## Release Notes

### Version 1.0.1
- Sophisticated multi-difficulty AI system (Easy, Normal, Hard)
- Win32/Direct2D native UI with hardware acceleration
- Comprehensive CLI evaluation tools (20+ commands)
- Deterministic gameplay for reproducible testing
- 74 test files with extensive coverage
- 14 evaluation scripts for AI research
- Complete architecture documentation

## License & Credits

**mdhearts** - Modern Rust implementation of Microsoft Hearts

Built with:
- Rust 2024 edition
- Win32 API & Direct2D for native Windows UI
- Cargo workspace architecture
- Criterion.rs for benchmarking

---

**For questions, issues, or contributions, see the GitHub repository.**