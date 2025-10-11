"""Quick test to verify imports work."""

import sys
print(f"Python version: {sys.version}")

try:
    import torch
    print(f"PyTorch version: {torch.__version__}")
    print(f"CUDA available: {torch.cuda.is_available()}")
except ImportError:
    print("PyTorch not installed. Run: pip install -r requirements.txt")
    sys.exit(1)

try:
    from hearts_rl.config import TrainingConfig
    from hearts_rl.model import ActorCritic
    from hearts_rl.dataset import ExperienceDataset
    from hearts_rl.trainer import PPOTrainer
    from hearts_rl.utils import compute_gae

    print("\n[OK] All imports successful")

    # Test model creation
    config = TrainingConfig()
    model = ActorCritic(
        obs_dim=config.obs_dim,
        action_dim=config.action_dim,
        hidden_dims=config.hidden_dims,
    )
    print(f"[OK] Model created: {sum(p.numel() for p in model.parameters())} parameters")

    # Test forward pass
    batch_size = 4
    obs = torch.randn(batch_size, config.obs_dim)
    logits, values = model(obs)
    print(f"[OK] Forward pass: logits shape {logits.shape}, values shape {values.shape}")

    # Test action sampling
    action, log_prob, entropy, value = model.get_action_and_value(obs)
    print(f"[OK] Action sampling: action shape {action.shape}, log_prob shape {log_prob.shape}")

    print("\n[OK] All tests passed!")

except Exception as e:
    print(f"\n[ERROR] Error: {e}")
    import traceback
    traceback.print_exc()
    sys.exit(1)
