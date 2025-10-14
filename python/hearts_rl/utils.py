"""Utility functions for PPO training."""

import torch
import numpy as np
from typing import Tuple


def compute_gae(
    rewards: torch.Tensor,
    values: torch.Tensor,
    dones: torch.Tensor,
    gamma: float = 0.99,
    gae_lambda: float = 0.95,
) -> Tuple[torch.Tensor, torch.Tensor]:
    """Compute Generalized Advantage Estimation (GAE).

    Args:
        rewards: Reward tensor [batch_size]
        values: Value estimates [batch_size]
        dones: Done flags [batch_size] (1 = terminal, 0 = non-terminal)
        gamma: Discount factor
        gae_lambda: GAE lambda parameter

    Returns:
        advantages: Advantage estimates [batch_size]
        returns: Discounted returns [batch_size]
    """
    batch_size = rewards.size(0)
    advantages = torch.zeros_like(rewards)
    last_gae = 0.0

    # Compute GAE in reverse order
    for t in reversed(range(batch_size)):
        if t == batch_size - 1:
            next_value = 0.0
        else:
            next_value = values[t + 1]

        # Mask next_value if episode terminated
        next_value = next_value * (1.0 - dones[t])

        # TD error: δ_t = r_t + γ * V(s_{t+1}) - V(s_t)
        delta = rewards[t] + gamma * next_value - values[t]

        # GAE: A_t = delta_t + (gamma * lambda) * A_{t+1}
        last_gae = delta + gamma * gae_lambda * (1.0 - dones[t]) * last_gae
        advantages[t] = last_gae

    # Returns are advantages + values
    returns = advantages + values

    return advantages, returns


def compute_episode_gae(
    episode_rewards: list,
    episode_values: list,
    episode_dones: list,
    gamma: float = 0.99,
    gae_lambda: float = 0.95,
) -> Tuple[list, list]:
    """Compute GAE for a single episode.

    Args:
        episode_rewards: List of rewards
        episode_values: List of value estimates
        episode_dones: List of done flags
        gamma: Discount factor
        gae_lambda: GAE lambda parameter

    Returns:
        advantages: List of advantage estimates
        returns: List of discounted returns
    """
    rewards = torch.tensor(episode_rewards, dtype=torch.float32)
    values = torch.tensor(episode_values, dtype=torch.float32)
    dones = torch.tensor(episode_dones, dtype=torch.float32)

    advantages, returns = compute_gae(rewards, values, dones, gamma, gae_lambda)

    return advantages.tolist(), returns.tolist()


def normalize_advantages(advantages: torch.Tensor, eps: float = 1e-8) -> torch.Tensor:
    """Normalize advantages to have mean 0 and std 1.

    Args:
        advantages: Advantage tensor
        eps: Small epsilon for numerical stability

    Returns:
        Normalized advantages
    """
    return (advantages - advantages.mean()) / (advantages.std() + eps)


def explained_variance(y_pred: torch.Tensor, y_true: torch.Tensor) -> float:
    """Compute explained variance.

    Args:
        y_pred: Predicted values
        y_true: True values

    Returns:
        Explained variance ratio
    """
    var_y = torch.var(y_true)
    return 1.0 - torch.var(y_true - y_pred) / (var_y + 1e-8)


def compute_ppo_loss(
    old_log_probs: torch.Tensor,
    new_log_probs: torch.Tensor,
    advantages: torch.Tensor,
    clip_epsilon: float = 0.2,
) -> torch.Tensor:
    """Compute PPO clipped surrogate loss.

    Args:
        old_log_probs: Log probabilities from old policy
        new_log_probs: Log probabilities from new policy
        advantages: Advantage estimates
        clip_epsilon: PPO clipping parameter

    Returns:
        PPO loss (to be minimized)
    """
    # Probability ratio: π_new / π_old
    ratio = torch.exp(new_log_probs - old_log_probs)

    # Unclipped objective
    surr1 = ratio * advantages

    # Clipped objective
    surr2 = torch.clamp(ratio, 1.0 - clip_epsilon, 1.0 + clip_epsilon) * advantages

    # PPO loss is negative of minimum (we minimize loss)
    ppo_loss = -torch.min(surr1, surr2).mean()

    return ppo_loss


def compute_value_loss(
    predicted_values: torch.Tensor,
    target_returns: torch.Tensor,
    clip_value: bool = False,
    clip_epsilon: float = 0.2,
    old_values: torch.Tensor = None,
) -> torch.Tensor:
    """Compute value function loss.

    Args:
        predicted_values: Predicted value estimates
        target_returns: Target returns (from GAE)
        clip_value: Whether to clip value loss
        clip_epsilon: Clipping parameter (if clip_value=True)
        old_values: Old value estimates (if clip_value=True)

    Returns:
        Value loss (MSE)
    """
    if clip_value and old_values is not None:
        # Clipped value loss (as in OpenAI baselines)
        value_pred_clipped = old_values + torch.clamp(
            predicted_values - old_values, -clip_epsilon, clip_epsilon
        )
        value_loss_unclipped = (predicted_values - target_returns).pow(2)
        value_loss_clipped = (value_pred_clipped - target_returns).pow(2)
        value_loss = 0.5 * torch.max(value_loss_unclipped, value_loss_clipped).mean()
    else:
        # Standard MSE loss
        value_loss = 0.5 * (predicted_values - target_returns).pow(2).mean()

    return value_loss


def compute_entropy_loss(entropy: torch.Tensor) -> torch.Tensor:
    """Compute entropy bonus (negative for maximization).

    Args:
        entropy: Policy entropy

    Returns:
        Negative entropy (to encourage exploration)
    """
    return -entropy.mean()
