# Hearts RL Training

PPO-based reinforcement learning for the Hearts card game.

## Installation

```bash
pip install -r requirements.txt
```

## Quick Start

### Automated Pipeline

Run the full training pipeline with one command:

```bash
# Windows
train_pipeline.bat

# Or manually
cd python
python -m hearts_rl.orchestrator --collection-games 1000 --training-iterations 100
```

For a quick test run:

```bash
train_pipeline.bat --quick
```

### Manual Steps

#### 1. Collect Experiences

```bash
# From Rust binary
mdhearts eval 1000 --self-play --collect-rl experiences.jsonl --reward-mode shaped
```

#### 2. Train PPO Model

```bash
python -m hearts_rl.train --data experiences.jsonl --output weights.json --iterations 100
```

#### 3. Evaluate Policy

```bash
python -m hearts_rl.evaluate --mode compare --games 100 --weights weights.json --baseline normal
```

## Project Structure

```
python/
├── hearts_rl/
│   ├── __init__.py      # Package initialization
│   ├── config.py        # Training configuration
│   ├── model.py         # Actor-critic network
│   ├── dataset.py       # Experience dataset loader
│   ├── trainer.py       # PPO training loop
│   ├── utils.py         # GAE computation utilities
│   └── train.py         # Training entry point
├── requirements.txt     # Python dependencies
└── README.md           # This file
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
