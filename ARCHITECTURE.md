# MDHearts Architecture Summary

**Date:** 2025-11.06
**Purpose:** High-level architecture overview synthesizing all component documentation

---

## Executive Summary

MDHearts is a production-quality Rust implementation of Microsoft Hearts featuring sophisticated multi-difficulty AI, native Win32/Direct2D UI, and comprehensive CLI evaluation tools. The architecture demonstrates clean separation of concerns across 3 Cargo crates with ~8,000 lines of code.

**Key Metrics:**
- 3 workspace crates (hearts-core, hearts-ui, hearts-app)
- 4 AI difficulty levels with 60+ tunable parameters
- <15ms Hard AI decisions, 60 FPS rendering
- 74 test files, 14 evaluation scripts
- 162 design documents

---

## System Architecture

### Three-Tier Design

**Tier 1: hearts-core (Game Engine)**
- Pure Rust, platform-independent
- Deterministic Hearts rules implementation
- ~2,500 lines, zero unsafe code
- Key types: Card, Hand, Trick, RoundState, ScoreBoard
- Snapshot serialization for testing

**Tier 2: hearts-ui (Presentation)**
- Asset and theme management
- ~160 lines, foundational layer
- Compile-time asset embedding
- Platform-agnostic color schemes
- Awaiting full integration into renderer

**Tier 3: hearts-app (Platform + AI + CLI)**
- Win32/Direct2D native UI (~5,700 lines platform code)
- Sophisticated AI system (~3,700 lines bot code)
- Comprehensive CLI tools for evaluation
- Controller orchestrating all layers
- Telemetry and dataset export

### Dependency Graph

```
hearts-app
├── hearts-core (game rules)
├── hearts-ui (presentation)
├── windows 0.62 (Win32/D2D APIs)
├── parking_lot (fast mutexes)
└── serde ecosystem

hearts-ui
├── once_cell
└── serde

hearts-core
├── rand
└── serde
```

Clean separation: no circular dependencies, core is fully portable.

---

## AI System Architecture

### Four Difficulty Levels

**1. Easy (Legacy)**
- Simple heuristics
- <1μs per decision
- Beginner level

**2. Normal (Heuristic) - Default**
- Feature-based scoring with ~15 weighted factors
- Single-trick simulation
- 5-50μs per decision
- Intermediate level
- 9 tunable weights via environment variables

**3. Hard (Search) - Advanced**
- Shallow search with continuation scoring
- Top-K candidate selection (default K=6)
- Belief-guided opponent modeling
- Leverage-based adaptive search depth
- 2-15ms per decision (deterministic 120 steps)
- 30+ tunable parameters

**4. Search (Ultra-Hard) - Expert**
- Deep multi-ply search with alpha-beta pruning (up to 10 plies)
- Transposition tables (10M positions, ~160MB)
- Iterative deepening with aspiration windows
- Killer move heuristic for move ordering
- Belief-state sampling (100 samples for imperfect information)
- Perfect endgame solver (up to 13 cards)
- Moon shooting detection and scoring
- 0.5-2 second decisions with timeout protection
- 20+ tunable search parameters

### AI Decision Flow

**Normal AI:**
1. Generate legal cards
2. Score each with weighted features
3. Simulate trick outcome
4. Select highest score

**Hard AI:**
1. Base score all candidates (Normal AI)
2. Select top-K by base score
3. For each top-K:
   - Simulate playing card
   - Probe M next-trick scenarios (M=3)
   - Compute continuation score
4. Total = base + continuation
5. Select highest total

**Search AI (Ultra-Hard):**
1. Check if endgame (≤13 cards remaining)
   - If yes: Use perfect minimax solver with belief sampling
   - If no: Continue to deep search
2. Generate 100 belief-state samples (opponent hand distributions)
3. For each legal move:
   - Iterative deepening (depth 1→10):
     - Try aspiration window search first (narrow bounds)
     - If fail, re-search with full window
   - Alpha-beta pruning with transposition table
   - Order moves by: killer moves → heuristic score
   - Simulate opponent responses recursively
   - Cache positions in transposition table (10M entries)
4. Return best move from deepest completed depth
5. Timeout protection: return best move found so far

**Endgame Solver (Perfect Play):**
1. Build position: hands + trick + penalties
2. Minimax with memoization:
   - Base case: all hands empty → return penalties (with moon check)
   - Recursive case: try all legal moves
   - Minimize our penalties, assume opponents minimize theirs
3. Cache results by (position, perspective)
4. Timeout protection: return None if exceeded

### Bot Components

- **mod.rs:** Core types (BotContext, BotDifficulty, BotStyle, DecisionLimit)
- **play.rs:** PlayPlanner (Normal heuristic AI, ~1,700 lines)
- **search.rs:** PlayPlannerHard (Hard search AI, ~1,900 lines)
- **search_deep.rs:** DeepSearchEngine (Search AI with alpha-beta, ~776 lines)
- **endgame.rs:** EndgameSolver (Perfect endgame play, ~400 lines)
- **pass.rs:** PassPlanner (card passing logic, ~600 lines)
- **tracker.rs:** UnseenTracker (void inference, beliefs, sampling, ~700 lines)
- **adviser.rs:** Optional external bias system (~65 lines)

---

## Platform Integration

### Win32/Direct2D Stack

**Window Management:**
- RegisterClassW + CreateWindowExW
- Standard Win32 message loop
- Per-monitor V2 DPI awareness

**Rendering:**
- Hardware-accelerated Direct2D
- Device loss recovery
- 60 FPS with animations
- Card atlas (1248×384, 96×96 per card)
- 10 card back variants

**Animations:**
- Play: 260ms ease-out
- Collect: 350ms + 320ms two-phase
- Pass: Three-phase simultaneous

**Safety:**
- 211 unsafe blocks (all FFI)
- Documented invariants for each
- Isolated in platform/ module

---

## CLI Evaluation System

### Command Categories

**1. Snapshot Management**
- `--export-snapshot`: Save game state to JSON
- `--import-snapshot`: Load and inspect state

**2. AI Explanation**
- `--explain-once <seed> <seat>`: Single decision analysis
- `--explain-batch <seat> <start> <count>`: Multi-seed analysis
- `--explain-json`: Detailed JSON with candidate breakdown

**3. AI Comparison**
- `--compare-once`: Normal vs Hard single decision
- `--compare-batch`: CSV comparison across seeds
- `--only-disagree`: Filter to disagreements only

**4. Head-to-Head Evaluation**
- `--match-batch`: Simulate games A vs B, output penalty CSVs
- `--match-mixed-file`: Mixed-seat tournaments (NNHH, HHNN, etc.)

**5. Utility**
- `--show-weights`: Display active AI weights
- `--show-hard-telemetry`: Export decision data to NDJSON

### Evaluation Scripts (14 total)

**Bash/PowerShell:**
- `run_eval.sh` / `run_eval.ps1`: Full deterministic evaluation
- `smoke_fast.sh`: Ultra-fast CI smoke (1 seed per seat)
- `compare_small.sh` / `compare_medium.sh`: Quick comparisons
- `check_smoke_artifacts.sh`: Validate CSV outputs
- `index_compare.sh`: Generate markdown indexes

### Deterministic Evaluation

```bash
MDH_HARD_DETERMINISTIC=1
MDH_HARD_TEST_STEPS=120
mdhearts --compare-batch west 1000 50 --out results.csv
```

Critical for reproducible AI research and regression testing.

---

## Controller Orchestration

### Responsibilities

1. **Game State Management**
   - Owns MatchState, ScoreBoard, UnseenTracker
   - Enforces game rules via hearts-core
   - Validates all state transitions

2. **Bot Integration**
   - Spawns worker threads for bot thinking
   - Enforces timeout with fallback strategies
   - Provides immutable BotContext snapshot

3. **State Coordination**
   - Match → Round → Trick → Play flow
   - Passing phase → Playing phase transitions
   - Round scoring with shoot-the-moon detection

4. **Telemetry Collection**
   - Records Hard AI decision data
   - Exports to NDJSON for analysis
   - Manages telemetry retention (default 20 exports)

### Threading Model

```
Main Thread (UI)              Worker Thread (Bot)
      │                              │
      ├─ Start bot thinking ────────►│
      │                              │ Compute decision
      │                              │ (with timeout)
      │◄─── Return card ─────────────┤
      ├─ Validate & apply
      └─ Update UI
```

---

## Build System & CI

### Workspace Structure

```
Cargo.toml (workspace root)
.cargo/config.toml (-D warnings)
├── crates/hearts-core/
├── crates/hearts-ui/
└── crates/hearts-app/
    ├── src/
    ├── benches/ (2 Criterion suites)
    └── build.rs (timestamp, Windows resources)
```

### CI Pipeline (GitHub Actions)

**Job 1: test (Linux + Windows matrix)**
- Build --all --release
- Test --all --verbose
- Cached cargo registry

**Job 2: eval-smoke (Linux, PR only)**
- Deterministic Hard AI (60 steps)
- 5 seeds per seat
- Compare & match evaluation

**Job 3: ultra-smoke (Linux, PR only)**
- 1 seed per seat, 2 configs (NNHH + HHNN)
- Artifact validation
- Upload CSVs

### Quality Gates

- ✅ Zero warnings (`-D warnings` enforced)
- ✅ Clippy passes
- ✅ All tests pass
- ✅ Smoke tests succeed (PR only)
- ✅ Format check (cargo fmt)

---

## Key Design Decisions

### 1. Cargo Workspace (3 Crates)
**Rationale:** Clean separation without over-fragmentation. Core is portable, UI is presentation layer, app is platform-specific.

### 2. Win32/Direct2D (Not WinUI 3)
**Rationale:** Maximum control, best performance, no framework overhead. Acceptable trade-off: Windows-only.

### 3. Shallow Search (Not Deep Search)
**Rationale:** 1-ply fits in <15ms, significant strength gain, interpretable, deterministic for testing. Hearts variance limits deep search benefits.

### 4. Deterministic Mode
**Rationale:** Critical for reproducible testing, fair benchmarking, AI research. Step-based budget instead of wall-clock time.

### 5. Extensive CLI Tooling
**Rationale:** AI research requires batch evaluation, automated testing, external analysis. 20+ commands enable rigorous development.

---

## Data Flow

### Player Turn
```
Human Click
  → Platform (WM_LBUTTONDOWN, pixel to card)
  → Controller (validate against legal moves)
  → RoundState (play_card, update state)
  → Check trick complete (4 cards)
  → Determine winner (highest of led suit)
  → Award penalties, update UnseenTracker
  → Check round end (no cards left)
  → Score round (shoot moon detection)
  → Update ScoreBoard
  → Check match end (player ≥ 100)
  → UI refresh
```

### Bot Turn
```
Controller spawns worker thread
  → Bot receives immutable BotContext
  → Bot computes decision (Normal or Hard AI)
  → Worker returns card (with timeout)
  → Controller validates card is legal
  → Apply to RoundState
  → Continue game flow
```

---

## Quality Attributes Achieved

**Performance:**
- ✅ Normal AI: 5-50μs (target: <100μs)
- ✅ Hard AI: 2-15ms deterministic (target: <30ms)
- ✅ Rendering: 60 FPS (target: 60 FPS)
- ✅ Startup: <1s cold start

**Testability:**
- ✅ 74 test files
- ✅ Deterministic gameplay
- ✅ Snapshot-based regression tests
- ✅ CLI evaluation framework

**Maintainability:**
- ✅ Clean layer separation
- ✅ Zero warnings enforced
- ✅ 162 design documents
- ✅ Comprehensive code docs

**Tunability:**
- ✅ 60+ configuration parameters
- ✅ CLI flags for overrides
- ✅ External adviser bias
- ✅ Feature flags for staging work


## Reliability Enhancements (2025-11-18)

- **Rules Fidelity:** `RoundState::validate_play`/`legal_cards` centralize legality checks so bots/UI share the same logic without cloning entire rounds.
- **Tracker Integrity:** Passing no longer reveals cards; `UnseenTracker` rebuilds after passes so the unseen set always reflects 52 minus played cards.
- **Telemetry Retention:** `TelemetrySink::push` now enforces retention caps immediately, turning the sink into a ring buffer for long GUI sessions.
- **Regression Coverage:** Stage 1 guard tests (`hard_guard_round_leader_saturated_blocks_feed`, `hard_flat_scores_uses_round_leader_penalties_gt0`) run in CI via deterministic fixtures guarded by feature flags and mutexes.---

## Component Documentation

Detailed architecture documentation for each component:

1. **hearts-core crate:** `wrk_docs/2025.11.06 - Architecture - hearts-core crate.md`
   - Complete model breakdown (Card, Hand, Trick, Round, Score, etc.)
   - Game rules implementation
   - Serialization system

2. **hearts-ui crate:** `wrk_docs/2025.11.06 - Architecture - hearts-ui crate.md`
   - Asset management (card atlas metadata)
   - Theme system (color schemes)
   - Resource loading patterns

3. **AI Bot System:** `wrk_docs/2025.11.06 - Architecture - AI Bot System.md`
   - All 6 modules detailed
   - Normal vs Hard AI algorithms
   - 60+ configuration parameters
   - Belief system and void inference
   - Performance characteristics

4. **Platform Layer:** `wrk_docs/2025.11.06 - Architecture - Platform Layer.md`
   - Win32 window management
   - Direct2D rendering pipeline
   - Animation system (3 types)
   - Asset loading (WIC-based)
   - Safety considerations (211 unsafe blocks)

5. **Controller & Orchestration:** `wrk_docs/2025.11.06 - Architecture - Controller and Orchestration.md`
   - Game flow state machines
   - Bot integration patterns
   - Threading model
   - Telemetry collection

6. **CLI & Evaluation:** `wrk_docs/2025.11.06 - Architecture - CLI and Evaluation System.md`
   - 20+ CLI commands
   - Output formats (console, CSV, JSON, NDJSON)
   - Deterministic evaluation support

7. **Build & Tooling:** `wrk_docs/2025.11.06 - Architecture - Build System and Tooling.md`
   - Workspace structure and dependencies
   - CI pipeline (3 jobs)
   - 14 evaluation scripts
   - Quality gates
   - Testing infrastructure (74 files, 2 benchmarks)

---

## Conclusion

MDHearts demonstrates a **production-quality architecture** for a sophisticated card game implementation:

- ✅ Clean 3-tier architecture with no circular dependencies
- ✅ Sophisticated AI with 4 difficulty levels and 60+ tunable parameters
- ✅ Native platform integration with hardware-accelerated rendering
- ✅ Comprehensive evaluation tooling for AI research
- ✅ Deterministic gameplay enabling reproducible testing
- ✅ Type-safe Rust with zero warnings policy
- ✅ Extensive documentation (162 design docs + comprehensive architecture analysis)

**Architectural Highlights:**
- Immutability and snapshot isolation for thread safety
- Shallow search proving sufficient for strong play
- Leverage-based adaptive search depth
- Belief-guided opponent modeling
- Extensive telemetry for AI research
- Rich CLI tooling ecosystem

**Recommended For:**
- Card game AI implementations
- Game AI research and education
- Real-time AI systems requiring explainability
- Native Windows game development
- Rust game development patterns

This architecture proves that **well-engineered heuristics + shallow search + opponent modeling** can produce expert-level play without deep search or machine learning, while maintaining real-time performance and full explainability.



