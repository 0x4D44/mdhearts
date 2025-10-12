#!/usr/bin/env python3
"""
Evaluate a Gen3 training checkpoint to map loss -> performance.

Usage:
    python tools/eval_checkpoint.py gen3_checkpoints/checkpoint_50.pt --games 200
"""

import argparse
import subprocess
import json
import sys
from pathlib import Path


def export_checkpoint_to_json(checkpoint_path: str, output_json: str):
    """Export PyTorch checkpoint to JSON weights for Rust inference."""
    print(f"Exporting {checkpoint_path} to {output_json}...")

    result = subprocess.run(
        ["python", "python/export_checkpoint.py",
         checkpoint_path, output_json],
        capture_output=True,
        text=True
    )

    if result.returncode != 0:
        print(f"Error exporting checkpoint: {result.stderr}")
        sys.exit(1)

    print(f"✓ Exported to {output_json}")


def run_evaluation(weights_json: str, num_games: int):
    """Run evaluation games and return results."""
    print(f"Running {num_games} games vs Hard baseline...")

    result = subprocess.run(
        ["./target/release/mdhearts.exe", "eval", str(num_games),
         "--ai", "hard",
         "--ai-test", "embedded",
         "--weights", weights_json],
        capture_output=True,
        text=True
    )

    if result.returncode != 0:
        print(f"Error running evaluation: {result.stderr}")
        sys.exit(1)

    return result.stdout


def parse_eval_results(output: str):
    """Parse evaluation results from mdhearts output."""
    results = {}

    for line in output.split('\n'):
        if "Test avg:" in line:
            results['test_avg'] = float(line.split(':')[1].strip().split()[0])
        elif "Baseline avg:" in line:
            results['baseline_avg'] = float(line.split(':')[1].strip().split()[0])
        elif "Improvement:" in line:
            # Format: "Improvement: -27.4%"
            pct = line.split(':')[1].strip().replace('%', '')
            results['improvement_pct'] = float(pct)
        elif "P-value:" in line:
            results['p_value'] = float(line.split(':')[1].strip())
        elif "Result:" in line:
            results['significant'] = "significant" in line.lower() and "not" not in line.lower()

    return results


def main():
    parser = argparse.ArgumentParser(description="Evaluate Gen3 checkpoint")
    parser.add_argument("checkpoint", help="Path to checkpoint .pt file")
    parser.add_argument("--games", type=int, default=200, help="Number of evaluation games")
    parser.add_argument("--keep-json", action="store_true", help="Keep exported JSON file")

    args = parser.parse_args()

    checkpoint_path = Path(args.checkpoint)
    if not checkpoint_path.exists():
        print(f"Error: Checkpoint not found: {checkpoint_path}")
        sys.exit(1)

    # Extract iteration number from checkpoint name (e.g., checkpoint_50.pt)
    iteration = checkpoint_path.stem.split('_')[-1]

    # Export to JSON
    json_path = f"gen3_iter{iteration}_eval.json"
    export_checkpoint_to_json(str(checkpoint_path), json_path)

    # Run evaluation
    output = run_evaluation(json_path, args.games)

    # Parse results
    results = parse_eval_results(output)

    # Display results
    print("\n" + "="*60)
    print(f"Gen3 Iteration {iteration} Evaluation Results ({args.games} games)")
    print("="*60)
    print(f"Test policy avg:     {results.get('test_avg', 'N/A')} points")
    print(f"Baseline (Hard) avg: {results.get('baseline_avg', 'N/A')} points")
    print(f"Improvement:         {results.get('improvement_pct', 'N/A'):.1f}%")
    print(f"P-value:             {results.get('p_value', 'N/A'):.4f}")
    print(f"Statistically sig:   {'Yes' if results.get('significant') else 'No'}")
    print("="*60)

    # Append to tracking file
    with open("gen3_checkpoint_tracking.txt", "a") as f:
        f.write(f"Iter {iteration}: {results.get('improvement_pct', 'N/A'):.1f}% improvement, "
                f"p={results.get('p_value', 'N/A'):.4f}, "
                f"sig={'Y' if results.get('significant') else 'N'}\n")

    # Cleanup
    if not args.keep_json:
        Path(json_path).unlink()
        print(f"Cleaned up {json_path}")


if __name__ == "__main__":
    main()
