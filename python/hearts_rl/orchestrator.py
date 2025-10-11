"""Training orchestrator for end-to-end PPO pipeline."""

import subprocess
import argparse
import json
from pathlib import Path
from datetime import datetime
import shutil


class TrainingOrchestrator:
    """Orchestrates the full RL training pipeline."""

    def __init__(
        self,
        mdhearts_path: str = None,
        work_dir: str = "training_runs",
    ):
        """Initialize orchestrator.

        Args:
            mdhearts_path: Path to mdhearts executable
            work_dir: Working directory for outputs
        """
        if mdhearts_path is None:
            # Try to find mdhearts executable
            candidates = [
                Path("target/release/mdhearts.exe"),
                Path("target/debug/mdhearts.exe"),
                Path("../target/release/mdhearts.exe"),
                Path("../target/debug/mdhearts.exe"),
            ]
            for candidate in candidates:
                if candidate.exists():
                    self.mdhearts_path = str(candidate)
                    break
            else:
                raise FileNotFoundError("Could not find mdhearts executable")
        else:
            self.mdhearts_path = mdhearts_path

        self.work_dir = Path(work_dir)
        self.work_dir.mkdir(parents=True, exist_ok=True)

        print(f"Orchestrator initialized:")
        print(f"  mdhearts: {self.mdhearts_path}")
        print(f"  work_dir: {self.work_dir}")

    def collect_experiences(
        self,
        num_games: int,
        output_path: str,
        weights_path: str = None,
        reward_mode: str = "shaped",
    ):
        """Collect RL experiences via self-play.

        Args:
            num_games: Number of games to play
            output_path: Path for JSONL output
            weights_path: Optional custom weights
            reward_mode: Reward mode (shaped, per_trick, terminal)
        """
        print("\n" + "=" * 60)
        print("Step 1: Collecting Experiences")
        print("=" * 60)

        cmd = [
            self.mdhearts_path,
            "eval",
            str(num_games),
            "--self-play",
            "--collect-rl",
            output_path,
            "--reward-mode",
            reward_mode,
        ]

        if weights_path:
            cmd.extend(["--weights", weights_path])

        print(f"Running: {' '.join(cmd)}")

        result = subprocess.run(cmd, capture_output=True, text=True, timeout=600)

        if result.returncode != 0:
            raise RuntimeError(f"Experience collection failed:\n{result.stderr}")

        print(result.stdout)

        # Verify output file exists
        if not Path(output_path).exists():
            raise RuntimeError(f"Output file not created: {output_path}")

        # Count experiences
        with open(output_path, 'r') as f:
            num_experiences = sum(1 for _ in f)

        print(f"\n✓ Collected {num_experiences} experiences to {output_path}")
        return num_experiences

    def train_ppo(
        self,
        data_path: str,
        output_weights: str,
        iterations: int = 100,
        batch_size: int = 256,
        learning_rate: float = 3e-4,
        checkpoint_dir: str = None,
        log_dir: str = None,
    ):
        """Train PPO model.

        Args:
            data_path: Path to JSONL experiences
            output_weights: Path for output weights JSON
            iterations: Number of training iterations
            batch_size: Batch size
            learning_rate: Learning rate
            checkpoint_dir: Checkpoint directory
            log_dir: TensorBoard log directory
        """
        print("\n" + "=" * 60)
        print("Step 2: Training PPO Model")
        print("=" * 60)

        cmd = [
            "python",
            "-m",
            "hearts_rl.train",
            "--data",
            data_path,
            "--output",
            output_weights,
            "--iterations",
            str(iterations),
            "--batch-size",
            str(batch_size),
            "--lr",
            str(learning_rate),
        ]

        if checkpoint_dir:
            cmd.extend(["--checkpoint-dir", checkpoint_dir])

        if log_dir:
            cmd.extend(["--log-dir", log_dir])

        print(f"Running: {' '.join(cmd)}")

        result = subprocess.run(cmd, timeout=3600)  # 1 hour timeout

        if result.returncode != 0:
            raise RuntimeError("Training failed")

        # Verify weights file exists
        if not Path(output_weights).exists():
            raise RuntimeError(f"Weights file not created: {output_weights}")

        print(f"\n✓ Training complete, weights saved to {output_weights}")

    def evaluate_policy(
        self,
        weights_path: str,
        num_games: int = 100,
        baseline: str = "normal",
    ):
        """Evaluate trained policy.

        Args:
            weights_path: Path to trained weights
            num_games: Number of evaluation games
            baseline: Baseline AI type

        Returns:
            Evaluation results dictionary
        """
        print("\n" + "=" * 60)
        print("Step 3: Evaluating Policy")
        print("=" * 60)

        cmd = [
            "python",
            "-m",
            "hearts_rl.evaluate",
            "--mode",
            "compare",
            "--games",
            str(num_games),
            "--weights",
            weights_path,
            "--baseline",
            baseline,
            "--mdhearts",
            self.mdhearts_path,
        ]

        print(f"Running: {' '.join(cmd)}")

        result = subprocess.run(cmd, timeout=600)

        if result.returncode != 0:
            print("Warning: Evaluation failed")
            return None

        # Load results
        results_path = Path("evaluation_results.json")
        if results_path.exists():
            with open(results_path, 'r') as f:
                results = json.load(f)
            print(f"\n✓ Evaluation complete")
            return results

        return None

    def run_full_pipeline(
        self,
        num_collection_games: int = 1000,
        num_training_iterations: int = 100,
        num_eval_games: int = 100,
        reward_mode: str = "shaped",
        baseline: str = "normal",
        run_name: str = None,
    ):
        """Run the full training pipeline.

        Args:
            num_collection_games: Games for experience collection
            num_training_iterations: PPO training iterations
            num_eval_games: Evaluation games
            reward_mode: Reward mode for collection
            baseline: Baseline AI for evaluation
            run_name: Optional run name (defaults to timestamp)

        Returns:
            Pipeline results dictionary
        """
        if run_name is None:
            run_name = datetime.now().strftime("%Y%m%d_%H%M%S")

        run_dir = self.work_dir / run_name
        run_dir.mkdir(parents=True, exist_ok=True)

        print("\n" + "=" * 60)
        print(f"Starting Training Pipeline: {run_name}")
        print("=" * 60)
        print(f"Run directory: {run_dir}")

        # Define paths
        experiences_path = run_dir / "experiences.jsonl"
        weights_path = run_dir / "weights.json"
        checkpoint_dir = run_dir / "checkpoints"
        log_dir = run_dir / "logs"

        try:
            # Step 1: Collect experiences
            num_experiences = self.collect_experiences(
                num_games=num_collection_games,
                output_path=str(experiences_path),
                reward_mode=reward_mode,
            )

            # Step 2: Train PPO
            self.train_ppo(
                data_path=str(experiences_path),
                output_weights=str(weights_path),
                iterations=num_training_iterations,
                checkpoint_dir=str(checkpoint_dir),
                log_dir=str(log_dir),
            )

            # Step 3: Evaluate
            eval_results = self.evaluate_policy(
                weights_path=str(weights_path),
                num_games=num_eval_games,
                baseline=baseline,
            )

            # Save run metadata
            metadata = {
                "run_name": run_name,
                "timestamp": datetime.now().isoformat(),
                "config": {
                    "num_collection_games": num_collection_games,
                    "num_training_iterations": num_training_iterations,
                    "num_eval_games": num_eval_games,
                    "reward_mode": reward_mode,
                    "baseline": baseline,
                },
                "num_experiences": num_experiences,
                "evaluation": eval_results,
            }

            metadata_path = run_dir / "metadata.json"
            with open(metadata_path, 'w') as f:
                json.dump(metadata, f, indent=2)

            print("\n" + "=" * 60)
            print("Pipeline Complete!")
            print("=" * 60)
            print(f"Run directory: {run_dir}")
            print(f"Weights: {weights_path}")
            print(f"Metadata: {metadata_path}")

            if eval_results:
                improvement = eval_results.get("improvement", 0)
                improvement_pct = eval_results.get("improvement_pct", 0)
                print(f"\nPerformance vs {baseline}:")
                print(f"  Improvement: {improvement:.2f} points ({improvement_pct:.1f}%)")

            return metadata

        except Exception as e:
            print(f"\n✗ Pipeline failed: {e}")
            raise


def main():
    """Main orchestrator function."""
    parser = argparse.ArgumentParser(description="Run Hearts RL training pipeline")

    parser.add_argument(
        '--collection-games',
        type=int,
        default=1000,
        help='Number of games for experience collection (default: 1000)',
    )
    parser.add_argument(
        '--training-iterations',
        type=int,
        default=100,
        help='Number of PPO training iterations (default: 100)',
    )
    parser.add_argument(
        '--eval-games',
        type=int,
        default=100,
        help='Number of evaluation games (default: 100)',
    )
    parser.add_argument(
        '--reward-mode',
        type=str,
        default='shaped',
        choices=['shaped', 'per_trick', 'terminal'],
        help='Reward mode (default: shaped)',
    )
    parser.add_argument(
        '--baseline',
        type=str,
        default='normal',
        choices=['easy', 'normal', 'hard'],
        help='Baseline AI for evaluation (default: normal)',
    )
    parser.add_argument(
        '--run-name',
        type=str,
        default=None,
        help='Run name (default: timestamp)',
    )
    parser.add_argument(
        '--work-dir',
        type=str,
        default='training_runs',
        help='Working directory (default: training_runs)',
    )
    parser.add_argument(
        '--mdhearts',
        type=str,
        default=None,
        help='Path to mdhearts executable',
    )

    args = parser.parse_args()

    # Initialize orchestrator
    orchestrator = TrainingOrchestrator(
        mdhearts_path=args.mdhearts,
        work_dir=args.work_dir,
    )

    # Run pipeline
    try:
        results = orchestrator.run_full_pipeline(
            num_collection_games=args.collection_games,
            num_training_iterations=args.training_iterations,
            num_eval_games=args.eval_games,
            reward_mode=args.reward_mode,
            baseline=args.baseline,
            run_name=args.run_name,
        )
        return 0
    except Exception as e:
        print(f"Pipeline failed: {e}")
        return 1


if __name__ == '__main__':
    import sys
    sys.exit(main())
