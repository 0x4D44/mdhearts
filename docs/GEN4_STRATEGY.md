# Gen4 RL Strategy: Research-Informed Hybrid Approach

**Date**: 2025-10-14
**Status**: Design Complete - Ready to Implement
**Goal**: Beat Hard heuristic baseline using research-backed RL methods

## Research Findings Summary

### Key Insights from Literature

**Hearts AI State-of-the-Art (2024-2025):**
1. ❌ **No existing AI has achieved human-level performance** in full Hearts (with moon-shooting and passing)
2. ✅ **Some success in simplified versions** (without moon-shooting)
3. ⚠️ **Pure self-play RL has been unsuccessful** in previous research
4. ✅ **Hybrid approaches show promise** (supervised pretraining + RL fine-tuning)
5. ✅ **Experience replay critical** for preventing catastrophic forgetting

**Best Performing Methods:**
- **Monte Carlo sampling** with UCT algorithm
- **maxn/soft-maxn** search algorithms with lookahead
- **Feature engineering** (1,830-34,280 hand-crafted features)
- **Supervised learning from expert play** (6.35 avg points vs 7.30 for previous expert)
- **Neural Fictitious Self-Play (NFSP)** for Nash equilibrium approximation

**Common Failure Modes:**
- Pure DQN/PPO self-play: **Catastrophic forgetting**
- Splitting networks by game phase: **Robustness depends on opponents**
- Training on weak opponents: **Doesn't generalize**

---

## Gen4 Strategy: Multi-Pronged Attack

Based on research + our Gen3 failure analysis, Gen4 will combine **5 proven techniques**:

###

 1. **BC Regularization** (NEW - Based on NFSP insights)

**Problem Gen3 Had:**
- Pure RL gradients destroyed BC knowledge
- No penalty for forgetting expert strategies

**Solution:**
```python
total_loss = ppo_loss + λ * kl_divergence(current_policy, bc_hard_policy)
```

**How It Works:**
- Keep BC Hard model frozen as "expert reference"
- Penalize policy divergence from BC using KL-divergence
- λ = 0.1: Allows exploration while anchoring to expert knowledge
- Based on "importance weighting" from research paper

**Expected Benefit:**
- ✅ Prevents catastrophic forgetting
- ✅ Maintains BC Hard floor performance
- ✅ Allows RL to find incremental improvements on top of BC

### 2. **Opponent Diversity Training** (Based on UAlberta research)

**Problem Gen3 Had:**
- All 4 players identical (BC Hard self-play only)
- Self-play amplifies errors (no external correction)

**Solution:**
```bash
# Mix of opponent types (10k games each)
- 10k vs Easy bot
- 10k vs Normal bot
- 10k vs Hard bot
- 10k vs Self (BC Hard)
= 40k total games, 2.1M experiences
```

**Why This Works (from research):**
- "Training against different opponent types" explicitly mentioned as successful
- Exposes model to diverse strategies
- Prevents overfitting to single opponent style
- Similar to how humans learn (play against variety of opponents)

**Expected Benefit:**
- ✅ More robust policy
- ✅ Better generalization
- ✅ Less error amplification

### 3. **Experience Replay** (Already implemented, but critical)

**Research Finding:**
> "Catastrophic forgetting can be solved by implementing experience replay"

**Current Implementation:**
- ✅ All 2.1M experiences stored in JSONL
- ✅ Random batch sampling during training
- ✅ Multiple epochs over same data

**Why It Works:**
- Averages over diverse experiences
- Breaks temporal correlation
- Stabilizes gradient updates

### 4. **Conservative Hyperparameters** (Based on imperfect info game research)

**Gen3 Settings** (Too aggressive):
- Learning rate: 3e-4
- Clip epsilon: 0.2
- Iterations: 100

**Gen4 Settings** (Research-informed):
- Learning rate: **1e-4** (3× slower, less forgetting)
- Clip epsilon: **0.1** (tighter PPO clipping, more conservative)
- Iterations: **150** (50% longer training time)
- Checkpoint interval: **5** (catch best model early)

**Research Support:**
> "Learning rate (α) was a critical hyperparameter"

### 5. **Target Network / Frozen Critic** (Optional enhancement)

**Research Finding:**
> "Using a target network solves the problem of learning instability"

**Potential Implementation:**
```python
# Update critic every N steps instead of every step
if global_step % target_update_freq == 0:
    target_critic.load_state_dict(critic.state_dict())
```

**Status:** Not implementing in Gen4 v1, but available if needed

---

## Gen4 Architecture & Configuration

### Model Architecture
```
Input: 270 features (same as BC Hard)
├─ Layer 1: 270 → 256 (ReLU)
├─ Layer 2: 256 → 128 (ReLU)
├─ Actor Head: 128 → 52 (action logits)
└─ Critic Head: 128 → 1 (value estimate)

Total params: ~109k
```

**Why NOT scaling up:**
- Research shows "single large network performed better than clustered networks"
- But also shows feature count matters more than network size
- Our 270 features already comprehensive
- Scaling to 512-256-128 may help, but try Gen4 first

### Training Configuration

```python
# Algorithm
algorithm = "PPO with BC regularization"

# Core PPO params
learning_rate = 1e-4          # Conservative (Gen3: 3e-4)
clip_epsilon = 0.1            # Tighter clipping (Gen3: 0.2)
gamma = 0.99                  # Discount factor (same)
gae_lambda = 0.95             # GAE lambda (same)
batch_size = 256              # Batch size (same)
num_epochs = 4                # Epochs per iteration (same)

# BC regularization (NEW)
bc_lambda = 0.1               # BC KL-divergence weight
bc_reference = "ai_training/bc/bc_hard_20ep_10k.json"

# Training schedule
iterations = 150              # Longer training (Gen3: 100)
checkpoint_interval = 5       # More frequent (Gen3: 10)
device = "cpu"                # CPU training (same)
```

### Data Collection Plan

**Phase 1: Diverse Opponent Data** (~2 hours)

```bash
# Easy opponent (10k games)
./mdhearts.exe eval 10000 \
  --ai easy \
  --ai-test embedded \
  --weights ai_training/bc/bc_hard_20ep_10k.json \
  --collect-rl gen4_vs_easy.jsonl \
  --reward-mode shaped

# Normal opponent (10k games)
./mdhearts.exe eval 10000 \
  --ai normal \
  --ai-test embedded \
  --weights ai_training/bc/bc_hard_20ep_10k.json \
  --collect-rl gen4_vs_normal.jsonl \
  --reward-mode shaped

# Hard opponent (10k games)
./mdhearts.exe eval 10000 \
  --ai hard \
  --ai-test embedded \
  --weights ai_training/bc/bc_hard_20ep_10k.json \
  --collect-rl gen4_vs_hard.jsonl \
  --reward-mode shaped

# Self-play (10k games)
./mdhearts.exe eval 10000 \
  --self-play \
  --weights ai_training/bc/bc_hard_20ep_10k.json \
  --collect-rl gen4_selfplay.jsonl \
  --reward-mode shaped

# Merge all data
cat gen4_vs_easy.jsonl gen4_vs_normal.jsonl gen4_vs_hard.jsonl gen4_selfplay.jsonl > gen4_mixed.jsonl

# Expected: 40k games, ~2.1M experiences, ~2.3 GB
```

**Phase 2: Training** (~18 hours)

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

**Phase 3: Evaluation** (~3 hours)

```bash
# Evaluate every 10th checkpoint (15 total: 10, 20, 30...150)
for iter in 10 20 30 40 50 60 70 80 90 100 110 120 130 140 150; do
    python tools/eval_checkpoint.py gen4_checkpoints/checkpoint_${iter}.pt --games 200
done
```

---

## Success Criteria (Same as Gen3, but Achievable)

### Minimal Success ✅
- Win rate vs Hard: ≥32% (+1% over BC Hard's 31%)
- **Interpretation**: BC regularization prevented forgetting
- **Action**: Continue training to 300 iterations

### Moderate Success ⭐
- Win rate vs Hard: ≥35% (+4% over BC Hard)
- **Interpretation**: RL discovered small improvements
- **Action**: Try scaling up (larger model, more data)

### Strong Success ⭐⭐
- Win rate vs Hard: ≥40% (+9% over BC Hard)
- **Interpretation**: RL found significant strategic improvements
- **Action**: This is the goal! You'll have a challenging AI

### Breakthrough Success ⭐⭐⭐
- Win rate vs Hard: ≥45% (+14% over BC Hard)
- **Interpretation**: Superhuman Hearts play achieved
- **Action**: Publish research paper!

---

## Expected Challenges & Mitigation

### Challenge 1: BC Regularization Too Strong (λ=0.1 too high)

**Symptom**: All checkpoints identical to BC Hard, no improvement

**Diagnosis**: BC anchor preventing exploration

**Fix**: Reduce λ to 0.05 or 0.01, retry training

### Challenge 2: BC Regularization Too Weak (λ=0.1 too low)

**Symptom**: Performance degrades like Gen3

**Diagnosis**: BC anchor too weak, still catastrophic forgetting

**Fix**: Increase λ to 0.2 or 0.5, retry training

### Challenge 3: Opponent Diversity Insufficient

**Symptom**: Good vs Easy/Normal, bad vs Hard

**Diagnosis**: Not enough Hard opponent exposure

**Fix**: Collect 20k vs Hard + 10k each Easy/Normal/Self

### Challenge 4: Hyperparameters Still Too Aggressive

**Symptom**: Slow degradation over iterations

**Diagnosis**: Learning rate 1e-4 still too high

**Fix**: Reduce to 5e-5, extend to 300 iterations

---

## Research-Backed Predictions

Based on literature review, here's what we expect:

### Most Likely Outcome (70% probability)
- ✅ BC regularization prevents catastrophic forgetting
- ✅ Performance maintains BC Hard level (≥30% vs Hard)
- ⚠️ Small improvement (+1-3% vs Hard) but not statistically significant
- **Verdict**: Partial success, proves approach works

### Optimistic Outcome (20% probability)
- ✅ BC regularization + opponent diversity = synergistic effect
- ✅ Clear improvement (+4-7% vs Hard, statistically significant)
- ✅ Model discovers new strategies (better passing, void creation)
- **Verdict**: Strong success, Gen4 beats BC Hard

### Pessimistic Outcome (10% probability)
- ❌ BC regularization too constraining, no exploration
- ❌ Opponent diversity creates confusion, not robustness
- ❌ Performance identical to BC Hard (no learning)
- **Verdict**: λ=0.1 wrong, try λ=0.01

---

## If Gen4 Succeeds: Next Steps

### Gen4.1: Hyperparameter Optimization
- Grid search λ ∈ {0.01, 0.05, 0.1, 0.2, 0.5}
- Grid search LR ∈ {5e-5, 1e-4, 3e-4}
- Find optimal balance exploration vs preservation

### Gen4.2: Larger Model
```
Input: 270 → 512 → 256 → 128 → 52
Total params: ~300k (3× larger)
```

Hypothesis: More capacity = room for RL improvements without forgetting

### Gen4.3: More Training Data
- Collect 100k games (vs current 40k)
- 5M+ experiences
- Better coverage of rare situations

### Gen4.4: Advanced Features (from research)
- Research used **34,280 features** (three-wise combinations)
- We use **270 features** (basic state representation)
- Add: "cards seen", "queen location probability", "moon shoot risk"

### Gen4.5: Search + RL Hybrid
- Use RL policy as heuristic for Monte Carlo search
- Research shows Monte Carlo + UCT performs well
- Combine neural network evaluation with search

---

## If Gen4 Fails: Alternative Paths

### Option A: Better Heuristics (Easier than RL)

Improve Hard bot directly based on research strategies:

**Add to Hard Bot:**
1. **Spade bleeding detection** (research-mentioned strategy)
2. **Moon shoot probability estimation** (track cards seen)
3. **Passing strategy refinement** (void suit creation logic)
4. **Opponent modeling** (track tendencies)

**Result**: Hard+ bot → BC from Hard+ → Better than current BC

**Effort**: 1-2 days vs weeks for RL

### Option B: NFSP (Neural Fictitious Self-Play)

Research shows NFSP works for imperfect information games:

> "NFSP is the first end-to-end deep reinforcement learning approach that approximates Nash equilibria"

**Difference from PPO:**
- Learns best-response to average opponent strategy
- Combines Q-learning (exploitation) + supervised learning (exploration)
- Proven to work in poker (similar imperfect info game)

**Implementation**: 2-3 days, worth trying if Gen4 PPO fails

### Option C: Human Expert Data

**Best teacher approach:**
1. Record 10k+ games from expert human players
2. Train BC from human data (not bot data)
3. Human strategies likely better than Hard bot

**Challenge**: Need to find/recruit expert Hearts players

### Option D: Ensemble + Search

**Combination approach:**
```python
def play_card(game_state):
    # 1. Get suggestions from multiple models
    bc_hard_move = bc_hard_model.predict(state)
    rl_model_move = gen4_model.predict(state)
    hard_bot_move = hard_heuristic.get_move(state)

    # 2. Run mini Monte Carlo simulation for each
    bc_score = simulate(bc_hard_move, 100_games)
    rl_score = simulate(rl_model_move, 100_games)
    heuristic_score = simulate(hard_bot_move, 100_games)

    # 3. Pick best
    return best_move(bc_score, rl_score, heuristic_score)
```

**Research support**: Monte Carlo sampling mentioned as successful

---

## Timeline Estimate

**Day 1 (Today):**
- ✅ Research (complete)
- ✅ Strategy document (this file)
- ⏳ Implement BC regularization (in progress)
- ⏳ Add --bc-lambda parameter
- ⏳ Collect diverse opponent data (2-3 hours)

**Day 2:**
- Launch Gen4 training (overnight, ~18 hours)

**Day 3:**
- Evaluate checkpoints (~3 hours)
- Analyze results
- Decide: Continue vs pivot vs iterate

**Total**: ~2-3 days to answer "Can Gen4 beat BC Hard?"

---

## Conclusion

Gen4 is our **best shot** at beating BC Hard based on:

1. ✅ **Research-backed methods** (BC regularization, opponent diversity, experience replay)
2. ✅ **Lessons from Gen3 failure** (catastrophic forgetting diagnosis)
3. ✅ **Conservative approach** (lower LR, tighter clipping, more checkpoints)
4. ✅ **Clear success criteria** (≥35% vs Hard = strong success)
5. ✅ **Fallback plans** if it fails (better heuristics, NFSP, ensemble)

**Confidence Level**: 70% chance of at least maintaining BC Hard, 20% chance of significant improvement

**Next Action**: Finish BC regularization implementation and collect diverse training data.

---

**Document created**: 2025-10-14
**Ready to proceed**: YES
**Estimated completion**: 2-3 days
