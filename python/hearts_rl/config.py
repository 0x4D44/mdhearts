"""Training configuration for PPO."""

from dataclasses import dataclass
from typing import Optional


@dataclass
class TrainingConfig:
    """PPO training hyperparameters and settings."""

    # Model architecture
    obs_dim: int = 270
    action_dim: int = 52
    hidden_dims: tuple[int, ...] = (256, 128)

    # PPO hyperparameters
    clip_epsilon: float = 0.2
    value_coef: float = 0.5
    entropy_coef: float = 0.01
    gamma: float = 0.99
    gae_lambda: float = 0.95

    # Training settings
    learning_rate: float = 3e-4
    batch_size: int = 256
    num_epochs: int = 4
    max_grad_norm: float = 0.5

    # BC regularization (Gen4+)
    bc_lambda: float = 0.0  # Coefficient for BC regularization loss (0 = disabled)

    # Data settings
    data_path: str = "experiences.jsonl"
    checkpoint_dir: str = "checkpoints"
    log_dir: str = "runs"

    # Validation settings
    eval_interval: int = 10
    save_interval: int = 50

    # Device settings
    device: str = "cuda"  # "cuda" or "cpu"

    # Schema validation
    schema_version: int = 1
    schema_hash: str = ""

    @classmethod
    def from_dict(cls, config_dict: dict) -> "TrainingConfig":
        """Create config from dictionary."""
        return cls(**{k: v for k, v in config_dict.items() if k in cls.__dataclass_fields__})

    def to_dict(self) -> dict:
        """Convert config to dictionary."""
        return {
            "obs_dim": self.obs_dim,
            "action_dim": self.action_dim,
            "hidden_dims": self.hidden_dims,
            "clip_epsilon": self.clip_epsilon,
            "value_coef": self.value_coef,
            "entropy_coef": self.entropy_coef,
            "gamma": self.gamma,
            "gae_lambda": self.gae_lambda,
            "learning_rate": self.learning_rate,
            "batch_size": self.batch_size,
            "num_epochs": self.num_epochs,
            "max_grad_norm": self.max_grad_norm,
            "bc_lambda": self.bc_lambda,
            "data_path": self.data_path,
            "checkpoint_dir": self.checkpoint_dir,
            "log_dir": self.log_dir,
            "eval_interval": self.eval_interval,
            "save_interval": self.save_interval,
            "device": self.device,
            "schema_version": self.schema_version,
            "schema_hash": self.schema_hash,
        }
