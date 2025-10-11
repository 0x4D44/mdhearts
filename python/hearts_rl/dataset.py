"""Dataset loader for RL experiences."""

import json
import torch
from torch.utils.data import Dataset, DataLoader
from typing import List, Dict, Tuple
import numpy as np


class ExperienceDataset(Dataset):
    """Dataset for loading RL experiences from JSONL file.

    Each experience contains:
        - observation: 270-dimensional feature vector
        - action: Card ID (0-51)
        - reward: Step or terminal reward
        - done: Terminal flag
        - game_id: Game identifier
        - step_id: Step within game
        - seat: Player seat (0-3)
        - value: Value estimate from old policy
        - log_prob: Log probability from old policy
    """

    def __init__(self, jsonl_path: str):
        """Initialize dataset from JSONL file.

        Args:
            jsonl_path: Path to JSONL experience file
        """
        self.experiences = []
        self._load_experiences(jsonl_path)

    def _load_experiences(self, jsonl_path: str):
        """Load experiences from JSONL file.

        Args:
            jsonl_path: Path to JSONL file
        """
        with open(jsonl_path, 'r') as f:
            for line in f:
                exp = json.loads(line.strip())
                self.experiences.append(exp)

        print(f"Loaded {len(self.experiences)} experiences from {jsonl_path}")

    def __len__(self) -> int:
        """Return number of experiences."""
        return len(self.experiences)

    def __getitem__(self, idx: int) -> Dict:
        """Get experience at index.

        Args:
            idx: Experience index

        Returns:
            Experience dictionary with tensors
        """
        exp = self.experiences[idx]

        return {
            'observation': torch.tensor(exp['observation'], dtype=torch.float32),
            'action': torch.tensor(exp['action'], dtype=torch.long),
            'reward': torch.tensor(exp['reward'], dtype=torch.float32),
            'done': torch.tensor(exp['done'], dtype=torch.float32),
            'game_id': exp['game_id'],
            'step_id': exp['step_id'],
            'seat': exp['seat'],
            'value': torch.tensor(exp['value'], dtype=torch.float32),
            'log_prob': torch.tensor(exp['log_prob'], dtype=torch.float32),
        }

    def group_by_episodes(self) -> List[List[Dict]]:
        """Group experiences by episode (game_id + seat).

        Returns:
            List of episodes, where each episode is a list of experiences
        """
        episodes = {}

        for exp in self.experiences:
            episode_key = (exp['game_id'], exp['seat'])
            if episode_key not in episodes:
                episodes[episode_key] = []
            episodes[episode_key].append(exp)

        # Sort experiences within each episode by step_id
        for episode_key in episodes:
            episodes[episode_key].sort(key=lambda x: x['step_id'])

        return list(episodes.values())

    def compute_returns_and_advantages(
        self,
        gamma: float = 0.99,
        gae_lambda: float = 0.95,
    ) -> Tuple[torch.Tensor, torch.Tensor]:
        """Compute GAE advantages and returns for all experiences.

        Args:
            gamma: Discount factor
            gae_lambda: GAE lambda parameter

        Returns:
            advantages: Tensor of advantages [num_experiences]
            returns: Tensor of returns [num_experiences]
        """
        from .utils import compute_episode_gae

        episodes = self.group_by_episodes()

        all_advantages = []
        all_returns = []

        for episode in episodes:
            episode_rewards = [exp['reward'] for exp in episode]
            episode_values = [exp['value'] for exp in episode]
            episode_dones = [float(exp['done']) for exp in episode]

            advantages, returns = compute_episode_gae(
                episode_rewards,
                episode_values,
                episode_dones,
                gamma,
                gae_lambda,
            )

            all_advantages.extend(advantages)
            all_returns.extend(returns)

        return (
            torch.tensor(all_advantages, dtype=torch.float32),
            torch.tensor(all_returns, dtype=torch.float32),
        )


def create_dataloader(
    jsonl_path: str,
    batch_size: int = 256,
    shuffle: bool = True,
    num_workers: int = 0,
) -> DataLoader:
    """Create DataLoader for experience dataset.

    Args:
        jsonl_path: Path to JSONL file
        batch_size: Batch size
        shuffle: Whether to shuffle data
        num_workers: Number of worker processes

    Returns:
        DataLoader instance
    """
    dataset = ExperienceDataset(jsonl_path)

    return DataLoader(
        dataset,
        batch_size=batch_size,
        shuffle=shuffle,
        num_workers=num_workers,
        pin_memory=True,
    )


def collate_experiences(batch: List[Dict]) -> Dict[str, torch.Tensor]:
    """Collate function for experience batches.

    Args:
        batch: List of experience dictionaries

    Returns:
        Batched dictionary with stacked tensors
    """
    return {
        'observation': torch.stack([item['observation'] for item in batch]),
        'action': torch.stack([item['action'] for item in batch]),
        'reward': torch.stack([item['reward'] for item in batch]),
        'done': torch.stack([item['done'] for item in batch]),
        'value': torch.stack([item['value'] for item in batch]),
        'log_prob': torch.stack([item['log_prob'] for item in batch]),
        'game_id': [item['game_id'] for item in batch],
        'step_id': [item['step_id'] for item in batch],
        'seat': [item['seat'] for item in batch],
    }
