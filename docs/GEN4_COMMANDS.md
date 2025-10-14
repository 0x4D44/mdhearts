# Gen4 Training Commands Reference

## Quick Start

**Option 1: Use the batch script (Windows):**
```cmd
launch_gen4.bat
```

**Option 2: Manual step-by-step:**

### Step 1: Build Release Binary

```bash
cargo build --release
```

### Step 2: Collect Diverse Opponent Data (~2 hours)

```bash
# Easy opponent (10k games, ~30 min)
./target/release/mdhearts.exe eval 10000 \
  --ai easy \
  --ai-test embedded \
  --weights ai_training/bc/bc_hard_20ep_10k.json \
  --collect-rl gen4_vs_easy.jsonl \
  --reward-mode shaped

# Normal opponent (10k games, ~30 min)
./target/release/mdhearts.exe eval 10000 \
  --ai normal \
  --ai-test embedded \
  --weights ai_training/bc/bc_hard_20ep_10k.json \
  --collect-rl gen4_vs_normal.jsonl \
  --reward-mode shaped

# Hard opponent (10k games, ~30 min)
./target/release/mdhearts.exe eval 10000 \
  --ai hard \
  --ai-test embedded \
  --weights ai_training/bc/bc_hard_20ep_10k.json \
  --collect-rl gen4_vs_hard.jsonl \
  --reward-mode shaped

# Self-play (10k games, ~30 min)
./target/release/mdhearts.exe eval 10000 \
  --self-play \
  --weights ai_training/bc/bc_hard_20ep_10k.json \
  --collect-rl gen4_selfplay.jsonl \
  --reward-mode shaped

# Merge all data
cat gen4_vs_easy.jsonl gen4_vs_normal.jsonl gen4_vs_hard.jsonl gen4_selfplay.jsonl > gen4_mixed.jsonl
```

**Expected output:**
- Total: 40k games
- Total: ~2.1M experiences
- File size: ~2.3 GB
- Time: ~2 hours

### Step 3: Launch Gen4 Training (~18 hours)

```bash
cd python && python -m hearts_rl.train \
  --data ../gen4_mixed.jsonl \
  --resume ../ai_training/bc/bc_hard_20ep_10k.json \
  --output ../gen4_weights.json \
  --iterations 150 \
  --lr 1e-4 \
  --clip-epsilon 0.1 \
  --bc-lambda 0.1 \
  --bc-reference ../ai_training/bc/bc_hard_20ep_10k.json \
  --save-interval 5 \
  --checkpoint-dir ../gen4_checkpoints \
  --log-dir ../gen4_logs
```

**Parameters explained:**
- `--lr 1e-4`: Conservative learning rate (Gen3 used 3e-4)
- `--clip-epsilon 0.1`: Tight PPO clipping (Gen3 used 0.2)
- `--bc-lambda 0.1`: BC regularization strength (NEW!)
- `--bc-reference`: Path to BC Hard for regularization (NEW!)
- `--save-interval 5`: Save checkpoint every 5 iterations (Gen3 used 10)

### Step 4: Monitor Training

**Watch training progress:**
```bash
# View latest output
tail -f gen4_logs/*.tfevents.*

# Or use TensorBoard
tensorboard --logdir=gen4_logs
```

**Check checkpoints:**
```bash
ls -lht gen4_checkpoints/
# Should see: checkpoint_5.pt, checkpoint_10.pt, ..., checkpoint_150.pt
```

### Step 5: Evaluate Checkpoints (~3 hours)

**Quick evaluation (every 10th checkpoint):**
```bash
for iter in 10 20 30 40 50 60 70 80 90 100 110 120 130 140 150; do
    python tools/eval_checkpoint.py gen4_checkpoints/checkpoint_${iter}.pt --games 200
done
```

**Thorough evaluation (key checkpoints):**
```bash
# Best checkpoint candidates
python tools/eval_checkpoint.py gen4_checkpoints/checkpoint_50.pt --games 1000
python tools/eval_checkpoint.py gen4_checkpoints/checkpoint_100.pt --games 1000
python tools/eval_checkpoint.py gen4_checkpoints/checkpoint_150.pt --games 1000
```

**Review results:**
```bash
cat gen4_checkpoint_tracking.txt
```

---

## Troubleshooting

### If BC regularization is too strong (no learning):

**Symptom:** All checkpoints identical to BC Hard, no improvement

**Solution:** Reduce `bc_lambda` to 0.05 or 0.01:
```bash
python -m hearts_rl.train \
  --data ../gen4_mixed.jsonl \
  --resume ../ai_training/bc/bc_hard_20ep_10k.json \
  --output ../gen4_v2_weights.json \
  --bc-lambda 0.05 \  # REDUCED
  --bc-reference ../ai_training/bc/bc_hard_20ep_10k.json \
  ... # other params same
```

### If BC regularization is too weak (catastrophic forgetting):

**Symptom:** Performance degrades like Gen3

**Solution:** Increase `bc_lambda` to 0.2 or 0.5:
```bash
python -m hearts_rl.train \
  --data ../gen4_mixed.jsonl \
  --resume ../ai_training/bc/bc_hard_20ep_10k.json \
  --output ../gen4_v2_weights.json \
  --bc-lambda 0.5 \  # INCREASED
  --bc-reference ../ai_training/bc/bc_hard_20ep_10k.json \
  ... # other params same
```

### If training is too slow:

**Solution:** Reduce iterations or use fewer games:
```bash
# Shorter training (100 iterations instead of 150)
python -m hearts_rl.train \
  --iterations 100 \
  ... # other params same

# Or collect less data (20k games instead of 40k)
# Use 5k games per opponent type instead of 10k
```

---

## Expected Results

### Success Criteria:

**Minimal Success ✅:**
- Win rate vs Hard: ≥32% (+1% over BC's 31%)
- **Interpretation**: BC regularization prevented forgetting
- **Action**: Continue to 300 iterations or try λ=0.05

**Moderate Success ⭐:**
- Win rate vs Hard: ≥35% (+4% over BC)
- **Interpretation**: RL discovered improvements
- **Action**: Scale up (larger model, more data)

**Strong Success ⭐⭐:**
- Win rate vs Hard: ≥40% (+9% over BC)
- **Interpretation**: Significant strategic improvements found
- **Action**: You have a challenging AI!

**Breakthrough Success ⭐⭐⭐:**
- Win rate vs Hard: ≥45% (+14% over BC)
- **Interpretation**: Superhuman play achieved
- **Action**: Publish research!

### Likely Outcomes (based on research):

**Most Likely (70%):**
- Maintains BC Hard level (≥30% vs Hard)
- Small improvement (+1-3%, may not be statistically significant)
- Proves BC regularization prevents catastrophic forgetting

**Optimistic (20%):**
- Clear improvement (+4-7% vs Hard, statistically significant)
- Model discovers better passing/void strategies
- Strong success achieved

**Pessimistic (10%):**
- No learning, identical to BC Hard
- λ=0.1 too constraining
- Need to retry with λ=0.01

---

## Comparison Commands

### Gen3 vs Gen4 (side-by-side):

```bash
# Gen3 (failed)
python -m hearts_rl.train \
  --data ../gen3_selfplay.jsonl \
  --lr 3e-4 \              # Aggressive
  --clip-epsilon 0.2 \     # Loose clipping
  --bc-lambda 0.0 \        # NO REGULARIZATION
  --iterations 100

# Gen4 (research-informed)
python -m hearts_rl.train \
  --data ../gen4_mixed.jsonl \  # Diverse opponents
  --lr 1e-4 \                    # Conservative
  --clip-epsilon 0.1 \           # Tight clipping
  --bc-lambda 0.1 \              # BC REGULARIZATION
  --bc-reference ../ai_training/bc/bc_hard_20ep_10k.json \
  --iterations 150
```

### Key Differences:

| Feature | Gen3 | Gen4 |
|---------|------|------|
| Data | 25k self-play only | 40k mixed opponents |
| Learning rate | 3e-4 (aggressive) | 1e-4 (conservative) |
| Clip epsilon | 0.2 (loose) | 0.1 (tight) |
| BC regularization | ❌ None | ✅ λ=0.1 |
| Iterations | 100 | 150 |
| Checkpoints | Every 10 | Every 5 |
| **Result** | **Failed (-22% degradation)** | **TBD** |

---

## Timeline

**Total estimated time: ~23 hours**

- Data collection: ~2 hours (can run overnight)
- Training: ~18 hours (run overnight)
- Evaluation: ~3 hours (next day)

**Recommended schedule:**
- Day 1 evening: Start data collection (leave running overnight)
- Day 2 morning: Start training (leave running all day + overnight)
- Day 3 morning: Evaluate results, analyze, decide next steps

---

## Files Created

After Gen4 completes, you'll have:

```
gen4_vs_easy.jsonl          (~600 MB)
gen4_vs_normal.jsonl        (~600 MB)
gen4_vs_hard.jsonl          (~600 MB)
gen4_selfplay.jsonl         (~600 MB)
gen4_mixed.jsonl            (~2.3 GB - merged)
gen4_weights.json           (~2 MB - final model)
gen4_checkpoints/           (~40 MB - 30 checkpoints)
  checkpoint_5.pt
  checkpoint_10.pt
  ...
  checkpoint_150.pt
gen4_logs/                  (~50 MB - TensorBoard logs)
gen4_checkpoint_tracking.txt (~5 KB - evaluation results)
```

**Total disk space needed:** ~4.5 GB

---

Created: 2025-10-14
Based on: `docs/GEN4_STRATEGY.md`
Ready to launch: ✅ YES
