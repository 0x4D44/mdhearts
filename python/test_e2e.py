"""End-to-end integration test for RL pipeline."""

import sys
import subprocess
from pathlib import Path
import json
import tempfile
import shutil


def test_experience_collection():
    """Test experience collection with self-play."""
    print("\n" + "=" * 60)
    print("Test 1: Experience Collection")
    print("=" * 60)

    # Find mdhearts executable
    candidates = [
        Path("target/release/mdhearts.exe"),
        Path("target/debug/mdhearts.exe"),
        Path("../target/release/mdhearts.exe"),
        Path("../target/debug/mdhearts.exe"),
    ]

    mdhearts = None
    for candidate in candidates:
        if candidate.exists():
            mdhearts = str(candidate)
            break

    if not mdhearts:
        print("[FAIL] mdhearts executable not found")
        return False

    print(f"Using: {mdhearts}")

    # Create temp file
    with tempfile.NamedTemporaryFile(mode='w', suffix='.jsonl', delete=False) as f:
        temp_path = f.name

    try:
        # Run collection
        cmd = [mdhearts, "eval", "5", "--self-play", "--collect-rl", temp_path, "--reward-mode", "shaped"]
        print(f"Running: {' '.join(cmd)}")

        result = subprocess.run(cmd, capture_output=True, text=True, timeout=60)

        if result.returncode != 0:
            print(f"[FAIL] Command failed: {result.stderr}")
            return False

        # Check file exists
        if not Path(temp_path).exists():
            print("[FAIL] Output file not created")
            return False

        # Count experiences
        with open(temp_path, 'r') as f:
            experiences = [json.loads(line) for line in f]

        print(f"[OK] Collected {len(experiences)} experiences")

        # Validate structure
        if len(experiences) == 0:
            print("[FAIL] No experiences collected")
            return False

        exp = experiences[0]
        required_fields = ['observation', 'action', 'reward', 'done', 'value', 'log_prob']
        for field in required_fields:
            if field not in exp:
                print(f"[FAIL] Missing field: {field}")
                return False

        print("[OK] Experience structure valid")
        return True

    finally:
        # Cleanup
        if Path(temp_path).exists():
            Path(temp_path).unlink()


def test_model_creation():
    """Test model creation and forward pass."""
    print("\n" + "=" * 60)
    print("Test 2: Model Creation")
    print("=" * 60)

    try:
        import torch
        from hearts_rl.model import ActorCritic

        model = ActorCritic(obs_dim=270, action_dim=52, hidden_dims=(256, 128))
        print(f"[OK] Model created: {sum(p.numel() for p in model.parameters())} parameters")

        # Test forward pass
        obs = torch.randn(4, 270)
        logits, values = model(obs)

        if logits.shape != (4, 52):
            print(f"[FAIL] Wrong logits shape: {logits.shape}")
            return False

        if values.shape != (4, 1):
            print(f"[FAIL] Wrong values shape: {values.shape}")
            return False

        print("[OK] Forward pass successful")
        return True

    except Exception as e:
        print(f"[FAIL] {e}")
        return False


def test_dataset_loading():
    """Test dataset loading and GAE computation."""
    print("\n" + "=" * 60)
    print("Test 3: Dataset Loading")
    print("=" * 60)

    try:
        from hearts_rl.dataset import ExperienceDataset

        # Create temp dataset
        with tempfile.NamedTemporaryFile(mode='w', suffix='.jsonl', delete=False) as f:
            temp_path = f.name

            # Write fake experiences
            for i in range(10):
                exp = {
                    'observation': [0.0] * 270,
                    'action': 0,
                    'reward': -0.1,
                    'done': i == 9,
                    'game_id': 0,
                    'step_id': i,
                    'seat': 0,
                    'value': 0.0,
                    'log_prob': -1.5,
                }
                f.write(json.dumps(exp) + '\n')

        try:
            dataset = ExperienceDataset(temp_path)

            if len(dataset) != 10:
                print(f"[FAIL] Wrong dataset length: {len(dataset)}")
                return False

            print(f"[OK] Dataset loaded: {len(dataset)} experiences")

            # Test GAE computation
            advantages, returns = dataset.compute_returns_and_advantages()

            if len(advantages) != 10:
                print(f"[FAIL] Wrong advantages length: {len(advantages)}")
                return False

            print(f"[OK] GAE computed: {len(advantages)} advantages")
            return True

        finally:
            Path(temp_path).unlink()

    except Exception as e:
        print(f"[FAIL] {e}")
        import traceback
        traceback.print_exc()
        return False


def test_weight_export():
    """Test weight export to JSON."""
    print("\n" + "=" * 60)
    print("Test 4: Weight Export")
    print("=" * 60)

    try:
        import torch
        from hearts_rl.model import ActorCritic

        model = ActorCritic(obs_dim=270, action_dim=52, hidden_dims=(256, 128))

        # Export weights
        weights = model.export_weights()

        # Check structure
        if 'layer1' not in weights:
            print("[FAIL] Missing layer1")
            return False

        if 'weights' not in weights['layer1'] or 'biases' not in weights['layer1']:
            print("[FAIL] Invalid layer structure")
            return False

        # Check dimensions
        layer1_weights = weights['layer1']['weights']
        layer1_biases = weights['layer1']['biases']

        if len(layer1_weights) != 256 * 270:
            print(f"[FAIL] Wrong layer1 weights length: {len(layer1_weights)}")
            return False

        if len(layer1_biases) != 256:
            print(f"[FAIL] Wrong layer1 biases length: {len(layer1_biases)}")
            return False

        print("[OK] Weight export successful")

        # Test JSON serialization
        with tempfile.NamedTemporaryFile(mode='w', suffix='.json', delete=False) as f:
            temp_path = f.name
            weights['schema_version'] = 1
            weights['schema_hash'] = "test"
            json.dump(weights, f)

        try:
            # Verify JSON can be loaded
            with open(temp_path, 'r') as f:
                loaded = json.load(f)

            print("[OK] JSON serialization successful")
            return True

        finally:
            Path(temp_path).unlink()

    except Exception as e:
        print(f"[FAIL] {e}")
        import traceback
        traceback.print_exc()
        return False


def main():
    """Run all integration tests."""
    print("=" * 60)
    print("Hearts RL Integration Tests")
    print("=" * 60)

    tests = [
        ("Experience Collection", test_experience_collection),
        ("Model Creation", test_model_creation),
        ("Dataset Loading", test_dataset_loading),
        ("Weight Export", test_weight_export),
    ]

    results = []
    for name, test_func in tests:
        try:
            result = test_func()
            results.append((name, result))
        except Exception as e:
            print(f"\n[ERROR] Test '{name}' crashed: {e}")
            import traceback
            traceback.print_exc()
            results.append((name, False))

    # Summary
    print("\n" + "=" * 60)
    print("Test Summary")
    print("=" * 60)

    passed = sum(1 for _, r in results if r)
    total = len(results)

    for name, result in results:
        status = "[PASS]" if result else "[FAIL]"
        print(f"{status} {name}")

    print(f"\nPassed: {passed}/{total}")

    if passed == total:
        print("\n[SUCCESS] All tests passed!")
        return 0
    else:
        print("\n[FAILURE] Some tests failed")
        return 1


if __name__ == '__main__':
    sys.exit(main())
