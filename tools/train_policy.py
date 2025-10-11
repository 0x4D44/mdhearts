#!/usr/bin/env python3
"""
Train a neural network policy for Hearts using behavioral cloning.

This script trains an MLP to imitate the heuristic AI policy by
learning from collected game experiences.
"""

import json
import numpy as np
import argparse
from pathlib import Path

# Try to import PyTorch, fall back to numpy-only mode if not available
try:
    import torch
    import torch.nn as nn
    import torch.optim as optim
    from torch.utils.data import Dataset, DataLoader
    HAS_TORCH = True
except ImportError:
    print("Warning: PyTorch not found. Install with: pip install torch")
    HAS_TORCH = False


class ExperienceDataset(Dataset):
    """Dataset for loading Hearts experiences from JSONL format."""

    def __init__(self, jsonl_path):
        self.experiences = []

        with open(jsonl_path, 'r') as f:
            for line in f:
                exp = json.loads(line.strip())
                self.experiences.append(exp)

        print(f"Loaded {len(self.experiences)} experiences from {jsonl_path}")

    def __len__(self):
        return len(self.experiences)

    def __getitem__(self, idx):
        exp = self.experiences[idx]
        obs = torch.FloatTensor(exp['observation'])
        action = torch.LongTensor([exp['action']])
        return obs, action


class HeartsPolicy(nn.Module):
    """MLP policy matching the architecture in embedded.rs (270 -> 256 -> 128 -> 52)."""

    def __init__(self):
        super().__init__()
        self.fc1 = nn.Linear(270, 256)
        self.fc2 = nn.Linear(256, 128)
        self.fc3 = nn.Linear(128, 52)
        self.relu = nn.ReLU()

    def forward(self, x):
        x = self.relu(self.fc1(x))
        x = self.relu(self.fc2(x))
        x = self.fc3(x)  # No activation on output (logits)
        return x


def train(data_path, output_path, epochs=50, batch_size=32, lr=0.001):
    """Train the policy using behavioral cloning (supervised learning)."""

    if not HAS_TORCH:
        print("Error: PyTorch is required for training")
        return False

    # Load data
    dataset = ExperienceDataset(data_path)
    dataloader = DataLoader(dataset, batch_size=batch_size, shuffle=True)

    # Create model
    model = HeartsPolicy()
    criterion = nn.CrossEntropyLoss()
    optimizer = optim.Adam(model.parameters(), lr=lr)

    # Training loop
    print(f"\nTraining for {epochs} epochs...")
    for epoch in range(epochs):
        total_loss = 0.0
        correct = 0
        total = 0

        for batch_idx, (obs, actions) in enumerate(dataloader):
            optimizer.zero_grad()

            # Forward pass
            logits = model(obs)
            loss = criterion(logits, actions.squeeze())

            # Backward pass
            loss.backward()
            optimizer.step()

            # Track metrics
            total_loss += loss.item()
            pred = logits.argmax(dim=1)
            correct += (pred == actions.squeeze()).sum().item()
            total += actions.size(0)

        avg_loss = total_loss / len(dataloader)
        accuracy = 100.0 * correct / total

        if (epoch + 1) % 5 == 0 or epoch == 0:
            print(f"Epoch {epoch+1}/{epochs}: Loss={avg_loss:.4f}, Accuracy={accuracy:.2f}%")

    # Save model weights as numpy arrays
    print(f"\nSaving trained weights to {output_path}")
    weights = {
        'layer1_weights': model.fc1.weight.detach().cpu().numpy(),
        'layer1_biases': model.fc1.bias.detach().cpu().numpy(),
        'layer2_weights': model.fc2.weight.detach().cpu().numpy(),
        'layer2_biases': model.fc2.bias.detach().cpu().numpy(),
        'layer3_weights': model.fc3.weight.detach().cpu().numpy(),
        'layer3_biases': model.fc3.bias.detach().cpu().numpy(),
    }

    np.savez(output_path, **weights)
    print(f"✓ Training complete. Final accuracy: {accuracy:.2f}%")
    return True


def main():
    parser = argparse.ArgumentParser(description='Train Hearts policy with behavioral cloning')
    parser.add_argument('data', type=str, help='Path to training data (JSONL format)')
    parser.add_argument('-o', '--output', type=str, default='trained_policy.npz',
                       help='Output path for trained weights (.npz format)')
    parser.add_argument('-e', '--epochs', type=int, default=50,
                       help='Number of training epochs (default: 50)')
    parser.add_argument('-b', '--batch-size', type=int, default=32,
                       help='Batch size (default: 32)')
    parser.add_argument('--lr', type=float, default=0.001,
                       help='Learning rate (default: 0.001)')

    args = parser.parse_args()

    if not Path(args.data).exists():
        print(f"Error: Data file not found: {args.data}")
        return 1

    success = train(
        data_path=args.data,
        output_path=args.output,
        epochs=args.epochs,
        batch_size=args.batch_size,
        lr=args.lr
    )

    return 0 if success else 1


if __name__ == '__main__':
    exit(main())
