# Hearts RL Training

PPO-based reinforcement learning for the Hearts card game.

## Installation

```
python/
  hearts_rl/
    __init__.py      # Package initialization
    config.py        # Training configuration
    model.py         # Actor-critic network
    dataset.py       # Experience dataset loader
    trainer.py       # PPO training loop
    utils.py         # GAE computation utilities
    train.py         # Training entry point
  requirements.txt  # Python dependencies
  README.md         # This file
```


## Configuration

Edit `config.py` to adjust hyperparameters:

- **Architecture**: 270 → 256 → 128 → 52 (actor) + 1 (critic)
- **PPO clip epsilon**: 0.2
- **Learning rate**: 3e-4
- **Batch size**: 256
- **GAE lambda**: 0.95
- **Gamma**: 0.99

## Monitoring

Training metrics are logged to TensorBoard:

```bash
tensorboard --logdir runs
```

## Weight Export

Weights are exported in JSON format:

```json
{
  "schema_version": 1,
  "schema_hash": "abc123...",
  "layer1": {"weights": [...], "biases": [...]},
  "layer2": {"weights": [...], "biases": [...]},
  "layer3": {"weights": [...], "biases": [...]}
}
```
