"""Train model using supervised learning (behavioral cloning)."""

import argparse
import torch
from torch.utils.data import DataLoader, random_split

from hearts_rl.supervised_trainer import SupervisedDataset, SupervisedTrainer


def main():
    """Main training function."""
    parser = argparse.ArgumentParser(description="Train Hearts agent with supervised learning")

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
        default='bc_weights.json',
        help='Output path for exported weights (default: bc_weights.json)',
    )

    # Training arguments
    parser.add_argument(
        '--epochs',
        type=int,
        default=10,
        help='Number of training epochs (default: 10)',
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
        '--val-split',
        type=float,
        default=0.1,
        help='Validation split ratio (default: 0.1)',
    )

    # Device
    parser.add_argument(
        '--device',
        type=str,
        default='cpu',
        help='Device (cuda or cpu, default: cpu)',
    )

    args = parser.parse_args()

    print("=" * 60)
    print("Behavioral Cloning Training")
    print("=" * 60)
    print(f"Data: {args.data}")
    print(f"Device: {args.device}")
    print(f"Batch size: {args.batch_size}")
    print(f"Learning rate: {args.lr}")
    print(f"Epochs: {args.epochs}")
    print(f"Validation split: {args.val_split}")
    print("=" * 60)

    # Load dataset
    print(f"\nLoading dataset from {args.data}...")
    dataset = SupervisedDataset(args.data)
    print(f"Loaded {len(dataset)} experiences")

    # Auto-detect observation dimension
    obs_dim = len(dataset[0]['observation'])
    action_dim = 52  # Always 52 cards
    print(f"Detected observation dim: {obs_dim}, action dim: {action_dim}")

    # Split into train/val
    val_size = int(len(dataset) * args.val_split)
    train_size = len(dataset) - val_size
    train_dataset, val_dataset = random_split(dataset, [train_size, val_size])

    print(f"Train set: {len(train_dataset)} examples")
    print(f"Val set: {len(val_dataset)} examples")

    # Create data loaders
    train_loader = DataLoader(
        train_dataset,
        batch_size=args.batch_size,
        shuffle=True,
        num_workers=0,  # Single threaded for Windows compatibility
    )

    val_loader = DataLoader(
        val_dataset,
        batch_size=args.batch_size,
        shuffle=False,
        num_workers=0,
    )

    # Initialize trainer
    trainer = SupervisedTrainer(
        learning_rate=args.lr,
        device=args.device,
        obs_dim=obs_dim,
        action_dim=action_dim,
    )

    # Training loop
    print("\nStarting training...")
    best_val_loss = float('inf')

    for epoch in range(args.epochs):
        # Train
        train_metrics = trainer.train_epoch(train_loader)

        # Validate
        val_metrics = trainer.evaluate(val_loader)

        print(
            f"Epoch {epoch + 1}/{args.epochs} - "
            f"Train Loss: {train_metrics['loss']:.4f}, "
            f"Train Acc: {train_metrics['accuracy']:.2%}, "
            f"Val Loss: {val_metrics['loss']:.4f}, "
            f"Val Acc: {val_metrics['accuracy']:.2%}"
        )

        # Save best model
        if val_metrics['loss'] < best_val_loss:
            best_val_loss = val_metrics['loss']
            trainer.save_checkpoint('best_model.pt')

    # Export final weights
    print("\nExporting weights...")
    trainer.export_weights(args.output)

    print("\nTraining complete!")
    print(f"Weights exported to: {args.output}")
    print(f"Best validation loss: {best_val_loss:.4f}")


if __name__ == '__main__':
    main()
