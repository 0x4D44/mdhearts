"""Actor-Critic neural network for Hearts."""

import torch
import torch.nn as nn
from typing import Tuple


class ActorCritic(nn.Module):
    """Actor-Critic network with shared trunk.

    Architecture:
        Input: 270 features (observation)
        Hidden1: 256 units (ReLU)
        Hidden2: 128 units (ReLU)
        Actor head: 52 units (card logits)
        Critic head: 1 unit (value estimate)
    """

    def __init__(self, obs_dim: int = 270, action_dim: int = 52, hidden_dims: Tuple[int, ...] = (256, 128)):
        """Initialize actor-critic network.

        Args:
            obs_dim: Observation dimension (default: 270)
            action_dim: Action dimension (default: 52)
            hidden_dims: Hidden layer dimensions (default: (256, 128))
        """
        super().__init__()

        self.obs_dim = obs_dim
        self.action_dim = action_dim
        self.hidden_dims = hidden_dims

        # Shared trunk
        layers = []
        prev_dim = obs_dim
        for hidden_dim in hidden_dims:
            layers.append(nn.Linear(prev_dim, hidden_dim))
            layers.append(nn.ReLU())
            prev_dim = hidden_dim

        self.trunk = nn.Sequential(*layers)

        # Actor head (policy)
        self.actor_head = nn.Linear(prev_dim, action_dim)

        # Critic head (value function)
        self.critic_head = nn.Linear(prev_dim, 1)

        # Initialize weights
        self._init_weights()

    def _init_weights(self):
        """Initialize network weights using orthogonal initialization."""
        for module in self.modules():
            if isinstance(module, nn.Linear):
                nn.init.orthogonal_(module.weight, gain=1.0)
                nn.init.constant_(module.bias, 0.0)

    def forward(self, obs: torch.Tensor, mask: torch.Tensor = None) -> Tuple[torch.Tensor, torch.Tensor]:
        """Forward pass through network.

        Args:
            obs: Observation tensor [batch_size, obs_dim]
            mask: Legal move mask [batch_size, action_dim] (1 = legal, 0 = illegal)

        Returns:
            logits: Action logits [batch_size, action_dim]
            value: Value estimate [batch_size, 1]
        """
        # Shared trunk
        features = self.trunk(obs)

        # Actor head (logits)
        logits = self.actor_head(features)

        # Apply legal move mask if provided
        if mask is not None:
            logits = logits.masked_fill(mask == 0, float('-inf'))

        # Critic head (value)
        value = self.critic_head(features)

        return logits, value

    def get_action_and_value(
        self, obs: torch.Tensor, mask: torch.Tensor = None, action: torch.Tensor = None
    ) -> Tuple[torch.Tensor, torch.Tensor, torch.Tensor, torch.Tensor]:
        """Get action, log probability, entropy, and value.

        Args:
            obs: Observation tensor [batch_size, obs_dim]
            mask: Legal move mask [batch_size, action_dim]
            action: Optional action tensor [batch_size] for computing log_prob

        Returns:
            action: Sampled action [batch_size]
            log_prob: Log probability of action [batch_size]
            entropy: Policy entropy [batch_size]
            value: Value estimate [batch_size]
        """
        logits, value = self.forward(obs, mask)

        # Create categorical distribution
        probs = torch.softmax(logits, dim=-1)
        dist = torch.distributions.Categorical(probs)

        # Sample action if not provided
        if action is None:
            action = dist.sample()

        # Compute log probability and entropy
        log_prob = dist.log_prob(action)
        entropy = dist.entropy()

        return action, log_prob, entropy, value.squeeze(-1)

    def get_value(self, obs: torch.Tensor) -> torch.Tensor:
        """Get value estimate only.

        Args:
            obs: Observation tensor [batch_size, obs_dim]

        Returns:
            value: Value estimate [batch_size]
        """
        features = self.trunk(obs)
        value = self.critic_head(features)
        return value.squeeze(-1)

    def export_weights(self) -> dict:
        """Export weights in format compatible with Rust inference.

        Returns:
            Dictionary with layer weights and biases
        """
        state_dict = self.state_dict()

        # Extract layer weights
        # Assuming hidden_dims = (256, 128)
        layer1_weight = state_dict['trunk.0.weight'].detach().cpu().numpy()  # [256, 270]
        layer1_bias = state_dict['trunk.0.bias'].detach().cpu().numpy()      # [256]

        layer2_weight = state_dict['trunk.2.weight'].detach().cpu().numpy()  # [128, 256]
        layer2_bias = state_dict['trunk.2.bias'].detach().cpu().numpy()      # [128]

        layer3_weight = state_dict['actor_head.weight'].detach().cpu().numpy()  # [52, 128]
        layer3_bias = state_dict['actor_head.bias'].detach().cpu().numpy()      # [52]

        return {
            "layer1": {
                "weights": layer1_weight.flatten().tolist(),
                "biases": layer1_bias.tolist(),
            },
            "layer2": {
                "weights": layer2_weight.flatten().tolist(),
                "biases": layer2_bias.tolist(),
            },
            "layer3": {
                "weights": layer3_weight.flatten().tolist(),
                "biases": layer3_bias.tolist(),
            },
        }
