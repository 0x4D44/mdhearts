"""Export weights from a specific checkpoint."""

import sys
import torch
from hearts_rl.config import TrainingConfig
from hearts_rl.trainer import PPOTrainer

def export_checkpoint(checkpoint_path: str, output_path: str):
    """Export weights from checkpoint to JSON."""

    # Create default config (will be overridden by checkpoint)
    config = TrainingConfig(
        data_path="dummy.jsonl",
        batch_size=256,
        learning_rate=0.0003,
        num_epochs=4,
        device='cpu',
    )

    # Create trainer and load checkpoint
    trainer = PPOTrainer(config, device='cpu')
    trainer.load_checkpoint(checkpoint_path)

    # Export weights
    trainer.export_weights(output_path)

    print(f"\nSuccessfully exported weights from {checkpoint_path} to {output_path}")

if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("Usage: python export_checkpoint.py <checkpoint_path> <output_path>")
        sys.exit(1)

    checkpoint_path = sys.argv[1]
    output_path = sys.argv[2]

    export_checkpoint(checkpoint_path, output_path)
