# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**mdhearts** is a modern Rust revival of the classic Microsoft Hearts card game, targeting Windows with a native Win32/Direct2D UI. The project uses Rust 1.81+ with the 2024 edition.

## Build & Development Commands

### Basic Commands
- **Run the game**: `cargo run -p hearts-app --bin mdhearts`
- **Build release**: `cargo build --release`
- **Run all tests**: `cargo test --workspace`
- **Run specific test**: `cargo test --package hearts-core test_name`
- **Format code**: `cargo fmt --all`
- **Lint code**: `cargo clippy --workspace -- -D warnings`

### Snapshot CLI Tools
- **Export snapshot**: `mdhearts.exe --export-snapshot snapshots/test.json [seed] [seat]`
- **Import snapshot**: `mdhearts.exe --import-snapshot snapshots/test.json`

### Release Packaging
- Build: `cargo build --release`
- Package (requires Inno Setup): `iscc installers\Hearts.iss`
- Output: `installers/MDHearts-1.0.1-Setup.exe`

## Configuration

### Environment Variables
- `MDH_BOT_DIFFICULTY`: Set to `easy`, `normal`, or `hard`
  - `easy`/`legacy`: Legacy bot logic
  - `normal`/`default`: Heuristic planner (default)
  - `hard`/`future`: Advanced difficulty (triggers hunt-leader mode at lower thresholds)
- `MDH_DEBUG_LOGS=1`: Enables detailed AI decision logging via DebugView

## Codebase Architecture

### Workspace Structure (3 crates)

**hearts-core** (`crates/hearts-core/`)
- Pure Rust game logic with no platform dependencies
- Contains deterministic rules engine and AI logic
- Modules:
  - `model/`: Core game types (Card, Deck, Hand, Round, Player, Suit, Rank, Trick, Passing, Score)
  - `game/`: Match state management and serialization (MatchState, snapshot import/export)
- All core engine changes require unit tests

**hearts-ui** (`crates/hearts-ui/`)
- Presentation metadata and theme abstractions
- Resource management for card/table textures
- No direct Win32/WinUI API usage—pure view models
- Assets sourced from mdsol Solitaire project

**hearts-app** (`crates/hearts-app/`)
- Platform entry point and Windows-specific interop
- Modules:
  - `main.rs`: Entry point with panic hook that logs to `mdhearts-panic.log`
  - `cli.rs`: CLI argument handling for snapshot import/export
  - `controller.rs`: GameController orchestrating match state and AI
  - `bot/`: AI system (PassPlanner, PlayPlanner, UnseenTracker)
    - Difficulty levels: EasyLegacy, NormalHeuristic, FutureHard
    - Bot styles: Cautious, AggressiveMoon, HuntLeader
  - `platform/`: Win32/Direct2D rendering (~3800 lines in `win32.rs`)

### Key Design Patterns

1. **Separation of Concerns**: Game logic (core) is isolated from presentation (ui) and platform (app)
2. **Unsafe Isolation**: All `unsafe` blocks are confined to `hearts-app::platform` modules with documented invariants
3. **Bot Context Pattern**: `BotContext` bundles seat, round state, scores, passing direction, tracker, and difficulty for AI decisions
4. **Snapshot System**: Matches can be serialized/deserialized with seed, round number, and scores for deterministic replay

## Code Quality Standards

- **Rust edition**: 2024 across all crates
- **Warnings**: Code must compile with `#![deny(warnings)]` (enforced in lib.rs/main.rs)
- **Tooling**: `cargo fmt` and `cargo clippy` must pass cleanly before commits
- **Error handling**: Use `Result<T>` with descriptive error variants; avoid `unwrap()` in production paths
- **Documentation**: Public APIs must have doc comments

## Platform Notes

- **UI Stack**: Win32/Direct2D (NOT WinUI3 or Windows App SDK)
- **Target**: `stable-x86_64-pc-windows-msvc`
- **Prerequisites**: See `docs/SETUP_WINDOWS.md` for Visual Studio Build Tools setup
- **Assets**: Card atlas (`assets/cards.png`) and layout (`assets/cards.json`) are checked into repo

## Additional Documentation

- `docs/CODING_STANDARDS.md`: Detailed coding guidelines
- `docs/SETUP_WINDOWS.md`: Build environment setup
- `docs/WIN32_UI_PLAN.md`: Win32/Direct2D UI implementation roadmap
- `docs/CLI_TOOLS.md`: Snapshot CLI usage details
- `crates/hearts-ui/README.md`: Asset integration notes
