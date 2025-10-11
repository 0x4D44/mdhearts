# AI Training Artifacts

This directory contains training data, models, and checkpoints for the Hearts AI.

## Directory Structure

### `bc/` - Behavioral Cloning (Active)
**Current production model** trained via supervised learning from Hard bot demonstrations.

**Files:**
- `bc_hard_20ep_10k.json` - **PRODUCTION MODEL** (20 epochs, 10k games)
  - Training accuracy: 93.94%
  - Performance: ~Hard level (between Normal and Hard)
  - Training time: ~15 minutes
  - Status: **Use this model**

- `hard_demos_10k.jsonl` - Training data (520k experiences from 10k Hard bot games, 586 MB)

- `bc_hard_5ep.json` - Early 5-epoch model (1k games)
- `bc_hard_5ep_v2.json` - Early 5-epoch model v2 (1k games)
- `hard_demos_1k.jsonl` - Small training dataset (52k experiences, 59 MB)

**Performance (1000-game evaluation):**
- vs Hard: 31.8% win rate (Hard: 28%), 6.93 avg points (Hard: 6.07)
- vs Normal: 36.25% win rate (Normal: 25.5%), 6.32 avg points (Normal: 6.68)
- vs Easy: 42% win rate (Easy: 27.75%), 5.56 avg points (Easy: 7.45)
- **Verdict**: BC ≈ Hard level, significantly better than Gen0/Gen2

### `archive/gen0/` - Gen0 (Archived)
**First attempt** using supervised learning from heuristic bot traces.

**Files:**
- `final_weights.json` - Gen0 model (2.3 MB)
- `final_checkpoints/` - Training checkpoints
- `final_logs/` - TensorBoard logs

**Performance:**
- Between Easy and Normal difficulty
- vs Easy: +48.3% improvement
- vs Normal: -74.6% (loses)
- vs Hard: -63.9% (loses)

**Why archived**: BC significantly outperforms Gen0 (~62% more wins). Gen0 hit supervised learning ceiling.

## Deleted Artifacts

### Gen2 (Deleted - Failed)
Self-play reinforcement learning attempt that performed worse than Gen0.
- Trained 200 iterations over ~48 hours
- Result: -93% vs Normal (worse than Gen0's -74.6%)
- **Root cause**: Reinforced Gen0's weaknesses instead of learning better strategies
- **Deleted**: gen2_weights.json, gen2_checkpoints/, gen2_logs/, selfplay_gen1.jsonl (1.5GB)

### Early Experiments (Deleted)
Various failed experiments and prototypes:
- trained_weights.json, trained_policy.npz
- training_data.jsonl, training_exp.jsonl, large_exp.jsonl
- checkpoints/, logs/, train_pipeline.bat

## Training Methodology

### Behavioral Cloning (BC)
**Approach**: Supervised learning from expert demonstrations
- Collect gameplay data from Hard bot: `mdhearts eval --collect-rl`
- Train neural network to imitate Hard bot decisions
- Export weights: `python train_supervised.py`

**Success factors:**
- Fast training (15 min vs 48+ hours)
- Stable convergence
- Direct learning from strong baseline
- No reward shaping issues

### Self-Play RL (Failed)
**Approach**: PPO reinforcement learning with self-play
- Start from weak policy (Gen0)
- Play games against itself
- Learn from rewards

**Failure mode:**
- Reinforced existing weaknesses
- No diversity in training opponents
- Worse than starting policy

## Usage

### Load Production Model
```bash
# Evaluation
./mdhearts.exe eval 200 --ai-test embedded --weights ai_training/bc/bc_hard_20ep_10k.json

# Mixed evaluation
./mdhearts.exe eval 200 --ai-per-seat hard,hard,embedded,embedded \
  --weights-per-seat _,_,ai_training/bc/bc_hard_20ep_10k.json,ai_training/bc/bc_hard_20ep_10k.json
```

### Train New BC Model
```bash
# Generate training data (10k games = 21 seconds)
./mdhearts.exe eval 10000 --ai hard --collect-rl hard_demos.jsonl --reward-mode shaped

# Train model (20 epochs = 15 minutes)
cd python
python train_supervised.py --data hard_demos.jsonl --epochs 20 --output new_bc.json
```

## Key Learnings

1. **Behavioral cloning works better than self-play RL** for this domain
2. **Hard bot is a strong baseline** to learn from (~Hard level achievable)
3. **Sample size matters**: Need 1000+ games for reliable evaluation due to positional variance
4. **Position effects are large**: Identical bots can have 20-30% win rate variance by seat
5. **High accuracy ≠ equal performance**: 94% accuracy → Hard-level, not Hard-exceeding
