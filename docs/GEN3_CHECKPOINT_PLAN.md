# Gen3 Checkpoint Evaluation Plan

## Checkpoint Schedule

Gen3 training saves checkpoints every 10 iterations (increased from 50 for better granularity):

| Checkpoint | Iteration | Expected Time | Status |
|------------|-----------|---------------|--------|
| `checkpoint_10.pt` | 10/100 | ~1 hour after start | Pending |
| `checkpoint_20.pt` | 20/100 | ~2 hours after start | Pending |
| `checkpoint_30.pt` | 30/100 | ~3 hours after start | Pending |
| `checkpoint_40.pt` | 40/100 | ~4 hours after start | Pending |
| `checkpoint_50.pt` | 50/100 | ~5 hours after start | Pending |
| `checkpoint_60.pt` | 60/100 | ~6 hours after start | Pending |
| `checkpoint_70.pt` | 70/100 | ~7 hours after start | Pending |
| `checkpoint_80.pt` | 80/100 | ~8 hours after start | Pending |
| `checkpoint_90.pt` | 90/100 | ~9 hours after start | Pending |
| `checkpoint_100.pt` | 100/100 | ~10 hours after start | Pending |
| `gen3_weights.json` | Final export | After iteration 100 | Pending |

**Training restarted**: 2025-10-12 ~16:38 UTC (with --save-interval 10)
**First checkpoint ETA**: 2025-10-12 ~17:40 UTC
**Final checkpoint ETA**: 2025-10-13 ~02:40 UTC

## Baseline: BC Hard Performance

Before RL training (iteration 0 = BC Hard model):

```
vs Hard:   31.8% win rate (baseline)
vs Normal: 36.25% win rate
vs Easy:   42% win rate
Avg points vs Hard: 6.93 (Hard: 6.07)
```

## Evaluation Protocol

For each checkpoint, run:

```bash
python tools/eval_checkpoint.py gen3_checkpoints/checkpoint_50.pt --games 200
```

This will:
1. Export checkpoint to `gen3_iter50_eval.json`
2. Run 200 games vs Hard baseline
3. Display win rate improvement and statistical significance
4. Append summary to `gen3_checkpoint_tracking.txt`

### Quick vs Thorough Evaluation

- **Quick (200 games)**: ~0.4 seconds, good for checkpoints
- **Thorough (1000 games)**: ~2 seconds, use for final evaluation

## Expected Loss Trajectory

Based on Gen0/Gen2 training:

| Iteration | Expected Loss | Notes |
|-----------|---------------|-------|
| 0 (BC) | ~0.03-0.04 | Starting point (BC Hard) |
| 1 | ~0.023 | Initial RL adjustment |
| 10-20 | ~0.02 | Early learning |
| 50 | ~0.015-0.018 | **First checkpoint** |
| 75-100 | ~0.012-0.015 | Convergence |
| 100 | ~0.010-0.012 | **Final checkpoint** |

## Key Questions to Answer

### 1. Does loss decrease correlate with performance?
- Track: (iteration, loss, win_rate_vs_hard)
- Plot: Loss vs Win Rate
- Goal: Understand if lower loss = better play

### 2. When does performance peak?
- Does iteration 50 beat BC Hard?
- Does iteration 100 beat iteration 50?
- Or does performance degrade (overfitting)?

### 3. Is improvement statistically significant?
- p < 0.05 means real improvement
- p >= 0.05 means noise / not significant

## Scenarios & Interpretations

### Scenario A: Steady Improvement ⭐⭐⭐
```
Iter 0  (BC):  31.8% vs Hard (baseline)
Iter 50:       35% vs Hard (+3.2%, p<0.05)
Iter 100:      38% vs Hard (+6.2%, p<0.01)
```
**Interpretation**: RL is working! Discovering better strategies.
**Action**: Continue training (try 200 iterations)

### Scenario B: Early Peak, Then Plateau ⭐⭐
```
Iter 0  (BC):  31.8% vs Hard
Iter 50:       34% vs Hard (+2.2%, p<0.05)
Iter 100:      34% vs Hard (+2.2%, p<0.05)
```
**Interpretation**: RL improves early, then converges.
**Action**: Use iteration 50 model, investigate plateau cause

### Scenario C: No Improvement ❌
```
Iter 0  (BC):  31.8% vs Hard
Iter 50:       31.5% vs Hard (-0.3%, p>0.05)
Iter 100:      32.1% vs Hard (+0.3%, p>0.05)
```
**Interpretation**: RL not finding improvements, just noise.
**Action**: Try different approach (larger model, more data, different rewards)

### Scenario D: Performance Degradation ❌❌
```
Iter 0  (BC):  31.8% vs Hard
Iter 50:       29% vs Hard (-2.8%, p<0.05)
Iter 100:      27% vs Hard (-4.8%, p<0.01)
```
**Interpretation**: RL is unlearning good strategies (catastrophic forgetting).
**Action**: Lower learning rate, reduce training iterations

## Data Collection

All evaluation results will be appended to:
- `gen3_checkpoint_tracking.txt` - One-line summaries
- `eval_iter50_vs_hard.txt` - Full output for iteration 50
- `eval_iter100_vs_hard.txt` - Full output for iteration 100
- `eval_gen3_final_vs_hard.txt` - Thorough 1000-game final eval

## Analysis Tools

After collecting checkpoint data:

```python
# Parse tracking file
with open('gen3_checkpoint_tracking.txt') as f:
    for line in f:
        # Extract: iteration, improvement_pct, p_value
        # Plot: iteration vs improvement_pct
        pass
```

This will visualize the learning curve and help identify:
- When performance peaks
- Whether training should continue
- If RL approach is viable

## Next Steps After Results

If Gen3 succeeds (beats BC Hard):
- Document successful RL recipe
- Try larger models (300k params)
- Try longer training (200 iterations)
- Try mixed-opponent training

If Gen3 fails (doesn't beat BC Hard):
- Analyze why (loss decreasing but performance flat?)
- Try alternative approaches from `docs/GEN3_EXPERIMENT.md`
- Consider BC Hard as practical ceiling
- Focus on other improvements (UI, features)

---

**Remember**: Even negative results are valuable! They tell us what doesn't work and guide future experiments.
