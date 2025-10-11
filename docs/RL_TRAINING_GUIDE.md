# Hearts RL Training Guide

Complete guide for training reinforcement learning agents for Hearts using PPO.

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Pipeline Components](#pipeline-components)
- [Training Process](#training-process)
- [Evaluation](#evaluation)
- [Hyperparameter Tuning](#hyperparameter-tuning)
- [Troubleshooting](#troubleshooting)

## Overview

The Hearts RL training pipeline implements **Proximal Policy Optimization (PPO)** with:

- **Self-play data collection** in Rust (fast game simulation)
- **PPO training** in Python with PyTorch
- **Weight export/import** between Python and Rust
- **Automated evaluation** against baseline policies

### Architecture

```
Observation (270 features)
         ↓
   Hidden Layer 1 (256, ReLU)
         ↓
   Hidden Layer 2 (128, ReLU)
         ↓
    ┌─────┴─────┐
    ↓           ↓
Actor Head   Critic Head
 (52 logits)  (1 value)
```

**Total Parameters**: 109,109

## Quick Start

### 1. Install Dependencies

```bash
pip install -r python/requirements.txt
```

### 2. Build Rust Binary

```bash
cargo build --release
```

### 3. Run Quick Test

```bash
# Windows
train_pipeline.bat --quick

# Manual (from python/ directory)
python -m hearts_rl.orchestrator --collection-games 100 --training-iterations 10
```

This will:
1. Collect 100 games of experience (~5,200 steps)
2. Train for 10 iterations
3. Export weights
4. Evaluate against baseline

## Pipeline Components

### 1. Experience Collection (Rust)

```bash
mdhearts eval 1000 --self-play --collect-rl experiences.jsonl --reward-mode shaped
```

**Parameters**:
- `num_games`: Number of complete games
- `--self-play`: Use same policy for all 4 seats (4x data efficiency)
- `--collect-rl <path>`: Output JSONL file path
- `--reward-mode <mode>`: Reward shaping mode
  - `shaped`: Immediate feedback when winning tricks (recommended)
  - `per_trick`: Reward after each trick completion
  - `terminal`: Only reward at episode end

**Output**: JSONL file with experiences

```json
{
  "observation": [270 floats],
  "action": 0-51 (card ID),
  "reward": -1.0 to 0.0,
  "done": true/false,
  "game_id": 0,
  "step_id": 0,
  "seat": 0-3,
  "value": 0.0 (placeholder),
  "log_prob": -1.5 (log probability)
}
```

### 2. PPO Training (Python)

```bash
python -m hearts_rl.train \
    --data experiences.jsonl \
    --output weights.json \
    --iterations 100 \
    --batch-size 256 \
    --lr 3e-4
```

**Parameters**:
- `--data`: Path to JSONL experiences
- `--output`: Path for exported weights
- `--iterations`: Number of training iterations
- `--batch-size`: Batch size (default: 256)
- `--lr`: Learning rate (default: 3e-4)
- `--epochs`: Epochs per iteration (default: 4)
- `--clip-epsilon`: PPO clip parameter (default: 0.2)
- `--gamma`: Discount factor (default: 0.99)
- `--gae-lambda`: GAE lambda (default: 0.95)

**Training Loop**:
1. Load experiences from JSONL
2. Compute GAE advantages and returns
3. For each iteration:
   - For each epoch:
     - Shuffle data
     - Train on mini-batches
     - Compute PPO loss, value loss, entropy loss
   - Save checkpoint every N iterations
4. Export final weights to JSON

### 3. Weight Export/Import

**Export (Python → JSON)**:

```python
from hearts_rl.trainer import PPOTrainer

trainer = PPOTrainer(config)
trainer.export_weights("weights.json", schema_version=1, schema_hash="abc123")
```

**Import (JSON → Rust)**:

```rust
use hearts_app::policy::EmbeddedPolicy;

let policy = EmbeddedPolicy::from_file("weights.json")?;
```

### 4. Evaluation

```bash
python -m hearts_rl.evaluate \
    --mode compare \
    --games 100 \
    --weights weights.json \
    --baseline normal
```

**Modes**:
- `compare`: Compare trained vs baseline
- `benchmark`: Self-play benchmark

**Output**:
```
Baseline (normal) average points per seat:
  Seat 0: 6.50
  Seat 1: 6.50
  Seat 2: 6.50
  Seat 3: 6.50

Trained policy average points per seat:
  Seat 0: 5.20
  Seat 1: 5.20
  Seat 2: 5.20
  Seat 3: 5.20

Improvement: 1.30 points (20.0%)
```

## Training Process

### Recommended Workflow

#### Phase 1: Warmstart (Behavioral Cloning)

1. Collect experiences from heuristic policy:
```bash
mdhearts eval 5000 --ai normal --collect-data bc_data.jsonl
```

2. Train with behavioral cloning first (optional, future work)

#### Phase 2: Self-Play Training

1. **Initial Collection** (1000 games):
```bash
mdhearts eval 1000 --self-play --collect-rl exp_v1.jsonl --reward-mode shaped
```

2. **Train PPO** (100 iterations):
```bash
python -m hearts_rl.train --data exp_v1.jsonl --output weights_v1.json --iterations 100
```

3. **Evaluate**:
```bash
python -m hearts_rl.evaluate --mode compare --games 100 --weights weights_v1.json --baseline normal
```

4. **Iterate**:
   - If improvement > 10%: Collect more data with new policy
   - If improvement < 5%: Adjust hyperparameters or increase data

#### Phase 3: Iterative Improvement

Use trained policy to collect better data:

```bash
# Collect with trained policy
mdhearts eval 1000 --self-play --collect-rl exp_v2.jsonl --weights weights_v1.json

# Train on new data
python -m hearts_rl.train --data exp_v2.jsonl --output weights_v2.json --iterations 100

# Evaluate
python -m hearts_rl.evaluate --mode compare --games 100 --weights weights_v2.json --baseline normal
```

### Automated Pipeline

Run everything with one command:

```bash
python -m hearts_rl.orchestrator \
    --collection-games 1000 \
    --training-iterations 100 \
    --eval-games 100 \
    --reward-mode shaped \
    --baseline normal \
    --run-name experiment_1
```

Output structure:
```
training_runs/experiment_1/
├── experiences.jsonl       # Collected data
├── weights.json           # Trained weights
├── checkpoints/          # Training checkpoints
├── logs/                # TensorBoard logs
└── metadata.json        # Run configuration & results
```

## Evaluation

### Metrics

**Primary Metrics**:
- **Average Points**: Lower is better (0 best, 26 worst)
- **Points Improvement**: Baseline points - Trained points

**Secondary Metrics** (TensorBoard):
- Policy loss
- Value loss
- Entropy (exploration measure)
- Explained variance (critic quality)

### TensorBoard

Monitor training progress:

```bash
tensorboard --logdir training_runs/experiment_1/logs
```

Navigate to `http://localhost:6006`

**Key Plots**:
- `Loss/total`: Overall training loss
- `Loss/policy`: PPO surrogate loss
- `Loss/value`: Critic MSE loss
- `Loss/entropy`: Policy entropy

## Hyperparameter Tuning

### Key Hyperparameters

| Parameter | Default | Range | Effect |
|-----------|---------|-------|--------|
| Learning Rate | 3e-4 | 1e-5 to 1e-3 | Too high: unstable, too low: slow |
| Batch Size | 256 | 64 to 1024 | Larger = more stable, slower |
| Clip Epsilon | 0.2 | 0.1 to 0.3 | Lower = more conservative updates |
| GAE Lambda | 0.95 | 0.9 to 0.99 | Higher = less bias, more variance |
| Gamma | 0.99 | 0.95 to 0.999 | Discount factor for future rewards |
| Entropy Coef | 0.01 | 0.001 to 0.1 | Higher = more exploration |
| Value Coef | 0.5 | 0.25 to 1.0 | Weight of value loss |

### Tuning Tips

1. **Start with defaults** - They work reasonably well
2. **Adjust learning rate first** - If loss plateaus, decrease LR
3. **Increase batch size** if training is unstable
4. **Increase entropy coef** if policy becomes too deterministic
5. **Monitor TensorBoard** to diagnose issues

### Common Issues

**Loss exploding**:
- Decrease learning rate (try 1e-4)
- Increase batch size (try 512)
- Decrease clip epsilon (try 0.1)

**No improvement**:
- Increase learning rate (try 5e-4)
- Collect more data (try 5000 games)
- Check reward shaping (try "shaped" mode)

**Value function not learning**:
- Increase value coefficient (try 1.0)
- Check GAE lambda (try 0.9 for less bias)

## Troubleshooting

### Build Errors

**Error**: `cargo build` fails
- Run `cargo clean && cargo build --release`
- Check Rust version: `rustc --version` (should be 1.70+)

### Python Import Errors

**Error**: `ModuleNotFoundError: No module named 'hearts_rl'`
- Ensure you're in the `python/` directory
- Install dependencies: `pip install -r requirements.txt`

### Training Crashes

**Error**: `CUDA out of memory`
- Use CPU: `--device cpu`
- Reduce batch size: `--batch-size 128`

**Error**: Dataset loading fails
- Check JSONL format: `head -1 experiences.jsonl`
- Validate JSON: `python -c "import json; json.load(open('experiences.jsonl'))"`

### Evaluation Fails

**Error**: Cannot find mdhearts
- Build release binary: `cargo build --release`
- Specify path: `--mdhearts path/to/mdhearts.exe`

### Poor Performance

**Policy performs worse than baseline**:
- Check reward mode (shaped usually works best)
- Increase training iterations (try 200-500)
- Collect more data (try 5000-10000 games)
- Verify weights loaded correctly: Test with `mdhearts eval 10 --ai embedded --weights weights.json`

## Advanced Topics

### Multi-Stage Training

Train in stages with increasing difficulty:

```bash
# Stage 1: Train against easy baseline
mdhearts eval 1000 --self-play --collect-rl stage1.jsonl
python -m hearts_rl.train --data stage1.jsonl --output stage1_weights.json

# Stage 2: Self-play with stage 1 weights
mdhearts eval 2000 --self-play --collect-rl stage2.jsonl --weights stage1_weights.json
python -m hearts_rl.train --data stage2.jsonl --output stage2_weights.json

# Stage 3: Fine-tune
mdhearts eval 5000 --self-play --collect-rl stage3.jsonl --weights stage2_weights.json
python -m hearts_rl.train --data stage3.jsonl --output final_weights.json --iterations 200
```

### Curriculum Learning

Gradually increase task difficulty:

1. Train on "shaped" rewards (immediate feedback)
2. Switch to "per_trick" rewards (delayed feedback)
3. Final training on "terminal" rewards (sparse)

### Distributed Training

For large-scale training:

1. Collect data on multiple machines in parallel
2. Combine JSONL files: `cat exp1.jsonl exp2.jsonl > combined.jsonl`
3. Train on combined dataset

## References

- PPO Paper: [Schulman et al. 2017](https://arxiv.org/abs/1707.06347)
- GAE Paper: [Schulman et al. 2015](https://arxiv.org/abs/1506.02438)
- Implementation Plan: See `docs/RL_IMPLEMENTATION_PLAN.md`
