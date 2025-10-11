"""Supervised learning trainer for behavioral cloning."""

import torch
import torch.nn as nn
import torch.optim as optim
from torch.utils.data import Dataset, DataLoader
from pathlib import Path
import json
from typing import Optional

from .model import ActorCritic


class SupervisedDataset(Dataset):
    """Dataset for supervised learning from expert demonstrations."""

    def __init__(self, jsonl_path: str):
        """Load experiences from JSONL file."""
        self.experiences = []

        with open(jsonl_path, 'r') as f:
            for line in f:
                exp = json.loads(line)
                self.experiences.append({
                    'observation': torch.tensor(exp['observation'], dtype=torch.float32),
                    'action': exp['action'],
                })

    def __len__(self):
        return len(self.experiences)

    def __getitem__(self, idx):
        return self.experiences[idx]


class SupervisedTrainer:
    """Trainer for behavioral cloning with supervised learning."""

    def __init__(
        self,
        model: Optional[ActorCritic] = None,
        learning_rate: float = 0.0003,
        device: str = 'cpu',
        obs_dim: int = 278,
        action_dim: int = 52,
    ):
        """Initialize supervised trainer.

        Args:
            model: Optional pre-initialized model
            learning_rate: Learning rate for optimizer
            device: Device to train on (cpu or cuda)
            obs_dim: Observation dimension (default: 278)
            action_dim: Action dimension (default: 52)
        """
        self.device = device

        # Initialize or use provided model
        if model is None:
            self.model = ActorCritic(
                obs_dim=obs_dim,
                action_dim=action_dim,
                hidden_dims=[256, 128],  # Match Rust embedded policy architecture
            )
        else:
            self.model = model

        self.model.to(self.device)

        # Optimizer
        self.optimizer = optim.Adam(self.model.parameters(), lr=learning_rate)

        # Loss function (cross-entropy for classification)
        self.criterion = nn.CrossEntropyLoss()

    def train_epoch(self, dataloader: DataLoader) -> dict:
        """Train for one epoch.

        Args:
            dataloader: DataLoader for training data

        Returns:
            Dictionary of training metrics
        """
        self.model.train()

        total_loss = 0.0
        total_correct = 0
        total_samples = 0

        for batch in dataloader:
            observations = batch['observation'].to(self.device)
            actions = batch['action'].to(self.device)

            # Forward pass (only need logits, not values)
            logits, _ = self.model(observations)

            # Compute loss
            loss = self.criterion(logits, actions)

            # Backward pass
            self.optimizer.zero_grad()
            loss.backward()
            self.optimizer.step()

            # Metrics
            total_loss += loss.item() * len(observations)

            # Accuracy
            _, predicted = torch.max(logits, 1)
            total_correct += (predicted == actions).sum().item()
            total_samples += len(observations)

        return {
            'loss': total_loss / total_samples,
            'accuracy': total_correct / total_samples,
        }

    def evaluate(self, dataloader: DataLoader) -> dict:
        """Evaluate on validation set.

        Args:
            dataloader: DataLoader for validation data

        Returns:
            Dictionary of evaluation metrics
        """
        self.model.eval()

        total_loss = 0.0
        total_correct = 0
        total_samples = 0

        with torch.no_grad():
            for batch in dataloader:
                observations = batch['observation'].to(self.device)
                actions = batch['action'].to(self.device)

                # Forward pass
                logits, _ = self.model(observations)

                # Compute loss
                loss = self.criterion(logits, actions)

                # Metrics
                total_loss += loss.item() * len(observations)

                # Accuracy
                _, predicted = torch.max(logits, 1)
                total_correct += (predicted == actions).sum().item()
                total_samples += len(observations)

        return {
            'loss': total_loss / total_samples,
            'accuracy': total_correct / total_samples,
        }

    def save_checkpoint(self, path: str):
        """Save model checkpoint."""
        torch.save({
            'model_state_dict': self.model.state_dict(),
            'optimizer_state_dict': self.optimizer.state_dict(),
        }, path)
        print(f"Saved checkpoint to {path}")

    def load_checkpoint(self, path: str):
        """Load model checkpoint."""
        checkpoint = torch.load(path, map_location=self.device)
        self.model.load_state_dict(checkpoint['model_state_dict'])
        self.optimizer.load_state_dict(checkpoint['optimizer_state_dict'])
        print(f"Loaded checkpoint from {path}")

    def export_weights(self, output_path: str, schema_version: str = None, schema_hash: str = None):
        """Export model weights to JSON format for Rust inference."""
        # Auto-detect schema info if not provided
        if schema_version is None or schema_hash is None:
            schema_info = self._get_schema_info()
            if schema_version is None:
                schema_version = schema_info['schema_version']
            if schema_hash is None:
                schema_hash = schema_info['schema_hash']

        weights = self.model.export_weights()

        # Add metadata
        weights['schema_version'] = schema_version
        weights['schema_hash'] = schema_hash

        # Write to file
        with open(output_path, 'w') as f:
            json.dump(weights, f)

        print(f"Exported weights to {output_path}")
        print(f"  Schema version: {schema_version}")
        print(f"  Schema hash: {schema_hash[:16]}...")

    def _get_schema_info(self):
        """Get schema version and hash from mdhearts binary."""
        import subprocess

        # Try to find mdhearts executable
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
            print("Warning: Could not find mdhearts binary, using defaults")
            return {"schema_version": "1.1.0", "schema_hash": ""}

        try:
            result = subprocess.run(
                [mdhearts, "--schema-info"],
                capture_output=True,
                text=True,
                timeout=5
            )

            if result.returncode == 0:
                return json.loads(result.stdout.strip())
        except Exception as e:
            print(f"Warning: Could not get schema info: {e}")

        return {"schema_version": "1.1.0", "schema_hash": ""}
