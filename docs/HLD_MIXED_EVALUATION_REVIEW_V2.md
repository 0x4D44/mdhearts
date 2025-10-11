# HLD Mixed Evaluation - Second Review (v2.0)

## Document Information
- **Review Date**: October 7, 2025
- **Reviewer**: Claude Code
- **Document Reviewed**: HLD_MIXED_EVALUATION.md v2.0 (Revised)
- **Previous Review**: HLD_MIXED_EVALUATION_REVIEW.md (v1.0)

## Executive Summary

This is a second review of the revised HLD (v2.0) after addressing the 5 critical issues from the first review. While the critical bugs have been fixed, **15 significant issues remain**, including:
- **3 Critical Issues**: Conceptual confusion between seats and policies, missing implementation details, validation logic gaps
- **7 Important Issues**: Terminology problems, API inconsistencies, edge cases
- **5 Minor Issues**: Documentation gaps, missing helper functions

**Overall Assessment**: Design is improved but still has critical conceptual issues that must be resolved before implementation.

---

## Critical Issues (MUST FIX)

### 1. **Terminology Confusion: "test_seat" vs "test_policy"** ⚠️⚠️⚠️

**Severity**: Critical (affects entire API design)

**Problem**: Throughout the design, `test_seat` is used to mean "test policy index", but with systematic rotation, policies move between physical seats. This creates massive confusion.

**Evidence**:

```rust
// Line 258: OutputMode uses "test_seat"
pub enum OutputMode {
    Comparison { test_seat: usize },  // ← But this is actually a POLICY index!
}

// Line 290: ComparisonResults uses "test_seat"
pub struct ComparisonResults {
    pub test_seat: usize,  // ← Actually means "test_policy_index"
}

// Line 448: Used as array index into policy points
let test_scores: Vec<f64> = results.iter()
    .map(|r| r.points[test_seat] as f64)  // ← Index into POLICY array
    .collect();
```

With systematic rotation:
- **Physical seats** are fixed (0, 1, 2, 3)
- **Policies** rotate through physical seats
- `points` array after remapping (line 424) is indexed by **policy index**, not seat

**Impact**:
- API is confusing and error-prone
- Users will think `test_seat: 3` means "test in physical seat 3" when it actually means "test policy index 3"
- Incompatible with rotation mode

**Fix**: Rename consistently throughout:
```rust
pub enum OutputMode {
    Comparison { test_policy_index: usize },  // Clear!
}

pub struct ComparisonResults {
    pub test_policy_index: usize,
    // ...
}

pub fn compute_comparison(
    results: &[GameResult],
    test_policy_index: usize,  // Explicit
) -> Result<ComparisonResults, EvalError>
```

**Or**, introduce separate concepts:
```rust
pub enum ComparisonMode {
    /// Compare one policy against others
    TestVsBaseline { test_policy: usize, baseline_policies: Vec<usize> },
    /// Compare all policies pairwise
    AllPairs,
}
```

---

### 2. **Missing Implementation Detail: How does `--ai-test` work without `--test-seat`?** ⚠️⚠️⚠️

**Severity**: Critical (specification incomplete)

**Problem**: The recommended CLI example doesn't show `--test-seat`, but the implementation calls `parse_test_seat()`.

**Evidence**:

```bash
# Line 193-196: Recommended example
mdhearts eval 200 --ai-test embedded \
                  --baseline normal \
                  --weights final_weights.json \
                  --rotation systematic

# No --test-seat specified!
```

But implementation (line 569):
```rust
let test_seat = parse_test_seat(&mut args)?;  // ← What if not specified?
```

**Questions**:
1. Is `--test-seat` optional or required for `--ai-test` mode?
2. If optional, what's the default behavior?
3. How does the system construct the [baseline, baseline, baseline, test] configuration?
4. Which 3 policies are baseline and which 1 is test?

**Possible Interpretations**:

**Option A**: `--ai-test` implicitly creates 1 test + 3 baseline
```bash
# Creates [Normal, Normal, Normal, Embedded] automatically
mdhearts eval 200 --ai-test embedded --baseline normal --weights w.json
```

**Option B**: `--test-seat` is required but wasn't shown in example (documentation bug)
```bash
# Must specify which index is test
mdhearts eval 200 --ai-test embedded --test-seat 3 --baseline normal
```

**Option C**: Position doesn't matter because of systematic rotation
```bash
# System automatically uses policy index 3 as test, 0-2 as baseline
# With rotation, all policies play in all seats anyway
```

**Fix**: Clarify in spec:
```bash
# Simple mode: --ai-test creates 3 baseline + 1 test automatically
# Test policy is always at index 3 by convention
# --test-seat is NOT needed (meaningless with systematic rotation)
mdhearts eval 200 --ai-test embedded --baseline normal --weights w.json --rotation systematic

# If you need different configuration, use --ai-per-seat:
mdhearts eval 200 --ai-per-seat normal,embedded,normal,normal  # Test at index 1
```

And document the convention clearly.

---

### 3. **Rotation Mode and Comparison Mode Coupling Not Enforced** ⚠️⚠️

**Severity**: Critical (allows invalid configurations)

**Problem**: Rotation and comparison modes have implicit requirements that aren't validated.

**Invalid Configurations**:

1. **Systematic rotation without comparison mode**:
```bash
# User runs with rotation but OutputMode::Standard
mdhearts eval 200 --ai-per-seat normal,normal,normal,embedded --rotation systematic

# Result: Per-seat statistics, but each "seat" played in all positions!
# Seat 0 avg: 7.0  ← What does this mean? Policy 0 average!
# This is confusing because "seat" implies physical position
```

2. **Comparison mode with non-homogeneous baselines**:
```bash
# 3 different baseline types + 1 test
mdhearts eval 200 --ai-per-seat easy,normal,hard,embedded

# Which policies are "baseline" for comparison?
# System doesn't know!
```

**Fix**: Add validation rules:

```rust
pub fn run_mixed_eval(config: MixedEvalConfig) -> Result<MixedEvalResults, EvalError> {
    // Validation 1: Systematic rotation requires comparison mode
    if config.rotation_mode == RotationMode::Systematic {
        match config.output_mode {
            OutputMode::Standard => {
                return Err(EvalError::InvalidConfig(
                    "Systematic rotation requires OutputMode::Comparison. \
                     Use --comparison or switch to --rotation fixed.".into()
                ));
            }
            OutputMode::Comparison { test_policy_index } => {
                // Validate test_policy_index is in range
                if test_policy_index >= 4 {
                    return Err(EvalError::InvalidConfig(
                        format!("test_policy_index {} out of range [0,3]", test_policy_index)
                    ));
                }
            }
            OutputMode::Detailed => {
                // OK - per-game results make sense with rotation
            }
        }
    }

    // Validation 2: Comparison mode requires homogeneous baseline
    if let OutputMode::Comparison { test_policy_index } = config.output_mode {
        let baseline_types: Vec<_> = config.seat_configs.iter()
            .enumerate()
            .filter(|(i, _)| *i != test_policy_index)
            .map(|(_, cfg)| cfg.ai_type)
            .collect();

        if !baseline_types.windows(2).all(|w| w[0] == w[1]) {
            return Err(EvalError::InvalidConfig(
                "Comparison mode requires homogeneous baseline \
                 (all non-test policies must be same type)".into()
            ));
        }
    }

    // ... rest of implementation
}
```

---

## Important Issues (SHOULD FIX)

### 4. **SeatResults vs PolicyResults Confusion** ⚠️⚠️

**Severity**: Important (confusing terminology)

**Problem**: With rotation, the concept of "seat results" is ambiguous.

```rust
// Line 277-285
pub struct SeatResults {
    pub seat_index: usize,  // ← Is this physical seat or policy index?
    pub ai_label: String,
    pub avg_points: f64,
    // ...
}
```

With `RotationMode::Systematic`, policies rotate through seats. The results are tracked per-policy, not per-seat.

**Fix**: Rename to `PolicyResults` or add mode-specific result types:

```rust
pub struct PolicyResults {
    pub policy_index: usize,  // Clear: this is a policy
    pub ai_label: String,
    pub avg_points: f64,
    pub games_played: usize,  // How many games this policy played
    pub positions_played: Vec<usize>,  // Which physical seats it played in
}

pub struct MixedEvalResults {
    pub games_played: usize,
    pub policy_results: [PolicyResults; 4],  // Per-policy stats
    pub comparison: Option<ComparisonResults>,
    pub rotation_mode: RotationMode,  // Important for interpreting results
}
```

---

### 5. **Sample Size Validation Too Simplistic** ⚠️⚠️

**Severity**: Important (statistical validity)

**Problem**: Line 472 checks `if results.len() >= 30` but doesn't validate both sample sizes for Mann-Whitney U.

```rust
let significance = if results.len() >= 30 {
    Some(mann_whitney_u_test(&test_scores, &baseline_scores))
} else {
    None
};
```

**Issues**:
1. Mann-Whitney U normal approximation requires **both** n1, n2 >= 20
2. With 30 games, test has n1=30, baseline has n2=90 (3 baselines) → both OK
3. But with asymmetric configurations (e.g., 1 test + 1 baseline), n2=30, not 90
4. The check doesn't account for this

**Fix**:
```rust
let n_test = test_scores.len();
let n_baseline = baseline_scores.len();

let significance = if n_test >= 20 && n_baseline >= 20 {
    Some(mann_whitney_u_test(&test_scores, &baseline_scores))
} else {
    eprintln!(
        "Warning: Sample sizes too small for normal approximation \
         (test={}, baseline={}). P-value may be inaccurate. \
         Consider using exact Mann-Whitney U or increasing games to 100+.",
        n_test, n_baseline
    );
    None
};
```

---

### 6. **Missing Helper Functions** ⚠️

**Severity**: Important (implementation incomplete)

**Problem**: Several functions are called but not defined.

**Missing Functions**:
1. `mean(&scores)` - line 461, 462
2. `create_policy(ai_type, weights_path)` - line 392
3. `aggregate_results(results, config)` - line 354
4. `print_progress(results, current, total)` - line 349

**Fix**: Add implementations or reference to where they're defined:

```rust
/// Compute arithmetic mean
fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// Create policy instance from config
fn create_policy(
    ai_type: AiType,
    weights_path: Option<&PathBuf>
) -> Result<Box<dyn Policy>, EvalError> {
    match ai_type {
        AiType::Easy => Ok(Box::new(HeuristicPolicy::easy())),
        AiType::Normal => Ok(Box::new(HeuristicPolicy::normal())),
        AiType::Hard => Ok(Box::new(HeuristicPolicy::hard())),
        AiType::Embedded => {
            let path = weights_path.ok_or(EvalError::MissingWeights)?;
            Ok(Box::new(EmbeddedPolicy::from_file(path)?))
        }
    }
}
```

---

### 7. **Default Rotation Mode Not Specified** ⚠️

**Severity**: Important (ambiguous behavior)

**Problem**: What happens if user doesn't specify `--rotation`?

```bash
# User runs without --rotation flag
mdhearts eval 200 --ai-test embedded --baseline normal --weights w.json

# What rotation mode is used? Fixed? Systematic? Random?
```

**Fix**: Document and implement default behavior:

**Option A - Systematic by default (recommended)**:
```rust
// Default to systematic for fairness
let rotation_mode = args.parse_rotation().unwrap_or(RotationMode::Systematic);
```

**Option B - Fixed by default (backward compatible)**:
```rust
// Default to fixed for backward compatibility
let rotation_mode = args.parse_rotation().unwrap_or(RotationMode::Fixed);
```

Recommendation: **Systematic by default** for `--ai-test` mode, **Fixed** for `--ai-per-seat` mode.

```rust
let rotation_mode = if using_ai_test_mode {
    args.parse_rotation().unwrap_or(RotationMode::Systematic)
} else {
    args.parse_rotation().unwrap_or(RotationMode::Fixed)
};
```

---

### 8. **Rotation Section References Deleted CLI Example** ⚠️

**Severity**: Important (documentation error)

**Problem**: The "Rotation Testing" section (lines 793-848) still refers to testing in "all 4 positions" separately, but this contradicts the systematic rotation approach.

```bash
# Line 797-806: OLD approach
# Test in all 4 positions
for seat in 0 1 2 3; do
    mdhearts eval 200 --ai-test embedded --test-seat $seat \
                      --baseline normal --weights final_weights.json \
                      --json > results_seat_${seat}.json
done
```

This uses `--test-seat` which is incompatible with the simplified `--ai-test` mode.

**Fix**: Update section to match new design:

```bash
# NEW approach: Single command with systematic rotation
mdhearts eval 200 --ai-test embedded --baseline normal \
                  --weights final_weights.json \
                  --rotation systematic \
                  --json > rotation_results.json

# The system automatically rotates policies through all 4 positions
# No need for manual loop or --test-seat
```

Or, if you want per-position breakdown for debugging:

```bash
# Advanced: Test in specific positions (Fixed rotation)
for idx in 0 1 2 3; do
    mdhearts eval 50 --ai-per-seat \
        "$(build_config_with_test_at_index $idx)" \
        --rotation fixed \
        --json > results_position_${idx}.json
done
```

---

### 9. **Comparison Output Shows "test_seat" Field** ⚠️

**Severity**: Important (terminology consistency)

**Problem**: JSON output (line 612-620) still uses `test_seat`:

```json
"comparison": {
  "test_seat": 3,  // ← Should be "test_policy_index"
  "test_avg": 5.76,
  "baseline_avg": 7.06,
  ...
}
```

**Fix**: Rename in JSON output:
```json
"comparison": {
  "test_policy_index": 3,  // Clear!
  "test_avg": 5.76,
  "baseline_avg": 7.06,
  ...
}
```

Or use more descriptive names:
```json
"comparison": {
  "test_policy": {
    "index": 3,
    "ai_type": "Embedded",
    "avg_points": 5.76
  },
  "baseline_policies": {
    "indices": [0, 1, 2],
    "ai_type": "Normal",
    "avg_points": 7.06
  },
  ...
}
```

---

### 10. **CLI Parsing Logic Incomplete** ⚠️

**Severity**: Important (implementation gap)

**Problem**: The CLI parsing sketch (lines 556-595) is incomplete and has placeholder error handling.

```rust
let test_ai = parse_ai_type(args.next().ok_or(...)?)?;
//                                          ^^^
// What error? String literal? EvalError variant?
```

**Fix**: Provide complete implementations or at least proper error types:

```rust
"--ai-test" => {
    args.next(); // consume flag
    let test_ai = args.next()
        .ok_or(EvalError::MissingArgument("--ai-test requires AI type".into()))?;
    let test_ai_type = parse_ai_type(&test_ai)?;

    let baseline_ai = args.next_if_flag("--baseline")
        .ok_or(EvalError::MissingArgument("--ai-test requires --baseline".into()))?;
    let baseline_type = parse_ai_type(&baseline_ai)?;

    let weights_path = args.next_if_flag("--weights");
    let rotation = args.next_if_flag("--rotation")
        .map(|s| parse_rotation_mode(&s))
        .transpose()?
        .unwrap_or(RotationMode::Systematic);  // Default

    mixed_config = Some(create_test_vs_baseline_config(
        test_ai_type,
        baseline_type,
        weights_path,
        rotation,
        num_games
    ));
}
```

---

## Minor Issues

### 11. **Ranking Calculation Comment Clarity** ⚠️

**Severity**: Minor (documentation)

**Problem**: The comment at line 505 says "Average rank for tied values" but doesn't explain the formula.

```rust
let avg_rank = (i + j + 1) as f64 / 2.0;
```

**Fix**: Add explanation:
```rust
// Average rank for tied values at positions [i, j)
// Ranks are 1-indexed: position i has rank (i+1)
// For tie from i to j-1, average of ranks (i+1) to j
// = ((i+1) + j) / 2 = (i + j + 1) / 2
let avg_rank = (i + j + 1) as f64 / 2.0;
```

---

### 12. **Error Function Implementation Note** ⚠️

**Severity**: Minor (could be improved)

**Problem**: The `erf()` function (lines 536-548) uses Abramowitz and Stegun approximation but doesn't cite it.

**Fix**: Add reference:
```rust
/// Error function approximation using Abramowitz and Stegun formula
/// Maximum error: 1.5e-7
/// Reference: Abramowitz and Stegun, "Handbook of Mathematical Functions" (1964)
/// Formula 7.1.26
fn erf(x: f64) -> f64 {
    // Constants from Abramowitz and Stegun
    let a1 = 0.254829592;
    // ...
}
```

Also, consider using a crate instead:
```rust
// Use libm crate for better accuracy
use libm::erf;
```

---

### 13. **Progress Reporting Frequency Hardcoded** ⚠️

**Severity**: Minor (flexibility)

**Problem**: Line 348 hardcodes progress every 20 games:

```rust
if (game_idx + 1) % 20 == 0 {
    print_progress(&results, game_idx + 1, config.num_games);
}
```

**Fix**: Make configurable or adaptive:
```rust
let progress_interval = (config.num_games / 10).max(1);  // 10% increments
if (game_idx + 1) % progress_interval == 0 {
    print_progress(&results, game_idx + 1, config.num_games);
}
```

---

### 14. **Moon Shooter Remapping Edge Case** ⚠️

**Severity**: Minor (edge case)

**Problem**: Line 432 remaps moon shooter:
```rust
moon_shooter: state.moon_shooter().map(|s| seat_mapping[s]),
```

This is correct, but what if multiple players shoot the moon in one game? (Impossible in standard Hearts, but worth documenting.)

**Fix**: Add comment:
```rust
// Remap moon shooter physical seat to policy index
// Note: Hearts rules allow at most one moon shooter per game
moon_shooter: state.moon_shooter().map(|s| seat_mapping[s]),
```

---

### 15. **Output Example Shows Wins but Not Defined** ⚠️

**Severity**: Minor (incomplete)

**Problem**: Line 284 defines `win_count` in `SeatResults`, and output examples show it (line 489), but how is it computed?

```rust
pub struct SeatResults {
    pub win_count: usize,  // Times had lowest score in game
}
```

But with rotation, a "win" is when the policy has the lowest score among all 4 policies in that game.

**Fix**: Document clearly:
```rust
/// Number of games where this policy had the lowest score
/// (wins in Hearts = lowest points)
/// In case of tie, all tied policies count as winners
pub win_count: usize,
```

---

## Summary

### Critical Issues to Fix (3)
1. ⚠️⚠️⚠️ Rename `test_seat` → `test_policy_index` throughout
2. ⚠️⚠️⚠️ Specify how `--ai-test` works without explicit policy indices
3. ⚠️⚠️ Add validation for rotation+comparison mode coupling

### Important Issues to Fix (7)
4. ⚠️⚠️ Rename `SeatResults` → `PolicyResults`
5. ⚠️⚠️ Improve sample size validation for Mann-Whitney U
6. ⚠️ Add missing helper function implementations
7. ⚠️ Specify default rotation mode
8. ⚠️ Update rotation section to match new design
9. ⚠️ Fix JSON output field names
10. ⚠️ Complete CLI parsing logic

### Minor Issues to Fix (5)
11. ⚠️ Clarify ranking calculation comments
12. ⚠️ Add citation for erf() implementation
13. ⚠️ Make progress reporting configurable
14. ⚠️ Document moon shooter edge case
15. ⚠️ Define win_count computation

### Issues Resolved from v1.0 (5) ✅
1. ✅ Array ordering bug fixed
2. ✅ Mann-Whitney U test implemented
3. ✅ Bonferroni correction added
4. ✅ Versioned JSON format
5. ✅ Systematic rotation added

---

## Recommendation

**Status**: Requires one more revision before implementation

**Priority Actions**:
1. **CRITICAL**: Fix terminology throughout (test_seat → test_policy_index)
2. **CRITICAL**: Define `--ai-test` behavior precisely
3. **CRITICAL**: Add validation for mode combinations
4. **IMPORTANT**: Complete missing implementations

**Estimated Revision Time**: 4-6 hours

After addressing critical issues, the design will be ready for implementation.

---

**Document End**
