"""PPO Trainer for Hearts RL."""

import torch
import torch.nn as nn
import torch.optim as optim
from torch.utils.tensorboard import SummaryWriter
from pathlib import Path
from typing import Optional
import json

from .config import TrainingConfig
from .model import ActorCritic
from .dataset import ExperienceDataset
from .utils import (
    compute_ppo_loss,
    compute_value_loss,
    compute_entropy_loss,
    normalize_advantages,
    explained_variance,
)


class PPOTrainer:
    """PPO trainer with clipped objective."""

    def __init__(
        self,
        config: TrainingConfig,
        model: Optional[ActorCritic] = None,
        device: Optional[str] = None,
        bc_reference_path: Optional[str] = None,
    ):
        """Initialize PPO trainer.

        Args:
            config: Training configuration
            model: Optional pre-initialized model
            device: Optional device override
            bc_reference_path: Path to BC model for regularization (optional)
        """
        self.config = config
        self.device = device or config.device

        # Initialize model
        if model is None:
            self.model = ActorCritic(
                obs_dim=config.obs_dim,
                action_dim=config.action_dim,
                hidden_dims=config.hidden_dims,
            )
        else:
            self.model = model

        self.model.to(self.device)

        # Initialize BC reference model for regularization (Gen4+)
        self.bc_reference = None
        self.bc_reference_path = bc_reference_path
        if config.bc_lambda > 0.0 and bc_reference_path:
            print(f"Loading BC reference model from {bc_reference_path} for regularization...")
            self.bc_reference = ActorCritic(
                obs_dim=config.obs_dim,
                action_dim=config.action_dim,
                hidden_dims=config.hidden_dims,
            )
            self.bc_reference.to(self.device)
            self.bc_reference.eval()  # Freeze BC reference

            # Load BC weights
            with open(bc_reference_path, 'r') as f:
                weights = json.load(f)

            state_dict = {}
            layer1_weights = torch.tensor(weights['layer1']['weights']).reshape(256, 270)
            layer1_biases = torch.tensor(weights['layer1']['biases'])
            state_dict['trunk.0.weight'] = layer1_weights
            state_dict['trunk.0.bias'] = layer1_biases

            layer2_weights = torch.tensor(weights['layer2']['weights']).reshape(128, 256)
            layer2_biases = torch.tensor(weights['layer2']['biases'])
            state_dict['trunk.2.weight'] = layer2_weights
            state_dict['trunk.2.bias'] = layer2_biases

            layer3_weights = torch.tensor(weights['layer3']['weights']).reshape(52, 128)
            layer3_biases = torch.tensor(weights['layer3']['biases'])
            state_dict['actor_head.weight'] = layer3_weights
            state_dict['actor_head.bias'] = layer3_biases

            # Dummy critic (not used)
            state_dict['critic_head.weight'] = torch.zeros(1, 128)
            state_dict['critic_head.bias'] = torch.zeros(1)

            self.bc_reference.load_state_dict(state_dict)
            print(f"BC reference loaded (λ={config.bc_lambda})")
            print(f"Initial KL-divergence will be computed on first training batch")

        # Initialize optimizer
        self.optimizer = optim.Adam(self.model.parameters(), lr=config.learning_rate)

        # Initialize tensorboard writer
        self.writer = SummaryWriter(log_dir=config.log_dir)

        # Training state
        self.global_step = 0
        self.epoch = 0
        self.initial_kl_logged = False  # Track if we've logged initial KL-divergence

        # Create checkpoint directory
        Path(config.checkpoint_dir).mkdir(parents=True, exist_ok=True)

    def train_epoch(self, dataset: ExperienceDataset, advantages: torch.Tensor, returns: torch.Tensor) -> dict:
        """Train for one epoch on the dataset.

        Args:
            dataset: Experience dataset
            advantages: Precomputed advantages tensor (already normalized and on device)
            returns: Precomputed returns tensor (already on device)

        Returns:
            Dictionary of training metrics
        """
        self.model.train()

        # Training metrics
        total_loss = 0.0
        total_policy_loss = 0.0
        total_value_loss = 0.0
        total_entropy_loss = 0.0
        total_bc_reg_loss = 0.0
        total_grad_norm = 0.0
        num_batches = 0

        # Create batches
        num_experiences = len(dataset)
        indices = torch.randperm(num_experiences)
        batch_size = self.config.batch_size

        for start_idx in range(0, num_experiences, batch_size):
            end_idx = min(start_idx + batch_size, num_experiences)
            batch_indices = indices[start_idx:end_idx]

            # Get batch data
            batch_obs = torch.stack([dataset[i]['observation'] for i in batch_indices]).to(self.device)
            batch_actions = torch.stack([dataset[i]['action'] for i in batch_indices]).to(self.device)
            batch_old_log_probs = torch.stack([dataset[i]['log_prob'] for i in batch_indices]).to(self.device)
            batch_old_values = torch.stack([dataset[i]['value'] for i in batch_indices]).to(self.device)

            batch_advantages = advantages[batch_indices]
            batch_returns = returns[batch_indices]

            # Forward pass
            logits, values = self.model(batch_obs)
            values = values.squeeze(-1)

            # Compute log probabilities and entropy
            probs = torch.softmax(logits, dim=-1)
            dist = torch.distributions.Categorical(probs)
            new_log_probs = dist.log_prob(batch_actions)
            entropy = dist.entropy()

            # Compute losses
            policy_loss = compute_ppo_loss(
                old_log_probs=batch_old_log_probs,
                new_log_probs=new_log_probs,
                advantages=batch_advantages,
                clip_epsilon=self.config.clip_epsilon,
            )

            value_loss = compute_value_loss(
                predicted_values=values,
                target_returns=batch_returns,
                clip_value=False,  # Simple MSE for now
            )

            entropy_loss = compute_entropy_loss(entropy)

            # BC regularization loss (Gen4+)
            bc_reg_loss = torch.tensor(0.0, device=self.device)
            if self.bc_reference is not None and self.config.bc_lambda > 0.0:
                with torch.no_grad():
                    bc_logits, _ = self.bc_reference(batch_obs)

                # KL divergence between current policy and BC policy
                # KL(P_bc || P_current) = sum(P_bc * log(P_bc / P_current))
                bc_probs = torch.softmax(bc_logits, dim=-1)
                current_probs = torch.softmax(logits, dim=-1)

                bc_reg_loss = torch.nn.functional.kl_div(
                    torch.log(current_probs + 1e-8),
                    bc_probs,
                    reduction='batchmean'
                )

                # Log initial KL-divergence (before any training)
                if not self.initial_kl_logged:
                    print(f"Initial KL-divergence from BC reference: {bc_reg_loss.item():.6f}")
                    self.writer.add_scalar('BC/initial_kl_divergence', bc_reg_loss.item(), 0)
                    self.initial_kl_logged = True

            # Total loss
            loss = (
                policy_loss
                + self.config.value_coef * value_loss
                + self.config.entropy_coef * entropy_loss
                + self.config.bc_lambda * bc_reg_loss
            )

            # Backward pass
            self.optimizer.zero_grad()
            loss.backward()

            # Gradient clipping (and track gradient norm)
            grad_norm = nn.utils.clip_grad_norm_(self.model.parameters(), self.config.max_grad_norm)

            self.optimizer.step()

            # Update metrics
            total_loss += loss.item()
            total_policy_loss += policy_loss.item()
            total_value_loss += value_loss.item()
            total_entropy_loss += entropy_loss.item()
            total_bc_reg_loss += bc_reg_loss.item()
            total_grad_norm += grad_norm.item()
            num_batches += 1
            self.global_step += 1

        # Compute average metrics
        metrics = {
            'loss': total_loss / num_batches,
            'policy_loss': total_policy_loss / num_batches,
            'value_loss': total_value_loss / num_batches,
            'entropy_loss': total_entropy_loss / num_batches,
            'bc_reg_loss': total_bc_reg_loss / num_batches,
            'grad_norm': total_grad_norm / num_batches,
        }

        self.epoch += 1

        return metrics

    def train(self, data_path: str, num_iterations: int = 100):
        """Train for multiple iterations.

        Args:
            data_path: Path to JSONL experience file
            num_iterations: Number of training iterations
        """
        print(f"Loading dataset from {data_path}...")
        dataset = ExperienceDataset(data_path)

        print(f"Starting training for {num_iterations} iterations...")

        for iteration in range(num_iterations):
            # Compute GAE advantages and returns once per iteration
            print(f"Computing advantages for iteration {iteration + 1}...", flush=True)
            advantages, returns = dataset.compute_returns_and_advantages(
                gamma=self.config.gamma,
                gae_lambda=self.config.gae_lambda,
            )

            # Normalize advantages
            advantages = normalize_advantages(advantages)

            # Move to device
            advantages = advantages.to(self.device)
            returns = returns.to(self.device)

            # Train for num_epochs on the same data
            for epoch in range(self.config.num_epochs):
                metrics = self.train_epoch(dataset, advantages, returns)

                # Log metrics
                self.writer.add_scalar('Loss/total', metrics['loss'], self.global_step)
                self.writer.add_scalar('Loss/policy', metrics['policy_loss'], self.global_step)
                self.writer.add_scalar('Loss/value', metrics['value_loss'], self.global_step)
                self.writer.add_scalar('Loss/entropy', metrics['entropy_loss'], self.global_step)
                self.writer.add_scalar('Loss/bc_regularization', metrics['bc_reg_loss'], self.global_step)
                self.writer.add_scalar('Training/gradient_norm', metrics['grad_norm'], self.global_step)

                # Print progress
                log_msg = (
                    f"Iteration {iteration + 1}/{num_iterations}, "
                    f"Epoch {epoch + 1}/{self.config.num_epochs}, "
                    f"Loss: {metrics['loss']:.4f}, "
                    f"Policy: {metrics['policy_loss']:.4f}, "
                    f"Value: {metrics['value_loss']:.4f}"
                )
                if self.config.bc_lambda > 0.0:
                    log_msg += f", BC: {metrics['bc_reg_loss']:.4f}"
                log_msg += f", GradNorm: {metrics['grad_norm']:.4f}"
                print(log_msg, flush=True)

            # Save checkpoint
            if (iteration + 1) % self.config.save_interval == 0:
                self.save_checkpoint(iteration + 1)

        print("Training complete!")

    def save_checkpoint(self, iteration: int):
        """Save model checkpoint.

        Args:
            iteration: Current iteration number
        """
        checkpoint_path = Path(self.config.checkpoint_dir) / f"checkpoint_{iteration}.pt"

        checkpoint_data = {
            'iteration': iteration,
            'epoch': self.epoch,
            'global_step': self.global_step,
            'model_state_dict': self.model.state_dict(),
            'optimizer_state_dict': self.optimizer.state_dict(),
            'config': self.config.to_dict(),
        }

        # Include BC reference metadata if used
        if self.bc_reference is not None:
            checkpoint_data['bc_reference_path'] = self.bc_reference_path
            checkpoint_data['bc_lambda'] = self.config.bc_lambda

        torch.save(checkpoint_data, checkpoint_path)

        print(f"Saved checkpoint to {checkpoint_path}")

    def load_checkpoint(self, checkpoint_path: str):
        """Load model checkpoint.

        Args:
            checkpoint_path: Path to checkpoint file (.pt or .json)
        """
        checkpoint_path_obj = Path(checkpoint_path)

        # Check if it's a JSON weights file
        if checkpoint_path_obj.suffix == '.json':
            self.load_weights_from_json(checkpoint_path)
            return

        # Otherwise, load PyTorch checkpoint
        checkpoint = torch.load(checkpoint_path, map_location=self.device)

        self.model.load_state_dict(checkpoint['model_state_dict'])
        self.optimizer.load_state_dict(checkpoint['optimizer_state_dict'])
        self.epoch = checkpoint['epoch']
        self.global_step = checkpoint['global_step']

        print(f"Loaded checkpoint from {checkpoint_path}")
        print(f"Resuming from iteration {checkpoint['iteration']}, epoch {self.epoch}")

    def load_weights_from_json(self, json_path: str):
        """Load model weights from JSON file (BC or exported weights).

        Args:
            json_path: Path to JSON weights file
        """
        with open(json_path, 'r') as f:
            weights = json.load(f)

        # Build state dict from JSON weights
        state_dict = {}

        # Layer 1: 270 -> 256
        layer1_weights = torch.tensor(weights['layer1']['weights']).reshape(256, 270)
        layer1_biases = torch.tensor(weights['layer1']['biases'])
        state_dict['trunk.0.weight'] = layer1_weights
        state_dict['trunk.0.bias'] = layer1_biases

        # Layer 2: 256 -> 128
        layer2_weights = torch.tensor(weights['layer2']['weights']).reshape(128, 256)
        layer2_biases = torch.tensor(weights['layer2']['biases'])
        state_dict['trunk.2.weight'] = layer2_weights
        state_dict['trunk.2.bias'] = layer2_biases

        # Layer 3 (actor head): 128 -> 52
        layer3_weights = torch.tensor(weights['layer3']['weights']).reshape(52, 128)
        layer3_biases = torch.tensor(weights['layer3']['biases'])
        state_dict['actor_head.weight'] = layer3_weights
        state_dict['actor_head.bias'] = layer3_biases

        # Initialize critic head with small random weights (not trained yet)
        # This will be learned during RL training
        state_dict['critic_head.weight'] = torch.randn(1, 128) * 0.01
        state_dict['critic_head.bias'] = torch.zeros(1)

        # Load into model
        self.model.load_state_dict(state_dict)

        print(f"Loaded weights from {json_path}")
        print(f"Initialized actor from BC/exported weights, critic head randomly initialized")

    def export_weights(self, output_path: str, schema_version: str = None, schema_hash: str = None):
        """Export model weights to JSON format for Rust inference.

        Args:
            output_path: Path to output JSON file
            schema_version: Schema version (auto-detected if None)
            schema_hash: Schema hash (auto-detected if None)
        """
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
        """Get schema version and hash from mdhearts binary.

        Returns:
            Dictionary with schema_version and schema_hash
        """
        import subprocess
        from pathlib import Path

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

    def close(self):
        """Close tensorboard writer."""
        self.writer.close()
