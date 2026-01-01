# MDHearts 🂩

[![CI](https://github.com/your-username/mdhearts/actions/workflows/ci.yml/badge.svg)](https://github.com/your-username/mdhearts/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**MDHearts** is a modern, high-performance implementation of the classic Microsoft Hearts card game, engineered in Rust. It combines a polished native Windows UI (Direct2D) with a research-grade AI framework capable of ultra-hard gameplay analysis.

![Screenshot placeholder - add a screenshot of the game here](assets/screenshot_placeholder.png)

## ✨ Key Features

-   **🎮 Native Experience:** Built with the Win32 API and Direct2D for buttery smooth, hardware-accelerated rendering (60 FPS).
-   **🤖 Advanced AI:** Four distinct difficulty levels ranging from a simple heuristic bot to a "Search" engine that uses Monte Carlo simulations and alpha-beta pruning.
-   **⚡ High Performance:** Core game logic is highly optimized, with "Hard" bots making sub-15ms decisions and the engine simulating thousands of games per second for evaluation.
-   **🔬 Research Tooling:** Includes a suite of CLI tools for batch evaluation, deterministic replays, and detailed decision explanation—perfect for AI tuning and regression testing.
-   **🛠️ Modular Architecture:** Clean separation between the core game engine (`hearts-core`), the UI layer (`hearts-ui`), and the application shell (`hearts-app`).

## 🚀 Getting Started

### Prerequisites

-   **Operating System:** Windows 10/11 (x64)
-   **Tools:** [Rust Toolchain](https://rustup.rs/) (stable)

### Installation

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/your-username/mdhearts.git
    cd mdhearts
    ```

2.  **Build and Run:**
    ```bash
    cargo run -p hearts-app --bin mdhearts --release
    ```

### Running Tests

MDHearts maintains a high standard of code quality with extensive test coverage.

```bash
# Run unit and integration tests
cargo test --all

# Run benchmarks (requires criterion)
cargo bench -p hearts-app
```

## 🧠 AI System

The game features four difficulty levels configurable via the `MDH_BOT_DIFFICULTY` environment variable or CLI arguments:

| Difficulty | Description | Tech Stack |
| :--- | :--- | :--- |
| **Easy** | Basic rules follower. Avoids penalties but has no long-term plan. | Simple Heuristics |
| **Normal** (Default) | Competent player. Counts cards, avoids voids, and tries to shoot the moon opportunistically. | Weighted Heuristics |
| **Hard** | Strong opponent. Simulates tricks to evaluate outcomes, uses shallow search, and plays aggressively. | Shallow Search + Heuristics |
| **Search** | Expert level. Uses iterative deepening, belief-state sampling (for hidden cards), and a perfect endgame solver. | MCTS / Alpha-Beta Pruning |

To play against the hardest bot:
```powershell
$env:MDH_BOT_DIFFICULTY = "hard"
cargo run -p hearts-app --bin mdhearts --release
```

## 🛠️ CLI Tools for Research

MDHearts isn't just a game; it's a platform for AI research. The CLI exposes powerful commands for analyzing game states.

-   **Explain a Move:** See why the AI made a specific choice.
    ```bash
    mdhearts --explain-once <seed> <seat> <difficulty>
    ```
-   **Export Game State:** Save a snapshot to JSON for debugging.
    ```bash
    mdhearts --export-snapshot <path> <seed> <seat>
    ```
-   **Run a Match Batch:** Simulate thousands of games to measure win rates.
    ```bash
    mdhearts --match-batch <seat> <start_seed> <count> hard normal
    ```

See [`docs/CLI_TOOLS.md`](docs/CLI_TOOLS.md) for the complete reference.

## 📂 Project Structure

```text
mdhearts/
├── crates/
│   ├── hearts-core/       # Pure Rust game logic (Platform-agnostic)
│   ├── hearts-ui/         # Asset management and theming
│   └── hearts-app/        # Windows App, AI logic, and CLI entry point
├── tools/                 # PowerShell/Bash scripts for CI and evaluation
├── assets/                # Game assets (cards, config)
└── designs/               # Architecture decision records (ADRs) and plans
```

## 🤝 Contributing

Contributions are welcome! Please check out our [Contributing Guide](docs/CONTRIBUTING_AI_TUNING.md) and [Coding Standards](docs/CODING_STANDARDS.md).

1.  Fork the repo.
2.  Create your feature branch (`git checkout -b feature/amazing-feature`).
3.  Commit your changes.
4.  Push to the branch.
5.  Open a Pull Request.

## 📜 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

**Note:** This project serves as a comprehensive example of modern Rust application development, demonstrating FFI integration, complex state management, and high-performance computing patterns.