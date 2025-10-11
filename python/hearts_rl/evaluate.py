"""Evaluation harness for comparing policies."""

import subprocess
import json
import argparse
from pathlib import Path
from typing import Dict, List, Tuple
import statistics


class EvaluationHarness:
    """Harness for evaluating trained policies against baselines."""

    def __init__(self, mdhearts_path: str = None):
        """Initialize evaluation harness.

        Args:
            mdhearts_path: Path to mdhearts executable (default: auto-detect)
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
                raise FileNotFoundError("Could not find mdhearts executable. Please specify path.")
        else:
            self.mdhearts_path = mdhearts_path

        print(f"Using mdhearts at: {self.mdhearts_path}")

    def run_eval(
        self,
        num_games: int,
        ai_type: str = "normal",
        weights_path: str = None,
    ) -> Dict:
        """Run evaluation games.

        Args:
            num_games: Number of games to run
            ai_type: AI type (easy, normal, hard, embedded)
            weights_path: Optional path to custom weights

        Returns:
            Dictionary with evaluation results
        """
        cmd = [self.mdhearts_path, "eval", str(num_games), "--ai", ai_type]

        if weights_path:
            cmd.extend(["--weights", weights_path])

        print(f"Running: {' '.join(cmd)}")

        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=300,  # 5 minute timeout
        )

        if result.returncode != 0:
            raise RuntimeError(f"Evaluation failed: {result.stderr}")

        # Parse output to extract final summary
        lines = result.stdout.strip().split('\n')
        summary_line = None
        for line in lines:
            if line.strip().startswith('{') and "Final Summary" in result.stdout:
                # Try to parse as JSON
                try:
                    summary_line = line
                    break
                except:
                    pass

        # Find the final summary JSON
        for i, line in enumerate(lines):
            if "Final Summary:" in line and i + 1 < len(lines):
                summary_line = lines[i + 1]
                break

        if summary_line:
            try:
                summary = json.loads(summary_line)
                return summary
            except json.JSONDecodeError:
                pass

        # Fallback: parse the entire output
        return {"raw_output": result.stdout}

    def compare_policies(
        self,
        num_games: int,
        trained_weights: str,
        baseline_ai: str = "normal",
    ) -> Dict:
        """Compare trained policy against baseline.

        Args:
            num_games: Number of games to run for each policy
            trained_weights: Path to trained weights JSON
            baseline_ai: Baseline AI type

        Returns:
            Comparison results
        """
        print("=" * 60)
        print("Policy Comparison")
        print("=" * 60)

        # Run baseline
        print(f"\n1. Running {num_games} games with baseline ({baseline_ai})...")
        baseline_results = self.run_eval(num_games, ai_type=baseline_ai)

        # Run trained policy
        print(f"\n2. Running {num_games} games with trained policy...")
        trained_results = self.run_eval(num_games, ai_type="embedded", weights_path=trained_weights)

        # Compare results
        print("\n" + "=" * 60)
        print("Results")
        print("=" * 60)

        baseline_avg = baseline_results.get("avg_points", [0, 0, 0, 0])
        trained_avg = trained_results.get("avg_points", [0, 0, 0, 0])

        print(f"\nBaseline ({baseline_ai}) average points per seat:")
        for i, points in enumerate(baseline_avg):
            print(f"  Seat {i}: {points:.2f}")

        print(f"\nTrained policy average points per seat:")
        for i, points in enumerate(trained_avg):
            print(f"  Seat {i}: {points:.2f}")

        # Compute improvement
        baseline_mean = statistics.mean(baseline_avg)
        trained_mean = statistics.mean(trained_avg)
        improvement = baseline_mean - trained_mean  # Lower is better in Hearts

        print(f"\nOverall average:")
        print(f"  Baseline: {baseline_mean:.2f}")
        print(f"  Trained:  {trained_mean:.2f}")
        print(f"  Improvement: {improvement:.2f} points ({improvement/baseline_mean*100:.1f}%)")

        return {
            "baseline": baseline_results,
            "trained": trained_results,
            "baseline_mean": baseline_mean,
            "trained_mean": trained_mean,
            "improvement": improvement,
            "improvement_pct": improvement / baseline_mean * 100,
        }

    def benchmark_self_play(
        self,
        num_games: int,
        weights_path: str = None,
    ) -> Dict:
        """Run self-play benchmark.

        Args:
            num_games: Number of games
            weights_path: Optional weights (defaults to embedded)

        Returns:
            Benchmark results
        """
        print("=" * 60)
        print("Self-Play Benchmark")
        print("=" * 60)

        cmd = [self.mdhearts_path, "eval", str(num_games), "--self-play"]

        if weights_path:
            cmd.extend(["--weights", weights_path])

        print(f"Running: {' '.join(cmd)}")

        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=300,
        )

        if result.returncode != 0:
            raise RuntimeError(f"Benchmark failed: {result.stderr}")

        print(result.stdout)

        # Parse average points
        lines = result.stdout.strip().split('\n')
        avg_points = []
        for line in lines:
            if "Seat" in line and ":" in line:
                try:
                    points_str = line.split(':')[-1].strip()
                    avg_points.append(float(points_str))
                except:
                    pass

        if len(avg_points) == 4:
            mean_points = statistics.mean(avg_points)
            std_points = statistics.stdev(avg_points)

            print(f"\nStatistics:")
            print(f"  Mean: {mean_points:.2f}")
            print(f"  Std:  {std_points:.2f}")

            return {
                "avg_points": avg_points,
                "mean": mean_points,
                "std": std_points,
            }

        return {"raw_output": result.stdout}


def main():
    """Main evaluation function."""
    parser = argparse.ArgumentParser(description="Evaluate Hearts RL policies")

    parser.add_argument(
        '--mode',
        type=str,
        choices=['compare', 'benchmark'],
        default='compare',
        help='Evaluation mode (default: compare)',
    )
    parser.add_argument(
        '--games',
        type=int,
        default=100,
        help='Number of games to run (default: 100)',
    )
    parser.add_argument(
        '--weights',
        type=str,
        required=True,
        help='Path to trained weights JSON',
    )
    parser.add_argument(
        '--baseline',
        type=str,
        default='normal',
        choices=['easy', 'normal', 'hard'],
        help='Baseline AI type (default: normal)',
    )
    parser.add_argument(
        '--mdhearts',
        type=str,
        default=None,
        help='Path to mdhearts executable',
    )

    args = parser.parse_args()

    # Check weights file exists
    if not Path(args.weights).exists():
        print(f"Error: Weights file not found: {args.weights}")
        return 1

    # Initialize harness
    harness = EvaluationHarness(mdhearts_path=args.mdhearts)

    # Run evaluation
    if args.mode == 'compare':
        results = harness.compare_policies(
            num_games=args.games,
            trained_weights=args.weights,
            baseline_ai=args.baseline,
        )

        # Save results
        output_path = "evaluation_results.json"
        with open(output_path, 'w') as f:
            json.dump(results, f, indent=2)
        print(f"\nResults saved to: {output_path}")

    elif args.mode == 'benchmark':
        results = harness.benchmark_self_play(
            num_games=args.games,
            weights_path=args.weights,
        )

        # Save results
        output_path = "benchmark_results.json"
        with open(output_path, 'w') as f:
            json.dump(results, f, indent=2)
        print(f"\nResults saved to: {output_path}")

    return 0


if __name__ == '__main__':
    import sys
    sys.exit(main())
