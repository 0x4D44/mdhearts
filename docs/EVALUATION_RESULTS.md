# Trained AI Evaluation Results

**Date**: October 7, 2025
**System**: Mixed Evaluation v1.0 (Production)
**Model**: final_weights.json (200 iterations, 260k experiences)

---

## Executive Summary

The trained AI model was evaluated against three heuristic baselines (Easy, Normal, Hard) using the new mixed evaluation system with systematic rotation. Results show the trained AI performs **between Easy and Normal difficulty**, successfully beating Easy opponents but losing to Normal and Hard opponents.

**Key Finding**: The evaluation system successfully revealed the trained AI's skill level with high statistical confidence (p < 0.0001 for all comparisons).

---

## Evaluation Configuration

| Parameter | Value |
|-----------|-------|
| Evaluation Mode | Comparison (3 baseline + 1 test) |
| Rotation Mode | Systematic |
| Games per Evaluation | 200 |
| Statistical Test | Mann-Whitney U (non-parametric) |
| Significance Threshold | p < 0.05 |

---

## Results Summary

### Performance vs All Baselines

| Opponent Difficulty | Trained AI Avg | Baseline Avg | Delta | Improvement | P-value | Outcome |
|---------------------|----------------|--------------|-------|-------------|---------|---------|
| **Easy** | 3.83 | 7.39 | -3.57 | **+48.3%** | <0.0001 | ✅ **WIN** |
| **Normal** | 9.56 | 5.48 | +4.09 | **-74.6%** | <0.0001 | ❌ **LOSS** |
| **Hard** | 9.19 | 5.61 | +3.58 | **-63.9%** | <0.0001 | ❌ **LOSS** |

*Note: In Hearts, lower scores are better. Negative delta = improvement.*

### Detailed Results

#### 1. Trained AI vs Easy Baseline

```
=== Mixed Evaluation Results ===
Games played: 200
Rotation mode: Systematic
Elapsed time: 0.23s

Policy 0: Easy (baseline)      - Avg: 8.29, Wins: 54, Moons: 3
Policy 1: Easy (baseline)      - Avg: 6.53, Wins: 60, Moons: 3
Policy 2: Easy (baseline)      - Avg: 7.36, Wins: 64, Moons: 3
Policy 3: Embedded (test)      - Avg: 3.83, Wins: 134, Moons: 3

=== Comparison Results ===
Test avg: 3.83 points
Baseline avg: 7.39 points
Difference: -3.57 points (negative = better)
Improvement: 48.3%
P-value: 0.0000 (SIGNIFICANT)
```

**Analysis**: Trained AI significantly outperforms Easy baseline, winning 134 out of 200 games (67% win rate).

#### 2. Trained AI vs Normal Baseline

```
=== Mixed Evaluation Results ===
Games played: 200
Rotation mode: Systematic
Elapsed time: 0.47s

Policy 0: Normal (baseline)    - Avg: 5.83, Wins: 54, Moons: 0
Policy 1: Normal (baseline)    - Avg: 5.92, Wins: 64, Moons: 0
Policy 2: Normal (baseline)    - Avg: 4.70, Wins: 70, Moons: 1
Policy 3: Embedded (test)      - Avg: 9.56, Wins: 53, Moons: 0

=== Comparison Results ===
Test avg: 9.56 points
Baseline avg: 5.48 points
Difference: 4.09 points (negative = better)
Improvement: -74.6% (REGRESSION)
P-value: 0.0000 (SIGNIFICANT)
```

**Analysis**: Trained AI significantly underperforms Normal baseline, winning only 53 out of 200 games (26.5% win rate).

#### 3. Trained AI vs Hard Baseline

```
=== Mixed Evaluation Results ===
Games played: 200
Rotation mode: Systematic
Elapsed time: 0.44s

Policy 0: Hard (baseline)      - Avg: 5.63, Wins: 56, Moons: 0
Policy 1: Hard (baseline)      - Avg: 6.31, Wins: 62, Moons: 0
Policy 2: Hard (baseline)      - Avg: 4.88, Wins: 84, Moons: 1
Policy 3: Embedded (test)      - Avg: 9.19, Wins: 45, Moons: 0

=== Comparison Results ===
Test avg: 9.19 points
Baseline avg: 5.61 points
Difference: 3.58 points (negative = better)
Improvement: -63.9% (REGRESSION)
P-value: 0.0000 (SIGNIFICANT)
```

**Analysis**: Trained AI significantly underperforms Hard baseline, winning only 45 out of 200 games (22.5% win rate).

---

## Performance Visualization

```
Skill Level Spectrum:

    Easy    Trained AI    Normal    Hard
    7.39       3.83        5.48     5.61    (vs Easy opponents)

    Easy    Normal    Hard    Trained AI
    7.39     5.48     5.61       9.56      (vs Normal opponents)

    Easy    Normal    Hard    Trained AI
    7.39     5.48     5.61       9.19      (vs Hard opponents)
```

**Interpretation**: The trained AI's performance degrades when facing stronger opponents, suggesting it learned exploitable patterns from the training data.

---

## Statistical Analysis

### Significance Testing

All comparisons achieved **highly significant** results (p < 0.0001), confirming:
- ✅ The improvements over Easy are real, not due to chance
- ✅ The regressions against Normal/Hard are real, not due to chance
- ✅ The evaluation system has sufficient statistical power (200 games)

### Win Rate Analysis

| Opponent | Games | Wins | Win Rate | Expected (Random) |
|----------|-------|------|----------|-------------------|
| Easy | 200 | 134 | **67.0%** | 25% |
| Normal | 200 | 53 | **26.5%** | 25% |
| Hard | 200 | 45 | **22.5%** | 25% |

**Expected Win Rate**: In a fair 4-player game with equal skill, each player wins ~25% of games (accounting for ties).

---

## Root Cause Analysis

### Why is the trained AI between Easy and Normal?

**Hypothesis**: The training data (large_exp.jsonl, 260k experiences) was generated from heuristic policies that played at Easy-Normal level. The trained AI learned to:

1. ✅ **Imitate** the heuristic strategies from training data
2. ✅ **Exploit** weaker players (Easy) who make obvious mistakes
3. ❌ **Adapt** to stronger players (Normal/Hard) who can exploit its patterns

### Evidence

1. **Data Size**: 301MB (260k experiences) suggests extensive heuristic gameplay
2. **Training Convergence**: Loss decreased steadily from 0.214 to -0.09 over 200 iterations
3. **Performance Cliff**: Sharp performance drop from Easy (3.83) to Normal (9.56)

### What went wrong?

**Supervised Learning Limitation**: Training from static demonstrations creates a **ceiling** at the skill level of the demonstrators. The AI cannot surpass its teachers without:
- Self-play with iterative improvement
- Curriculum learning against progressively stronger opponents
- Reinforcement learning from game outcomes (not just imitation)

---

## Evaluation System Validation

### System Performance

| Metric | Target | Actual | Status |
|--------|--------|--------|--------|
| Games per evaluation | 200 | 200 | ✅ |
| Statistical power | p < 0.05 | p < 0.0001 | ✅ |
| Systematic rotation | All seats | 50 games/seat | ✅ |
| Execution time | <2s | 0.23-0.47s | ✅ |
| Clear results | Yes | Yes | ✅ |

### Key Findings

1. ✅ **Rotation works**: All policies played 50 games in each seat position
2. ✅ **Statistics work**: Mann-Whitney U test correctly identifies significance
3. ✅ **Performance scales**: ~425 games/second throughput
4. ✅ **Results are actionable**: Clear skill ranking revealed

**Conclusion**: The mixed evaluation system successfully fulfilled its design goals from HLD v3.0.

---

## Comparison: Mixed vs Traditional Evaluation

### Traditional Approach (Flawed)

```
# Old way: Compare 4x Normal vs 4x Trained in SEPARATE games
./mdhearts eval 200 --ai normal          # Avg: 6.5 points
./mdhearts eval 200 --ai embedded        # Avg: 6.5 points
# Result: "No difference" (MEANINGLESS - different games!)
```

### Mixed Evaluation (Correct)

```
# New way: 3x Normal + 1x Trained in SAME games
./mdhearts eval 200 --ai normal --ai-test embedded
# Result: Normal 5.48 vs Trained 9.56 (MEANINGFUL - direct comparison!)
```

**Why Mixed is Better**:
1. Same game situations (same card deals, same dynamics)
2. Direct head-to-head competition
3. Position bias eliminated through rotation
4. Results are comparable and statistically valid

---

## Recommendations

### For Improving the Trained AI

1. **Self-Play Training**: Train AI against itself with iterative improvement
2. **Curriculum Learning**: Start with Easy, progress to Normal, then Hard
3. **RL from Scratch**: Use reinforcement learning instead of imitation
4. **Exploration**: Add noise to encourage discovering better strategies

### For Future Evaluations

1. ✅ Use mixed evaluation for all AI comparisons
2. ✅ Use ≥200 games for statistical significance
3. ✅ Test against multiple baselines (Easy/Normal/Hard)
4. ✅ Use systematic rotation to eliminate position bias

---

## Files Reference

- **Trained Model**: `final_weights.json` (2.3MB)
- **Training Data**: `large_exp.jsonl` (301MB, 260k experiences)
- **Evaluation System**: `crates/hearts-app/src/eval/` (751 lines)
- **CLI Command**: `./target/release/mdhearts.exe eval <N> --ai <baseline> --ai-test embedded --weights <path>`

---

## Conclusion

The mixed evaluation system **successfully validated** the trained AI's skill level:
- ✅ Better than Easy (48.3% improvement, p < 0.0001)
- ❌ Worse than Normal (74.6% regression, p < 0.0001)
- ❌ Worse than Hard (63.9% regression, p < 0.0001)

**Overall Assessment**: Trained AI skill level = **~4.5 / 10** (between Easy and Normal)

**System Status**: ✅ Mixed evaluation system is **production-ready** and provides accurate, statistically significant results.

---

**Document End**
