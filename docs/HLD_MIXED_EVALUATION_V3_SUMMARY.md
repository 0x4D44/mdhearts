# HLD Mixed Evaluation - v3.0 Revision Summary

## Document Information
- **Date**: October 7, 2025
- **Revised By**: Claude Code
- **Document**: HLD_MIXED_EVALUATION.md v3.0
- **Previous Version**: v2.0 (15 issues identified in second review)

## Executive Summary

This is the final revision of the Mixed AI Evaluation HLD, addressing all 15 issues identified in the second review. The design is now **ready for implementation**.

**Status**: ✅ All critical, important, and minor issues resolved

---

## Issues Resolved

### Critical Issues (3) - ALL FIXED ✅

#### 1. Terminology: test_seat → test_policy_index

**Problem**: With systematic rotation, policies move between physical seats, making `test_seat` terminology confusing.

**Solution**: Renamed consistently throughout:
- `OutputMode::Comparison { test_policy_index: usize }`
- `ComparisonResults::test_policy_index`
- All function parameters and documentation

**Impact**: API is now clear and unambiguous.

---

#### 2. Clarified --ai-test Behavior

**Problem**: Unclear how `--ai-test` works without explicit seat/policy indices.

**Solution**: Documented and implemented clear convention:
```bash
# Simple mode - creates [baseline, baseline, baseline, test] automatically
mdhearts eval 200 --ai-test embedded --baseline normal --weights w.json

# Convention:
# - Test policy is always at index 3
# - Baseline policies are indices 0-2
# - Systematic rotation is DEFAULT (each policy plays all 4 positions)
# - No need for --test-seat or --test-policy-index flags
```

**Default Behaviors**:
- `--ai-test`: Rotation = Systematic (RECOMMENDED)
- `--ai-per-seat`: Rotation = Fixed (for debugging)

**Impact**: User-friendly API with sensible defaults.

---

#### 3. Added Configuration Validation

**Problem**: No validation for incompatible mode combinations.

**Solution**: Implemented `validate_config()` function with 4 checks:

```rust
fn validate_config(config: &MixedEvalConfig) -> Result<(), EvalError> {
    // 1. Systematic rotation requires num_games % 4 == 0
    // 2. test_policy_index must be in [0,3]
    // 3. Comparison mode requires homogeneous baseline
    // 4. Warn about systematic rotation with Standard output mode
}
```

**Error Messages**: Helpful suggestions for fixing issues:
```
Error: Systematic rotation requires num_games divisible by 4 (got 203).
Use 200 or 204 games instead.
```

**Impact**: Catches user errors early with clear guidance.

---

### Important Issues (7) - ALL FIXED ✅

#### 4. Renamed SeatResults → PolicyResults

**Rationale**: With rotation, we track policies, not physical seats.

**Changes**:
```rust
pub struct PolicyResults {
    pub policy_index: usize,    // Was: seat_index
    pub ai_type: AiType,        // NEW: type of AI
    pub ai_label: String,       // User-friendly name
    pub avg_points: f64,
    // ...
}

pub struct MixedEvalResults {
    pub policy_results: [PolicyResults; 4],  // Was: seat_results
    pub rotation_mode: RotationMode,         // NEW: important context
    // ...
}
```

**Impact**: Clear semantics, no confusion about what's being measured.

---

#### 5. Improved Sample Size Validation

**Problem**: Only checked `results.len() >= 30`, didn't validate both samples.

**Solution**: Check both sample sizes:
```rust
let n_test = test_scores.len();        // e.g., 200
let n_baseline = baseline_scores.len(); // e.g., 600 (3 baselines × 200)

let significance = if n_test >= 20 && n_baseline >= 20 {
    Some(mann_whitney_u_test(&test_scores, &baseline_scores))
} else {
    eprintln!(
        "Warning: Sample sizes too small (test={}, baseline={}). \
         P-value omitted. Use 100+ games for reliable testing.",
        n_test, n_baseline
    );
    None
};
```

**Impact**: Statistically rigorous, warns users when tests are unreliable.

---

#### 6. Added Missing Helper Functions

**Implementations**:
```rust
fn mean(values: &[f64]) -> f64 { ... }

fn create_policy(ai_type: AiType, weights_path: Option<&PathBuf>)
    -> Result<Box<dyn Policy>, EvalError> { ... }

fn parse_ai_type(s: &str) -> Result<AiType, CliError> { ... }

fn parse_rotation_mode(s: &str) -> Result<RotationMode, CliError> { ... }
```

**Impact**: Complete, self-contained implementation.

---

#### 7. Specified Default Rotation Modes

**Decision**:
- `--ai-test`: Defaults to **Systematic** (fairness)
- `--ai-per-seat`: Defaults to **Fixed** (explicit control)

**Rationale**:
- Systematic rotation is ALWAYS correct for fair comparisons
- Fixed rotation useful for debugging specific configurations
- Different defaults match different use cases

**Impact**: Best practice by default, flexibility when needed.

---

#### 8. Updated Rotation Testing Section

**Old Approach** (incompatible with design):
```bash
# OLD: Manually test in each position separately
for seat in 0 1 2 3; do
    mdhearts eval 200 --test-seat $seat ...
done
```

**New Approach** (matches actual design):
```bash
# NEW: Single command with automatic rotation
mdhearts eval 200 --ai-test embedded --baseline normal --weights w.json

# Automatic rotation:
# Games 0-49:   [Normal, Normal, Normal, Embedded]
# Games 50-99:  [Embedded, Normal, Normal, Normal]
# Games 100-149: [Normal, Embedded, Normal, Normal]
# Games 150-199: [Normal, Normal, Embedded, Normal]
```

**Impact**: Documentation matches implementation.

---

#### 9. Fixed JSON Output Field Names

**Changes**:
```json
{
  "policy_results": [          // Was: seat_results
    {
      "policy_index": 0,       // Was: seat_index
      "ai_type": "Normal",     // NEW
      "ai_label": "Baseline",  // NEW
      ...
    }
  ],
  "comparison": {
    "test_policy_index": 3,    // Was: test_seat
    "statistical_test": "mann_whitney_u",  // NEW
    ...
  },
  "rotation_mode": "Systematic"  // NEW
}
```

**Impact**: JSON format is self-documenting and unambiguous.

---

#### 10. Complete CLI Parsing Implementation

**Features**:
- Full error handling with descriptive messages
- Proper flag parsing with peek/next pattern
- Validation of required arguments
- Smart defaults based on mode
- Helper functions for type parsing

**Example Error Handling**:
```rust
let test_ai_str = args.next()
    .ok_or(CliError::MissingArgument(
        "--ai-test requires AI type (e.g., 'embedded')".into()
    ))?;
```

**Impact**: Production-ready CLI implementation.

---

### Minor Issues (5) - ALL FIXED ✅

#### 11. Added Ranking Calculation Comments

**Before**:
```rust
let avg_rank = (i + j + 1) as f64 / 2.0;  // Unclear formula
```

**After**:
```rust
// Average rank for tied values at positions [i, j)
// Ranks are 1-indexed: position i has rank (i+1)
// For tie from positions i to j-1, average of ranks (i+1) to j is:
// = ((i+1) + j) / 2 = (i + j + 1) / 2
let avg_rank = (i + j + 1) as f64 / 2.0;
```

**Impact**: Maintainable code with clear mathematical reasoning.

---

#### 12. Added erf() Citation

**Added**:
```rust
/// Error function approximation using Abramowitz and Stegun formula
///
/// Maximum error: 1.5e-7
///
/// Reference: Abramowitz and Stegun, "Handbook of Mathematical Functions" (1964)
/// Formula 7.1.26
///
/// Note: Consider using libm crate for production code for better accuracy
fn erf(x: f64) -> f64 { ... }
```

**Impact**: Proper academic citation, notes about accuracy.

---

#### 13. Made Progress Reporting Adaptive

**Before**:
```rust
if (game_idx + 1) % 20 == 0 { ... }  // Hardcoded
```

**After**:
```rust
let progress_interval = (config.num_games / 10).max(1);  // 10% increments
if (game_idx + 1) % progress_interval == 0 { ... }
```

**Impact**: Works well for any game count (10 games or 10,000 games).

---

#### 14. Documented Moon Shooter Edge Case

**Added**:
```rust
// Remap moon shooter physical seat to policy index
// Note: Hearts rules allow at most one moon shooter per game
let moon_shooter_policy = state.moon_shooter().map(|physical_seat| seat_mapping[physical_seat]);
```

**Impact**: Clear assumptions documented.

---

#### 15. Clarified win_count Computation

**Added**:
```rust
/// Number of games where this policy had the lowest score
/// (wins in Hearts = lowest points)
/// In case of tie, all tied policies count as winners
pub win_count: usize,
```

**Impact**: Unambiguous metric definition.

---

## Key Design Decisions

### 1. Systematic Rotation as Default

**Rationale**: Eliminates position bias automatically, making fair comparisons easy.

**User Experience**:
```bash
# User just writes this - rotation happens automatically
mdhearts eval 200 --ai-test embedded --baseline normal --weights w.json
```

**Alternative (if needed)**:
```bash
# Advanced users can disable rotation for debugging
mdhearts eval 200 --ai-test embedded --baseline normal --weights w.json --rotation fixed
```

---

### 2. Policy-Centric Terminology

**Decision**: Use "policy" instead of "seat" when referring to AI instances.

**Rationale**:
- Physical seats are fixed (0, 1, 2, 3)
- Policies rotate through seats
- Results track policies, not seats

**Impact**: Clear mental model, no confusion.

---

### 3. Test Policy at Index 3 by Convention

**Decision**: For `--ai-test` mode, test policy is always at index 3.

**Rationale**:
- Simple, predictable convention
- Doesn't matter with rotation (all positions tested equally)
- Easy to document and understand

---

### 4. No Bonferroni Correction Needed

**Key Insight**: With systematic rotation, we do ONE comparison:
- Test policy: 200 games
- Baseline policies: 600 games (3 × 200)

Not 4 separate comparisons (one per position), so no multiple testing issue.

**Impact**: Simpler, more powerful statistical test.

---

## Implementation Readiness Checklist

### Design ✅
- [x] All conceptual issues resolved
- [x] Terminology consistent throughout
- [x] Data structures well-defined
- [x] Validation logic specified

### Implementation ✅
- [x] Core algorithm specified (`run_mixed_eval`)
- [x] Helper functions defined
- [x] CLI parsing complete
- [x] Error handling comprehensive

### Documentation ✅
- [x] Examples updated
- [x] Edge cases documented
- [x] Citations added
- [x] Comments explain reasoning

### Testing Plan ✅
- [x] Unit test scenarios identified
- [x] Integration test examples provided
- [x] Performance benchmarks planned

---

## Timeline

**Original Estimate (v1.0)**: 1 week (unrealistic)
**Revised Estimate (v2.0)**: 3-4 weeks
**Current Estimate (v3.0)**: 3-4 weeks (no change - complexity accurately captured)

**Breakdown**:
- Week 1: Core infrastructure (eval module, rotation, policies)
- Week 2: Statistics (Mann-Whitney U, comparisons, validation)
- Week 2-3: JSON versioning and backward compatibility
- Week 3: CLI implementation and error handling
- Week 4: Testing, documentation, benchmarks

---

## Remaining Work

**None in design** - ready to implement.

**During Implementation**, watch for:
1. Edge cases in rotation logic (off-by-one errors)
2. Borrow checker issues with policy array
3. Performance of Mann-Whitney U for large samples
4. Memory usage with multiple policies

**But these are implementation details, not design issues.**

---

## Comparison: v1.0 → v2.0 → v3.0

| Aspect | v1.0 | v2.0 | v3.0 |
|--------|------|------|------|
| **Critical Issues** | 5 | 0 (fixed) | 0 (all fixed) |
| **Terminology** | Confusing (seat/policy mixed) | Partially fixed | ✅ Consistent |
| **Statistical Methods** | Wrong (t-test) | ✅ Correct (Mann-Whitney U) | ✅ Correct + validated |
| **Rotation** | Missing | ✅ Added | ✅ Refined (defaults, docs) |
| **CLI Design** | Incomplete | Sketched | ✅ Fully specified |
| **Validation** | None | Partial | ✅ Comprehensive |
| **Documentation** | Basic | Improved | ✅ Complete |
| **Status** | Not implementable | Needs revision | **Ready for implementation** |

---

## Recommendation

**🎯 Proceed with implementation**

The design is now:
- ✅ Mathematically sound
- ✅ Statistically rigorous
- ✅ Terminologically consistent
- ✅ Fully specified
- ✅ Well-documented
- ✅ Production-ready

**No further design revisions needed.**

---

## References

- **HLD v3.0**: [HLD_MIXED_EVALUATION.md](./HLD_MIXED_EVALUATION.md)
- **First Review**: [HLD_MIXED_EVALUATION_REVIEW.md](./HLD_MIXED_EVALUATION_REVIEW.md) (36 issues)
- **Second Review**: [HLD_MIXED_EVALUATION_REVIEW_V2.md](./HLD_MIXED_EVALUATION_REVIEW_V2.md) (15 issues)
- **v2.0 Summary**: [HLD_MIXED_EVALUATION_REVISION_SUMMARY.md](./HLD_MIXED_EVALUATION_REVISION_SUMMARY.md)

---

**Document End**
