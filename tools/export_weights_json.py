#!/usr/bin/env python3
"""
Export neural network weights to JSON format for loading into Rust.

This script can:
1. Convert weights from numpy arrays to JSON
2. Export the dummy weights from generated.rs to JSON format
3. Validate weight dimensions before export

Usage:
    python export_weights_json.py --input weights.npz --output weights.json
    python export_weights_json.py --from-generated --output weights.json
"""

import argparse
import json
import hashlib
import sys
from typing import Dict, List
import re


def compute_schema_hash() -> str:
    """Compute the same schema hash as build.rs."""
    schema_desc = (
        "v1.1.0:"
        "hand_onehot[52],"
        "seen_onehot[52],"
        "trick_led_suit[4],"
        "trick_cards[4][17],"
        "trick_count,"
        "my_trick_position,"
        "trick_pad,"
        "scores_relative[4],"
        "hearts_broken,"
        "tricks_completed,"
        "passing_phase,"
        "passing_direction[4],"
        "opp_voids[12],"
        "last_4_cards[68]"
    )
    return hashlib.sha256(schema_desc.encode()).hexdigest()


def validate_dimensions(weights_dict: Dict) -> bool:
    """Validate that weight dimensions match expected architecture."""
    expected = {
        "layer1_weights": 270 * 256,
        "layer1_biases": 256,
        "layer2_weights": 256 * 128,
        "layer2_biases": 128,
        "layer3_weights": 128 * 52,
        "layer3_biases": 52,
    }

    for key, expected_size in expected.items():
        if key not in weights_dict:
            print(f"Error: Missing {key}", file=sys.stderr)
            return False

        actual_size = len(weights_dict[key])
        if actual_size != expected_size:
            print(f"Error: {key} has wrong size. Expected {expected_size}, got {actual_size}", file=sys.stderr)
            return False

    return True


def export_from_numpy(npz_path: str, output_path: str) -> bool:
    """Export weights from numpy .npz file."""
    try:
        import numpy as np
    except ImportError:
        print("Error: numpy is required. Install with: pip install numpy", file=sys.stderr)
        return False

    try:
        data = np.load(npz_path)

        weights_dict = {
            "layer1_weights": data["layer1_weights"].flatten().tolist(),
            "layer1_biases": data["layer1_biases"].flatten().tolist(),
            "layer2_weights": data["layer2_weights"].flatten().tolist(),
            "layer2_biases": data["layer2_biases"].flatten().tolist(),
            "layer3_weights": data["layer3_weights"].flatten().tolist(),
            "layer3_biases": data["layer3_biases"].flatten().tolist(),
        }

        if not validate_dimensions(weights_dict):
            return False

        manifest = {
            "schema_version": "1.1.0",
            "schema_hash": compute_schema_hash(),
            "layer1": {
                "weights": weights_dict["layer1_weights"],
                "biases": weights_dict["layer1_biases"],
            },
            "layer2": {
                "weights": weights_dict["layer2_weights"],
                "biases": weights_dict["layer2_biases"],
            },
            "layer3": {
                "weights": weights_dict["layer3_weights"],
                "biases": weights_dict["layer3_biases"],
            },
        }

        with open(output_path, 'w') as f:
            json.dump(manifest, f, indent=2)

        total_params = sum(len(v) for v in weights_dict.values())
        print(f"[OK] Exported {total_params:,} parameters to {output_path}")
        return True

    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return False


def parse_rust_array(content: str, array_name: str) -> List[float]:
    """Parse a Rust static array from generated.rs."""
    # Find the array definition
    pattern = rf'pub static {array_name}: \[f32; \d+\] = \[(.*?)\];'
    match = re.search(pattern, content, re.DOTALL)

    if not match:
        raise ValueError(f"Could not find array {array_name}")

    # Extract numbers
    numbers_str = match.group(1)
    # Remove commas and newlines, split on whitespace
    numbers = []
    for token in numbers_str.replace(',', ' ').split():
        try:
            numbers.append(float(token))
        except ValueError:
            continue

    return numbers


def export_from_generated(generated_path: str, output_path: str) -> bool:
    """Export weights from generated.rs file."""
    try:
        with open(generated_path, 'r') as f:
            content = f.read()

        # Parse all arrays
        layer1_weights = parse_rust_array(content, "WEIGHTS")
        # Need to find layer1 and layer2 separately - let's parse by module

        # Split by module boundaries
        modules = re.split(r'pub mod (layer\d+) \{', content)

        weights_dict = {}

        # Find layer1 module
        for i, module_name in enumerate(modules):
            if module_name == "layer1":
                layer1_content = modules[i + 1].split("}")[0]
                weights_dict["layer1_weights"] = parse_rust_array("pub static " + layer1_content, "WEIGHTS")
                weights_dict["layer1_biases"] = parse_rust_array("pub static " + layer1_content, "BIASES")
            elif module_name == "layer2":
                layer2_content = modules[i + 1].split("}")[0]
                weights_dict["layer2_weights"] = parse_rust_array("pub static " + layer2_content, "WEIGHTS")
                weights_dict["layer2_biases"] = parse_rust_array("pub static " + layer2_content, "BIASES")
            elif module_name == "layer3":
                layer3_content = modules[i + 1].split("}")[0]
                weights_dict["layer3_weights"] = parse_rust_array("pub static " + layer3_content, "WEIGHTS")
                weights_dict["layer3_biases"] = parse_rust_array("pub static " + layer3_content, "BIASES")

        if not validate_dimensions(weights_dict):
            return False

        manifest = {
            "schema_version": "1.1.0",
            "schema_hash": compute_schema_hash(),
            "layer1": {
                "weights": weights_dict["layer1_weights"],
                "biases": weights_dict["layer1_biases"],
            },
            "layer2": {
                "weights": weights_dict["layer2_weights"],
                "biases": weights_dict["layer2_biases"],
            },
            "layer3": {
                "weights": weights_dict["layer3_weights"],
                "biases": weights_dict["layer3_biases"],
            },
        }

        with open(output_path, 'w') as f:
            json.dump(manifest, f, indent=2)

        total_params = sum(len(v) for v in weights_dict.values())
        print(f"[OK] Exported {total_params:,} parameters to {output_path}")
        return True

    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc()
        return False


def main():
    parser = argparse.ArgumentParser(description="Export neural network weights to JSON")
    parser.add_argument("--input", help="Input .npz file (numpy format)")
    parser.add_argument("--from-generated", action="store_true",
                       help="Export from generated.rs instead of numpy")
    parser.add_argument("--generated-path", default="crates/hearts-app/src/weights/generated.rs",
                       help="Path to generated.rs file")
    parser.add_argument("--output", required=True, help="Output JSON file path")

    args = parser.parse_args()

    if args.from_generated:
        success = export_from_generated(args.generated_path, args.output)
    elif args.input:
        success = export_from_numpy(args.input, args.output)
    else:
        print("Error: Must specify either --input or --from-generated", file=sys.stderr)
        parser.print_help()
        return 1

    return 0 if success else 1


if __name__ == "__main__":
    sys.exit(main())
