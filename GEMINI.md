# MDHearts - Project Context for Gemini

## 1. Project Overview

**MDHearts** is a production-quality, modern Rust implementation of the Microsoft Hearts card game. It features a native Win32/Direct2D user interface, a sophisticated multi-difficulty AI system (Easy, Normal, Hard, Search), and a comprehensive suite of evaluation tools for AI research.

### Key Characteristics
- **Language:** Rust (2024 edition)
- **Architecture:** 3-tier Cargo workspace (`hearts-core`, `hearts-ui`, `hearts-app`)
- **UI:** Native Windows (Win32 API + Direct2D) with hardware acceleration.
- **AI:** 4 levels, including Search (MCTS-like) and Hard (Shallow Search). Deterministic replay support.
- **Tooling:** Extensive CLI for batch evaluation, regression testing, and AI tuning.

## 2. Directory Structure

```text
C:\language\mdhearts\
├── crates\
│   ├── hearts-core\       # Pure game logic (Rules, State, Scoring) - Platform Agnostic
│   ├── hearts-ui\         # Presentation layer (Assets, Themes, ViewModels)
│   └── hearts-app\        # Application layer (Win32 Entry, Direct2D, AI System, Controller)
├── tools\                 # Shell/PowerShell scripts for evaluation and CI
├── docs\                  # Developer documentation (Setup, Standards, CLI usage)
├── designs\               # Design docs, plans, and AI tuning reports
├── assets\                # Game assets (cards.png, cards.json)
└── ARCHITECTURE.md        # detailed system architecture
```

## 3. Development Workflow

### Build & Run
- **Run Game:** `cargo run -p hearts-app --bin mdhearts`
- **Build Release:** `cargo build --release`
- **Run Tests:** `cargo test --all`
- **Run Benchmarks:** `cargo bench -p hearts-app`

### Code Quality Standards
- **Formatting:** `cargo fmt --all`
- **Linting:** `cargo clippy --workspace -- -D warnings` (Strict: Zero warnings policy)
- **Safety:** `unsafe` code must be isolated in `hearts-app::platform` modules.
- **Error Handling:** Use `Result<T>`, avoid `unwrap()` in production code.

### AI Evaluation & Tuning
The project relies heavily on CLI tools for AI development.
- **Compare AI:** `cargo run -p hearts-app --bin mdhearts -- --compare-batch west 1000 50`
- **Explain Decision:** `cargo run -p hearts-app --bin mdhearts -- --explain-once 100 west hard`
- **Full Eval Suite:** `powershell tools/run_eval.ps1` (Windows) or `bash tools/run_eval.sh` (Linux)

## 4. Key CLI Commands & Environment Variables

| Command / Var | Description |
| :--- | :--- |
| **CLI Flags** | |
| `--export-snapshot <path>` | Save game state to JSON. |
| `--import-snapshot <path>` | Load game state. |
| `--match-batch ...` | Run head-to-head AI matches. |
| `--hard-deterministic` | Use step-based budget (not time) for reproducible results. |
| **Env Vars** | |
| `MDH_BOT_DIFFICULTY` | `easy` \| `normal` \| `hard` \| `search` |
| `MDH_DEBUG_LOGS` | `1` to enable detailed AI logging. |
| `MDH_HARD_TIME_CAP_MS` | Max think time for Hard AI (default: 10ms). |

## 5. Architecture Highlights

- **hearts-core:** Contains the `RoundState`, `Trick`, and `Card` models. It is the "source of truth".
- **hearts-app:**
  - **Controller:** Orchestrates the game loop, manages threads for AI.
  - **Platform:** Handles Win32 message loop and Direct2D painting.
  - **AI:** Implements the bot logic (`PlayPlanner`, `SearchEngine`).
- **Determinism:** The game is fully deterministic given a seed. Hard AI can run in "deterministic mode" (step-limited) for regression testing.

## 6. Documentation Index
- `ARCHITECTURE.md`: High-level system design.
- `docs/CLI_TOOLS.md`: Detailed reference for all CLI commands.
- `docs/CODING_STANDARDS.md`: Style guides and best practices.
- `designs/`: Chronological design notes and AI experiments.
