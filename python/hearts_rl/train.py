"""Training entry point for Hearts PPO."""

import argparse
import torch
from pathlib import Path

from .config import TrainingConfig
from .model import ActorCritic
from .trainer import PPOTrainer


def main():
    """Main training function."""
    parser = argparse.ArgumentParser(description="Train Hearts RL agent with PPO")

    # Data arguments
    parser.add_argument(
        '--data',
        type=str,
        required=True,
        help='Path to JSONL experience file',
    )
    parser.add_argument(
        '--output',
        type=str,
        default='weights.json',
        help='Output path for exported weights (default: weights.json)',
    )

    # Training arguments
    parser.add_argument(
        '--iterations',
        type=int,
        default=100,
        help='Number of training iterations (default: 100)',
    )
    parser.add_argument(
        '--batch-size',
        type=int,
        default=256,
        help='Batch size (default: 256)',
    )
    parser.add_argument(
        '--lr',
        type=float,
        default=3e-4,
        help='Learning rate (default: 3e-4)',
    )
    parser.add_argument(
        '--epochs',
        type=int,
        default=4,
        help='Number of epochs per iteration (default: 4)',
    )

    # PPO hyperparameters
    parser.add_argument(
        '--clip-epsilon',
        type=float,
        default=0.2,
        help='PPO clip epsilon (default: 0.2)',
    )
    parser.add_argument(
        '--gamma',
        type=float,
        default=0.99,
        help='Discount factor (default: 0.99)',
    )
    parser.add_argument(
        '--gae-lambda',
        type=float,
        default=0.95,
        help='GAE lambda (default: 0.95)',
    )

    # BC regularization arguments (Gen4+)
    parser.add_argument(
        '--bc-lambda',
        type=float,
        default=0.0,
        help='BC regularization coefficient (default: 0.0 = disabled)',
    )
    parser.add_argument(
        '--bc-reference',
        type=str,
        default=None,
        help='Path to BC reference model for regularization (JSON weights)',
    )

    # Checkpoint arguments
    parser.add_argument(
        '--checkpoint-dir',
        type=str,
        default='checkpoints',
        help='Checkpoint directory (default: checkpoints)',
    )
    parser.add_argument(
        '--log-dir',
        type=str,
        default='runs',
        help='TensorBoard log directory (default: runs)',
    )
    parser.add_argument(
        '--save-interval',
        type=int,
        default=50,
        help='Save checkpoint every N iterations (default: 50)',
    )
    parser.add_argument(
        '--resume',
        type=str,
        default=None,
        help='Resume from checkpoint',
    )

    # Schema validation (auto-detected from Rust binary)

    # Device
    parser.add_argument(
        '--device',
        type=str,
        default='cuda' if torch.cuda.is_available() else 'cpu',
        help='Device (cuda or cpu)',
    )

    args = parser.parse_args()

    # Create config
    config = TrainingConfig(
        data_path=args.data,
        batch_size=args.batch_size,
        learning_rate=args.lr,
        num_epochs=args.epochs,
        clip_epsilon=args.clip_epsilon,
        gamma=args.gamma,
        gae_lambda=args.gae_lambda,
        bc_lambda=args.bc_lambda,
        checkpoint_dir=args.checkpoint_dir,
        log_dir=args.log_dir,
        save_interval=args.save_interval,
        device=args.device,
    )

    print("=" * 60)
    print("Hearts PPO Training")
    print("=" * 60)
    print(f"Data: {args.data}")
    print(f"Device: {config.device}")
    print(f"Batch size: {config.batch_size}")
    print(f"Learning rate: {config.learning_rate}")
    print(f"Iterations: {args.iterations}")
    print(f"Epochs per iteration: {config.num_epochs}")
    print(f"Clip epsilon: {config.clip_epsilon}")
    print(f"Gamma: {config.gamma}")
    print(f"GAE lambda: {config.gae_lambda}")
    if config.bc_lambda > 0.0:
        print(f"BC regularization: lambda={config.bc_lambda}")
        if args.bc_reference:
            print(f"BC reference: {args.bc_reference}")
    print("=" * 60)

    # Initialize trainer
    trainer = PPOTrainer(config, bc_reference_path=args.bc_reference)

    # Resume from checkpoint if specified
    if args.resume:
        trainer.load_checkpoint(args.resume)

    # Train
    try:
        trainer.train(args.data, num_iterations=args.iterations)
    except KeyboardInterrupt:
        print("\nTraining interrupted by user")

    # Export final weights
    print("\nExporting weights...")
    trainer.export_weights(args.output)

    # Close trainer
    trainer.close()

    print("\nTraining complete!")
    print(f"Weights exported to: {args.output}")
    print(f"Checkpoints saved to: {args.checkpoint_dir}")
    print(f"TensorBoard logs: {args.log_dir}")


if __name__ == '__main__':
    main()
