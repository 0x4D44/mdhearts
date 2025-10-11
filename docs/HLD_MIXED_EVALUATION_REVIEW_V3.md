# HLD Mixed Evaluation - Third Review (v3.0)

## Document Information
- **Review Date**: October 7, 2025
- **Reviewer**: Claude Code
- **Document Reviewed**: HLD_MIXED_EVALUATION.md v3.0
- **Previous Review**: HLD_MIXED_EVALUATION_REVIEW_V2.md (15 issues, all addressed)

## Executive Summary

This is a final review of v3.0 after addressing all 15 issues from the second review. The design is significantly improved and **mostly ready for implementation**. However, **8 minor issues remain** that should be addressed:

- **0 Critical Issues** ✅
- **2 Important Issues** ⚠️ (naming inconsistency, missing function spec)
- **6 Minor Issues** ⚠️ (edge cases, documentation gaps)

**Overall Assessment**: Design is solid. Minor cleanup needed but implementation can proceed in parallel.

---

## Issues Found

### Important Issues (2)

#### 1. **Naming Inconsistency: `SeatConfig` Should Be `PolicyConfig`** ⚠️⚠️

**Severity**: Important (terminology consistency)

**Problem**: The struct is called `SeatConfig` but it configures a *policy*, not a seat.

```rust
// Line 252-258: Inconsistent naming
pub struct SeatConfig {  // ← Should be PolicyConfig
    pub ai_type: AiType,
    pub weights_path: Option<PathBuf>,
    pub label: Option<String>,
}

// Line 235: Used in MixedEvalConfig
pub seat_configs: [SeatConfig; 4],  // ← Should be policy_configs
```

With systematic rotation, policies move between seats. The configuration describes the policy (AI type, weights), not the physical seat.

**Evidence of Confusion**:
- Line 235: `seat_configs` field name
- Line 730-738: CLI creates seat_configs but they're really policy configs
- Everywhere else uses "policy" terminology correctly

**Impact**:
- Inconsistent with PolicyResults, test_policy_index, etc.
- May confuse implementers
- Not a runtime bug, but harms code clarity

**Fix**:
```rust
/// Configuration for a single policy
#[derive(Debug, Clone)]
pub struct PolicyConfig {  // Renamed from SeatConfig
    pub ai_type: AiType,
    pub weights_path: Option<PathBuf>,
    pub label: Option<String>,
}

pub struct MixedEvalConfig {
    pub num_games: usize,
    pub policy_configs: [PolicyConfig; 4],  // Renamed from seat_configs
    pub output_mode: OutputMode,
    pub rotation_mode: RotationMode,
}
```

**Recommendation**: Rename for consistency, but not blocking for implementation.

---

#### 2. **Missing Specification: `aggregate_results` Function** ⚠️⚠️

**Severity**: Important (implementation gap)

**Problem**: Function is called (line 366) but never defined or specified.

```rust
// Line 366: Called but not defined
let aggregated = aggregate_results(&results, &config)?;
```

**What This Function Must Do**:
1. Take `Vec<GameResult>` (per-game results with points remapped to policy indices)
2. Compute per-policy aggregates:
   - total_points (sum across all games)
   - avg_points (total / num_games)
   - moon_count (count games where policy shot moon)
   - win_count (count games where policy had lowest score)
3. Return `MixedEvalResults`

**Edge Cases**:
- Ties for lowest score: All tied policies get win_count++
- Multiple moon shooters: Impossible in Hearts rules, but should document
- Division by zero: If num_games=0 (should be caught by validation)

**Fix**: Add specification:

```rust
/// Aggregate per-game results into per-policy statistics
fn aggregate_results(
    results: &[GameResult],
    config: &MixedEvalConfig
) -> Result<MixedEvalResults, EvalError> {
    let num_games = results.len();

    // Initialize counters for each policy
    let mut total_points = [0usize; 4];
    let mut moon_counts = [0usize; 4];
    let mut win_counts = [0usize; 4];

    for game_result in results {
        // Accumulate points
        for policy_idx in 0..4 {
            total_points[policy_idx] += game_result.points[policy_idx] as usize;
        }

        // Count moon shots
        if let Some(shooter_idx) = game_result.moon_shooter {
            moon_counts[shooter_idx] += 1;
        }

        // Count wins (lowest score)
        let min_score = *game_result.points.iter().min().unwrap();
        for policy_idx in 0..4 {
            if game_result.points[policy_idx] == min_score {
                win_counts[policy_idx] += 1;
            }
        }
    }

    // Build policy results
    let mut policy_results = Vec::new();
    for policy_idx in 0..4 {
        let config_for_policy = &config.policy_configs[policy_idx];
        policy_results.push(PolicyResults {
            policy_index: policy_idx,
            ai_type: config_for_policy.ai_type,
            ai_label: config_for_policy.label.clone()
                .unwrap_or_else(|| format!("{:?}", config_for_policy.ai_type)),
            avg_points: total_points[policy_idx] as f64 / num_games as f64,
            total_points: total_points[policy_idx],
            moon_count: moon_counts[policy_idx],
            win_count: win_counts[policy_idx],
        });
    }

    Ok(MixedEvalResults {
        games_played: num_games,
        policy_results: policy_results.try_into().unwrap(),
        comparison: None,  // Filled in later if needed
        rotation_mode: config.rotation_mode.clone(),
        elapsed_seconds: 0.0,  // Filled in by caller
    })
}
```

**Recommendation**: Add this specification to the HLD.

---

### Minor Issues (6)

#### 3. **Homogeneous Baseline Check Incomplete** ⚠️

**Severity**: Minor (edge case)

**Problem**: Validation checks baseline policies have same `ai_type`, but not same weights.

```rust
// Line 401-418: Only checks ai_type
let baseline_types: Vec<_> = config.seat_configs.iter()
    .filter(|(i, _)| *i != test_policy_index)
    .map(|(_, cfg)| cfg.ai_type)  // ← Only checks type, not weights
    .collect();

if !baseline_types.windows(2).all(|w| w[0] == w[1]) {
    return Err(...);
}
```

**Edge Case**:
```bash
# User creates 3 embedded baselines with different weights
mdhearts eval 200 --ai-per-seat embedded,embedded,embedded,embedded \
                  --weights-per-seat w1.json,w2.json,w3.json,w4.json

# Validation passes (all are "embedded")
# But baselines are NOT homogeneous (different weights)!
```

**Fix**: Check weights too:

```rust
// After checking ai_type, also check weights for embedded AIs
if baseline_types[0] == AiType::Embedded {
    let baseline_weights: Vec<_> = config.seat_configs.iter()
        .filter(|(i, _)| *i != test_policy_index)
        .map(|(_, cfg)| &cfg.weights_path)
        .collect();

    if !baseline_weights.windows(2).all(|w| w[0] == w[1]) {
        return Err(EvalError::InvalidConfig(
            "Comparison mode with embedded baseline requires same weights for all baseline policies".into()
        ));
    }
}
```

**Recommendation**: Add to validation or document as limitation.

---

#### 4. **Win Count Semantics Need Clarification** ⚠️

**Severity**: Minor (documentation)

**Problem**: Current documentation says "In case of tie, all tied policies count as winners" but doesn't explain implications.

**Implication**: Sum of win_counts can exceed num_games.

**Example**:
- Game 1: Scores [6, 6, 7, 8] → Policies 0 and 1 both win
- Game 2: Scores [5, 6, 7, 8] → Policy 0 wins
- Game 3: Scores [6, 6, 6, 8] → Policies 0, 1, 2 all win

Result: win_count = [3, 2, 1, 0], sum = 6 > 3 games

**Fix**: Add note in documentation:

```rust
/// Number of games where this policy had the lowest score
/// (wins in Hearts = lowest points)
///
/// Note: In case of ties, all tied policies count as winners,
/// so sum of win_counts across all policies may exceed games_played.
///
/// Example: If all 4 policies tie at 6 points, all get win_count++
pub win_count: usize,
```

**Recommendation**: Clarify in comments and output interpretation.

---

#### 5. **Missing GameResult Definition** ⚠️

**Severity**: Minor (specification completeness)

**Problem**: `GameResult` struct is used throughout but never defined in HLD.

```rust
// Used on line 356, 432, 505, etc., but never defined
let game_result = run_single_game(&policies, seat_mapping)?;
results.push(game_result);
```

**Fix**: Add definition:

```rust
/// Result from a single game (after remapping to policy indices)
#[derive(Debug, Clone)]
pub struct GameResult {
    /// Points earned by each policy in this game
    /// points[policy_index] = points earned
    pub points: [u8; 4],

    /// Index of policy that shot the moon (if any)
    pub moon_shooter: Option<usize>,
}
```

**Recommendation**: Add to data structures section.

---

#### 6. **--weights-per-seat Edge Case** ⚠️

**Severity**: Minor (user experience)

**Problem**: User can specify embedded AI with "_" for weights, which should error but isn't validated.

```bash
# User mistake: embedded AI without weights
mdhearts eval 200 --ai-per-seat normal,normal,normal,embedded \
                  --weights-per-seat _,_,_,_  # Oops! Forgot weights for seat 3

# What happens?
# - Parsing succeeds (creates None for all weights)
# - create_policy() is called with Embedded + None
# - Error only happens deep in execution, not at CLI parsing
```

**Fix**: Add validation in CLI parsing or in `create_policy`:

```rust
fn parse_weights_per_seat(s: &str, ai_types: &[AiType]) -> Result<Vec<Option<PathBuf>>, CliError> {
    let parts: Vec<&str> = s.split(',').collect();
    let mut weights = Vec::new();

    for (i, part) in parts.iter().enumerate() {
        let weight = if part.trim() == "_" {
            None
        } else {
            Some(PathBuf::from(part.trim()))
        };

        // Validate: embedded AI requires weights
        if ai_types[i] == AiType::Embedded && weight.is_none() {
            return Err(CliError::MissingWeights(
                format!("Policy {} is embedded but no weights specified (use path instead of '_')", i)
            ));
        }

        weights.push(weight);
    }

    Ok(weights)
}
```

**Recommendation**: Add early validation for better UX.

---

#### 7. **JSON Schema Example Has Legacy Field** ⚠️

**Severity**: Minor (documentation)

**Problem**: JSON schema (line ~1185) references `avg_points_per_seat` which is legacy v1.0 format.

```json
// Line ~1185 in Appendix B
{
  "avg_points_per_seat": [7.32, 7.13, 6.07, 5.49],  // ← Legacy field name
  ...
}
```

Should be updated to v2.0 format or clearly marked as v1.0 legacy.

**Fix**: Either update to v2.0 or add comment:

```json
// Version 1.0 (Legacy) - for --ai mode
{
  "format_version": "1.0",
  "avg_points_per_seat": [7.32, 7.13, 6.07, 5.49],  // Per-seat for homogeneous
  ...
}
```

**Recommendation**: Clarify which version is being shown.

---

#### 8. **Progress Reporting During Validation** ⚠️

**Severity**: Minor (user experience)

**Problem**: If validation fails, user gets error immediately. But for long-running commands, they might want to know validation passed before games start.

**Suggestion**: Add confirmation message:

```rust
pub fn run_mixed_eval(config: MixedEvalConfig) -> Result<MixedEvalResults, EvalError> {
    // 1. Validate configuration
    validate_config(&config)?;

    // Print confirmation
    println!("Configuration validated successfully");
    println!("Running {} games with {} rotation mode",
        config.num_games,
        match config.rotation_mode {
            RotationMode::Fixed => "fixed",
            RotationMode::Systematic => "systematic",
            RotationMode::Random => "random",
        }
    );

    // 2. Initialize policies...
}
```

**Recommendation**: Nice-to-have for better UX, not critical.

---

## Issues Resolved ✅

### From v2.0 → v3.0

All 15 issues from second review have been properly addressed:

- ✅ Terminology (test_seat → test_policy_index)
- ✅ CLI behavior specification
- ✅ Validation function added
- ✅ SeatResults → PolicyResults (fields renamed)
- ✅ Sample size validation improved
- ✅ Helper functions added
- ✅ Default rotation modes specified
- ✅ Rotation testing updated
- ✅ JSON fields renamed
- ✅ CLI parsing completed
- ✅ Ranking comments
- ✅ erf() citation
- ✅ Progress reporting
- ✅ Moon shooter documented
- ✅ win_count documented

---

## Summary

### Issue Count by Severity

| Severity | Count | Status |
|----------|-------|--------|
| **Critical** | 0 | ✅ None |
| **Important** | 2 | ⚠️ Minor naming and spec gaps |
| **Minor** | 6 | ⚠️ Edge cases and docs |
| **Total** | 8 | Manageable |

### Comparison: v1.0 → v2.0 → v3.0

| Version | Critical | Important | Minor | Status |
|---------|----------|-----------|-------|--------|
| v1.0 | 5 | 10 | 21 | Not implementable |
| v2.0 | 3 | 7 | 5 | Needs revision |
| v3.0 | 0 | 2 | 6 | **Ready (with notes)** |

---

## Recommendations

### Priority 1: Fix Before Implementation Starts

**None** - All critical issues resolved

### Priority 2: Fix During Implementation (Can Code in Parallel)

1. **Rename `SeatConfig` → `PolicyConfig`** (15 minutes)
   - Find/replace throughout codebase
   - Update all references to `seat_configs` → `policy_configs`

2. **Add `aggregate_results` specification** (30 minutes)
   - Copy implementation from review into HLD
   - Add unit test examples

### Priority 3: Address During Testing

3. Homogeneous baseline weight validation
4. Win count documentation
5. GameResult definition
6. CLI edge case validation
7. JSON schema cleanup
8. Progress reporting UX

---

## Implementation Readiness

**Assessment**: ✅ **READY FOR IMPLEMENTATION**

**Rationale**:
- No critical or blocking issues
- Important issues are minor naming/spec gaps
- Can be fixed during implementation
- Core logic is sound
- Statistical methods correct
- Validation comprehensive

**Suggested Approach**:
1. Start implementation with current design
2. Rename SeatConfig → PolicyConfig in first PR
3. Add aggregate_results as you implement
4. Address minor issues as encountered
5. Add unit tests for edge cases

---

## Final Verdict

**Status**: 🟢 **APPROVED FOR IMPLEMENTATION**

The design has evolved significantly through three review cycles:
- v1.0: 36 issues (5 critical)
- v2.0: 15 issues (3 critical)
- v3.0: 8 issues (0 critical)

The remaining 8 issues are minor and can be addressed during implementation without blocking progress. The core design is sound, well-documented, and ready for coding.

**Recommendation**: Proceed with implementation. Open issues as GitHub tasks for the 8 minor items.

---

## References

- **HLD v3.0**: [HLD_MIXED_EVALUATION.md](./HLD_MIXED_EVALUATION.md)
- **First Review (v1.0)**: [HLD_MIXED_EVALUATION_REVIEW.md](./HLD_MIXED_EVALUATION_REVIEW.md) (36 issues)
- **Second Review (v2.0)**: [HLD_MIXED_EVALUATION_REVIEW_V2.md](./HLD_MIXED_EVALUATION_REVIEW_V2.md) (15 issues)
- **v3.0 Summary**: [HLD_MIXED_EVALUATION_V3_SUMMARY.md](./HLD_MIXED_EVALUATION_V3_SUMMARY.md)

---

**Document End**
