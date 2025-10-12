# Gen3 RL Experiment: Bootstrap from Hard-Level BC Model

**Date**: 2025-10-12
**Status**: Training in progress
**Branch**: `gen3-hard-bootstrap`

## Hypothesis

**Problem**: Gen0 and Gen2 RL training failed because they started from weak policies (Normal-level), causing RL to reinforce weaknesses rather than discover improvements.

**Solution**: Start RL training from the Hard-level BC model (`bc_hard_20ep_10k.json`) which already plays at ~Hard difficulty.

**Expected outcome**: RL can discover strategic improvements when starting from competent play, potentially surpassing the Hard heuristic baseline.

## Experiment Design

### Data Collection
```bash
./mdhearts.exe eval 25000 \
  --self-play \
  --weights ai_training/bc/bc_hard_20ep_10k.json \
  --collect-rl gen3_selfplay.jsonl \
  --reward-mode shaped
```

**Results:**
- Games: 25,000
- Experiences: 1,300,000
- Collection time: 74 seconds
- File size: 1.5 GB
- Average points per seat: 6.43-6.56 (balanced self-play)

### Training Configuration
```bash
cd python && python -m hearts_rl.train \
  --data ../gen3_selfplay.jsonl \
  --output ../gen3_weights.json \
  --iterations 100 \
  --checkpoint-dir ../gen3_checkpoints \
  --log-dir ../gen3_logs \
  --resume ../ai_training/bc/bc_hard_20ep_10k.json
```

**Key innovations:**
- ✅ **JSON weight loading**: Added `load_weights_from_json()` method to initialize actor from BC model
- ✅ **Actor initialization**: Loads all 3 layers from BC Hard model (270→256→128→52)
- ✅ **Critic initialization**: Randomly initialized (will be learned during RL training)
- ✅ **Optimizer reset**: Fresh Adam optimizer state for RL fine-tuning

**Parameters:**
- Algorithm: PPO
- Iterations: 100
- Batch size: 256
- Learning rate: 3e-4
- Clip epsilon: 0.2
- Gamma: 0.99
- GAE lambda: 0.95
- Device: CPU

**Expected duration**: ~10 hours (6 min/iteration × 100 iterations)

## Baseline Comparisons

### Gen0 (Failed)
- **Starting point**: Supervised learning from Normal bot
- **Performance**: Lost to Normal by 74.6%
- **Root cause**: Weak starting policy

### Gen2 (Failed)
- **Starting point**: Self-play RL from Gen0
- **Performance**: Lost to Normal by 93% (even worse!)
- **Root cause**: Reinforced Gen0's weaknesses via self-play

### BC Hard (Current best)
- **Approach**: Supervised learning from Hard bot
- **Performance**: ~Hard level (31.8% win rate vs Hard, 36.25% vs Normal)
- **Limitation**: Can't exceed teacher (Hard bot)

### Gen3 (This experiment)
- **Starting point**: RL from BC Hard model
- **Target**: Beat Hard bot by discovering strategic improvements
- **Advantage**: Starts from competent play, not weakness

## Success Criteria

### Minimal Success ✅
Gen3 maintains BC Hard level performance (doesn't regress)
- Win rate vs Hard: ≥30%
- Win rate vs Normal: ≥35%

### Moderate Success ⭐
Gen3 shows improvement over BC Hard
- Win rate vs Hard: ≥35% (5% improvement)
- Win rate vs Normal: ≥40% (4% improvement)

### Strong Success ⭐⭐
Gen3 significantly beats Hard bot
- Win rate vs Hard: ≥40% (10% improvement)
- Win rate vs Normal: ≥45%

### Breakthrough Success ⭐⭐⭐
Gen3 dominates Hard bot
- Win rate vs Hard: ≥45%
- Win rate vs Normal: ≥50%

## Evaluation Plan

Once training completes (`gen3_weights.json` created):

```bash
# Evaluation 1: Gen3 vs Hard baseline
./mdhearts.exe eval 1000 \
  --ai hard \
  --ai-test embedded \
  --weights gen3_weights.json

# Evaluation 2: Gen3 vs Normal baseline
./mdhearts.exe eval 1000 \
  --ai normal \
  --ai-test embedded \
  --weights gen3_weights.json

# Evaluation 3: Gen3 vs BC Hard (self-improvement)
./mdhearts.exe eval 1000 \
  --ai-per-seat embedded,embedded,embedded,embedded \
  --weights-per-seat ai_training/bc/bc_hard_20ep_10k.json,ai_training/bc/bc_hard_20ep_10k.json,ai_training/bc/bc_hard_20ep_10k.json,gen3_weights.json
```

## Technical Details

### Code Changes
**File**: `python/hearts_rl/trainer.py`

Added `load_weights_from_json()` method:
- Loads actor weights (3 layers) from JSON BC model
- Reshapes flat weight arrays to PyTorch tensors
- Initializes critic head with small random weights
- Allows seamless BC→RL transfer

**Key insight**: BC model learns policy (actor), RL needs to learn value function (critic) from scratch.

### Architecture Consistency
Both BC and RL use identical architecture:
- Input: 270 features (observation)
- Hidden 1: 256 units (ReLU)
- Hidden 2: 128 units (ReLU)
- Actor head: 52 units (action logits)
- Critic head: 1 unit (value estimate) [**NEW for RL**]

Total parameters: ~109k

### Training Data Quality
Gen3 self-play data is from BC Hard model (not Normal):
- Higher quality demonstrations
- Better strategic patterns
- Consistent ~Hard-level play across all 4 seats
- Balanced (no seat bias)

Compare to Gen2:
- Gen2 used Gen0 (Normal-level) self-play
- Lower quality, reinforced weaknesses
- Failed to improve

## Monitoring Progress

Check training status:
```bash
# Monitor training output
# Background bash ID: a87316

# Check loss convergence
ls -lht gen3_logs/*.tfevents.*

# Check checkpoints
ls -lht gen3_checkpoints/

# Check if training finished
ls -lh gen3_weights.json
```

## Next Steps (if Gen3 succeeds)

1. **Mixed-opponent training**: Train against diverse opponents (Hard + Gen3)
2. **Larger model**: Scale up to 270→512→256→128→52 (~300k params)
3. **Add seat position features**: Explicit seat encoding (274 dims instead of 270)
4. **Attention mechanism**: Replace feed-forward with transformer layers
5. **Curriculum learning**: Progressive difficulty ramping

## Next Steps (if Gen3 fails)

1. **Try longer training**: 200 iterations instead of 100
2. **Try different learning rate**: 1e-4 instead of 3e-4
3. **Try sparse rewards**: Terminal rewards only (not shaped)
4. **Collect more data**: 50k games instead of 25k
5. **Accept BC Hard as ceiling**: Focus on UI/UX improvements instead

## References

- Gen0 results: `docs/FINAL_TRAINING_RESULTS.md`
- Gen2 status: `docs/2025.10.09 - Status.md`
- BC model: `ai_training/README.md`
- Mixed evaluation: `docs/HLD_MIXED_EVALUATION.md`

---

**Training started**: 2025-10-12 17:08 UTC
**Expected completion**: 2025-10-13 ~03:00 UTC
**Check back in**: ~10 hours
