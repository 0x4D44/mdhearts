# MDHearts

**MDHearts** is a high-performance, modern implementation of the classic card game **Hearts**, written in **Rust**. 

It features a native Windows (Direct2D) user interface and a sophisticated, multi-tiered Artificial Intelligence system designed for competitive play and research. The project is structured as a Cargo workspace, separating core game logic from the application layer and UI.

---

## 🌟 Key Features

*   **Native Windows UI:** Uses Direct2D and DirectWrite for crisp, hardware-accelerated rendering and native OS integration.
*   **Advanced AI:** Features multiple bot difficulty levels, ranging from basic heuristics to advanced Monte Carlo-style search algorithms.
*   **High Performance:** Written in 100% Rust, optimizing for zero-allocation game loops during AI simulation.
*   **Determinism:** Fully deterministic game replay support via seed control, essential for AI debugging and regression testing.
*   **Headless Simulation:** Extensive CLI tools for running millions of hands to statistically verify AI improvements.

## 🏗️ Architecture

The project is organized as a Cargo workspace with three primary crates:

| Crate | Description |
| :--- | :--- |
| **`hearts-core`** | **The Source of Truth.** Pure Rust implementation of Hearts rules, state management, scoring, and card models. Platform-agnostic. |
| **`hearts-ui`** | **Presentation Layer.** Handles Direct2D rendering, asset management, and view logic. |
| **`hearts-app`** | **Application Layer.** The entry point. Handles the Win32 message loop, orchestrates the `GameController`, and manages AI threads. |

## 🚀 Getting Started

### Prerequisites
*   **OS:** Windows 10 or 11 (Required for the GUI).
*   **Build Tool:** Rust (latest stable).

### Building and Running

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/your-username/mdhearts.git
    cd mdhearts
    ```

2.  **Run the Game (GUI):**
    This compiles the project and launches the native Windows application.
    ```bash
    cargo run -p hearts-app --bin mdhearts --release
    ```

3.  **Run Tests:**
    Run the comprehensive test suite (including the rigorous AI logic tests).
    ```bash
    cargo test --all
    ```

## 🤖 The AI System

MDHearts includes several distinct AI "Planners":

1.  **Easy / Legacy:** Basic rule-following. Avoids penalties but lacks strategic depth.
2.  **Normal (Heuristic):** A strong heuristic-based bot. It understands "Shooting the Moon", defensive passing, and suit voiding.
3.  **Hard (FutureHard):** Implements a 1-ply search with "wide" candidate consideration. It simulates the current trick to make optimal decisions based on probability.
4.  **Search (MCTS-like):** A deeper search bot (still experimental) capable of looking ahead multiple tricks.

### AI Configuration

You can control the bot difficulty via environment variables or CLI arguments.

**Environment Variables:**
*   `MDH_BOT_DIFFICULTY`: Sets the bot logic (`easy`, `normal`, `hard`, `search`).
*   `MDH_DEBUG_LOGS`: Set to `1` to see the AI's internal scoring and decision-making process in the console.
*   `MDH_HARD_TIME_CAP_MS`: Limits the thinking time for the Hard/Search bots (default: 10ms).

## 🛠️ CLI Tools & Evaluation

MDHearts is built for research. It includes powerful CLI tools to evaluate bot performance.

### Headless Match
Run a single headless match and see the result:
```bash
cargo run -p hearts-app --bin mdhearts -- --headless --seed 12345
```

### Batch Comparison
Run 1000 games comparing the "Hard" bot (West) against "Normal" bots (others):
```bash
# Usage: <subject_seat> <games> <batch_size>
cargo run -p hearts-app --bin mdhearts --release -- --compare-batch west 1000 50
```

### Explain Decision
Force the AI to explain why it chose a specific card for a specific game state (snapshot):
```bash
cargo run -p hearts-app --bin mdhearts -- --explain-once <path_to_snapshot.json>
```

## 📂 Project Structure

*   `assets/`: Images and resources (card sprites).
*   `crates/`: Source code for the Rust crates.
*   `designs/`: Engineering journals, implementation plans, and AI tuning reports.
*   `docs/`: Detailed documentation on CLI usage, setup, and coding standards.
*   `tools/`: PowerShell and Shell scripts for automated evaluation pipelines (e.g., `run_eval.ps1`).

## 📜 License

Private / Proprietary. (See `LICENSE` file if applicable).
