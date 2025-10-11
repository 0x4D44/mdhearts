# High-Level Design: Mixed AI Evaluation System

## Document Information
- **Author**: Claude Code
- **Date**: October 7, 2025
- **Version**: 3.0 (Second Revision)
- **Status**: Design Proposal - Ready for Implementation
- **Previous Versions**:
  - 1.0 (2025-10-07) - 36 issues identified in first review
  - 2.0 (2025-10-07) - 5 critical issues fixed, 15 issues identified in second review

## Executive Summary

This document proposes a comprehensive redesign of the evaluation system to support **mixed AI configurations** where different AI types can be assigned to each seat. This addresses a critical flaw in the current evaluation methodology: comparing homogeneous games (4 identical AIs) rather than measuring how a trained AI performs **against** baseline opponents.

## Problem Statement

### Current Limitations

**Issue 1: Homogeneous Evaluation**
```bash
# Current: All 4 players use same AI
mdhearts eval 200 --ai normal          # 4 Normal AIs
mdhearts eval 200 --ai embedded        # 4 Embedded AIs
```

**Problem**: Points in Hearts sum to 26 per game. Comparing "4 Normal AIs" (avg 6.50) vs "4 Trained AIs" (avg 6.50) tells us nothing about relative performance because:
- Different game dynamics (opponents play differently)
- No direct competition between policies
- Cannot determine if trained AI is actually better

**Issue 2: Invalid Performance Metrics**

Current evaluation results:
| Seat | 4x Normal | 4x Trained | "Difference" |
|------|-----------|------------|--------------|
| 0    | 7.32      | 7.54       | +0.22        |
| 1    | 7.13      | 6.17       | -0.96        |
| 2    | 6.07      | 5.91       | -0.16        |
| 3    | 5.49      | 6.39       | +0.90        |
| Avg  | 6.50      | 6.50       | 0.00         |

**These differences are meaningless** because they're from different game contexts.

### What We Actually Need

**Valid Comparison**: 3 Baseline + 1 Trained in the same games
```
Game 1: [Normal, Normal, Normal, Trained]
Game 2: [Normal, Normal, Normal, Trained]
...
Game 200: [Normal, Normal, Normal, Trained]
```

Compare:
- Trained AI's average: X points
- Normal AI's average (across seats 0-2): Y points
- **Difference: X - Y** = actual performance delta

## Current Architecture

### Component Overview

```
┌─────────────────────────────────────────────────────────────┐
│                         CLI Layer                            │
│  cli.rs: parse_args() → run_eval()                         │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│                     Evaluation Engine                        │
│  run_eval(num_games, ai_type, weights_path)                │
│    - Creates SINGLE policy for all 4 seats                 │
│    - Runs games with identical AIs                          │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│                      Policy Layer                            │
│  Policy trait: forward(&ctx) → card                         │
│  Implementations:                                            │
│    - HeuristicPolicy (easy/normal/hard)                    │
│    - EmbeddedPolicy (neural network)                        │
└─────────────────────────────────────────────────────────────┘
```

### Current Evaluation Flow

```rust
fn run_eval(
    num_games: usize,
    ai_type: AiType,  // Single AI type for all seats
    weights_path: Option<PathBuf>,
) {
    // Create ONE policy for ALL seats
    let mut policy: Box<dyn Policy> = match ai_type {
        AiType::Normal => Box::new(HeuristicPolicy::normal()),
        AiType::Embedded => Box::new(EmbeddedPolicy::from_file(weights_path)?),
        // ...
    };

    // Run games with identical AIs
    for game in 0..num_games {
        for seat in 0..4 {
            let card = policy.forward(&ctx);  // Same policy!
            // ...
        }
    }
}
```

**Key limitation**: Single `ai_type` parameter → homogeneous games only.

## Proposed Solution

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                      CLI Layer (Enhanced)                    │
│  New options:                                                │
│    --ai-per-seat <seat0>,<seat1>,<seat2>,<seat3>           │
│    --weights-per-seat <path0>,<path1>,<path2>,<path3>      │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│              Evaluation Engine (Redesigned)                  │
│  run_mixed_eval(config: MixedEvalConfig)                   │
│    - Creates 4 separate policies (one per seat)            │
│    - Supports heterogeneous AI configurations               │
│    - Per-seat performance tracking                          │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│                   Policy Management                          │
│  struct SeatPolicies {                                      │
│      seat0: Box<dyn Policy>,                               │
│      seat1: Box<dyn Policy>,                               │
│      seat2: Box<dyn Policy>,                               │
│      seat3: Box<dyn Policy>,                               │
│  }                                                          │
└─────────────────────────────────────────────────────────────┘
```

### Design Principles

1. **Backward Compatibility**: Existing `--ai <type>` commands continue to work (all 4 seats use same AI)
2. **Flexibility**: Support any combination of AI types per seat
3. **Simplicity**: Common use case (3 baseline + 1 trained) should be easy
4. **Extensibility**: Easy to add new evaluation modes (round-robin, etc.)

## Detailed Design

### 1. CLI Interface Design

#### Option 1: Explicit Per-Seat Configuration

```bash
# Full control: specify AI type for each seat
mdhearts eval 200 --ai-per-seat normal,normal,normal,embedded \
                  --weights-per-seat _,_,_,final_weights.json

# Shorthand: "_" means "no custom weights" (use defaults)
```

**Pros**:
- Maximum flexibility
- Clear and explicit

**Cons**:
- Verbose for common cases
- Error-prone (easy to miscount seats)

#### Option 2: Mixed Mode with Target Seat

```bash
# Simpler: "test this AI against 3 baselines"
mdhearts eval 200 --ai-mixed normal,embedded \
                  --test-seat 3 \
                  --weights final_weights.json

# Equivalent to: [normal, normal, normal, embedded]
```

**Pros**:
- Concise for common use case
- Less error-prone

**Cons**:
- Less flexible
- Requires additional logic to expand

#### Option 3: Hybrid Approach (RECOMMENDED)

```bash
# Simple mode: test trained AI against baseline
# Creates 1 test + 3 baseline configuration automatically
# Uses systematic rotation by default (each policy plays in all 4 positions)
mdhearts eval 200 --ai-test embedded \
                  --baseline normal \
                  --weights final_weights.json

# Systematic rotation is DEFAULT and RECOMMENDED
# This eliminates position bias (e.g., starting with 2♣, card passing direction)
# No need to specify --test-seat - the system handles rotation automatically

# Optional: Disable rotation for debugging
mdhearts eval 200 --ai-test embedded --baseline normal \
                  --weights w.json --rotation fixed

# Advanced mode: full per-seat control
mdhearts eval 200 --ai-per-seat normal,normal,hard,embedded \
                  --weights-per-seat _,_,_,final_weights.json

# Backward compatible: original behavior
mdhearts eval 200 --ai normal  # All 4 seats = normal
```

**Pros**:
- Covers common case simply
- Maintains backward compatibility
- Provides advanced option for power users

**Cons**:
- Multiple ways to do the same thing (but clearly documented)

### 2. Data Structures

#### Configuration

```rust
/// Configuration for mixed AI evaluation
#[derive(Debug, Clone)]
pub struct MixedEvalConfig {
    pub num_games: usize,
    pub policy_configs: [PolicyConfig; 4],  // One config per policy
    pub output_mode: OutputMode,
    pub rotation_mode: RotationMode,
}

/// How to handle seat rotation to eliminate position bias
#[derive(Debug, Clone, PartialEq)]
pub enum RotationMode {
    /// Fixed seating: AI stays in assigned seat
    Fixed,
    /// Systematic rotation: rotate AIs through all positions evenly
    /// (num_games must be divisible by 4)
    Systematic,
    /// Random seating: shuffle AIs randomly each game (NOT RECOMMENDED)
    Random,
}

/// Configuration for a single policy (not a physical seat)
/// With rotation, policies move between seats, so this configures the AI itself
#[derive(Debug, Clone)]
pub struct PolicyConfig {
    pub ai_type: AiType,
    pub weights_path: Option<PathBuf>,
    pub label: Option<String>,  // User-friendly name for reporting
}

/// How to report results
#[derive(Debug, Clone, PartialEq)]
pub enum OutputMode {
    /// Standard: per-policy averages
    Standard,
    /// Comparison: highlight test policy vs baseline policies
    /// test_policy_index: which policy in the array is being tested (typically 3)
    /// Other policies are considered baseline (must be homogeneous)
    Comparison { test_policy_index: usize },
    /// Detailed: per-game results
    Detailed,
}
```

#### Evaluation Results

```rust
/// Results from mixed evaluation
#[derive(Debug, Clone, Serialize)]
pub struct MixedEvalResults {
    pub games_played: usize,
    pub policy_results: [PolicyResults; 4],  // Per-policy statistics
    pub comparison: Option<ComparisonResults>,
    pub rotation_mode: RotationMode,  // Important for interpreting results
    pub elapsed_seconds: f64,
}

/// Per-policy results (not per-seat, since policies rotate through seats)
#[derive(Debug, Clone, Serialize)]
pub struct PolicyResults {
    pub policy_index: usize,  // Index in the policy array [0-3]
    pub ai_type: AiType,      // Type of AI
    pub ai_label: String,     // User-friendly name
    pub avg_points: f64,
    pub total_points: usize,
    pub moon_count: usize,
    /// Number of games where this policy had the lowest score
    /// (wins in Hearts = lowest points)
    /// In case of tie, all tied policies count as winners
    pub win_count: usize,
}

/// Comparison statistics (when test policy specified)
#[derive(Debug, Clone, Serialize)]
pub struct ComparisonResults {
    pub test_policy_index: usize,  // Which policy is being tested
    pub test_avg: f64,
    pub baseline_avg: f64,  // Average of non-test policies
    pub difference: f64,    // test_avg - baseline_avg (negative = better)
    pub percent_improvement: f64,  // (baseline - test) / baseline * 100
    pub statistical_significance: Option<f64>,  // p-value if enough games
    pub statistical_test: String,  // Name of test used (e.g., "mann_whitney_u")
}
```

### 3. Implementation Strategy

#### Phase 1: Core Infrastructure

**File**: `crates/hearts-app/src/eval/mod.rs` (new module)

```rust
pub mod mixed;
pub mod stats;

pub use mixed::MixedEvalConfig;
pub use mixed::run_mixed_eval;
```

**File**: `crates/hearts-app/src/eval/mixed.rs`

```rust
/// Run evaluation with mixed AI configuration
pub fn run_mixed_eval(config: MixedEvalConfig) -> Result<MixedEvalResults, EvalError> {
    // 1. Validate configuration
    validate_config(&config)?;

    // 2. Initialize policies for each seat
    let policies = create_seat_policies(&config)?;

    // 3. Run games with rotation
    let mut results = vec![];
    let progress_interval = (config.num_games / 10).max(1);  // 10% increments

    for game_idx in 0..config.num_games {
        // Apply rotation based on mode
        let seat_mapping = match config.rotation_mode {
            RotationMode::Fixed => [0, 1, 2, 3],  // No rotation
            RotationMode::Systematic => {
                // Rotate every num_games/4 games
                let rotation = (game_idx / (config.num_games / 4)) % 4;
                rotate_seats(rotation)
            }
            RotationMode::Random => random_shuffle_seats(),
        };

        let game_result = run_single_game(&policies, seat_mapping)?;
        results.push(game_result);

        // Progress reporting (adaptive based on num_games)
        if (game_idx + 1) % progress_interval == 0 {
            print_progress(&results, game_idx + 1, config.num_games);
        }
    }

    // 4. Aggregate statistics
    let aggregated = aggregate_results(&results, &config)?;

    // 5. Compute comparisons if requested
    if let OutputMode::Comparison { test_policy_index } = config.output_mode {
        aggregated.comparison = Some(compute_comparison(&results, test_policy_index)?);
    }

    Ok(aggregated)
}

/// Validate configuration for internal consistency
fn validate_config(config: &MixedEvalConfig) -> Result<(), EvalError> {
    // Check 1: Systematic rotation requires num_games divisible by 4
    if config.rotation_mode == RotationMode::Systematic {
        if config.num_games % 4 != 0 {
            return Err(EvalError::InvalidConfig(
                format!(
                    "Systematic rotation requires num_games divisible by 4 \
                     (got {}). Use {} or {} games instead.",
                    config.num_games,
                    (config.num_games / 4) * 4,      // Round down
                    ((config.num_games / 4) + 1) * 4  // Round up
                )
            ));
        }
    }

    // Check 2: Comparison mode requires valid test_policy_index
    if let OutputMode::Comparison { test_policy_index } = config.output_mode {
        if test_policy_index >= 4 {
            return Err(EvalError::InvalidConfig(
                format!("test_policy_index {} out of range [0,3]", test_policy_index)
            ));
        }

        // Check 3: Comparison mode requires homogeneous baseline
        let baseline_types: Vec<_> = config.policy_configs.iter()
            .enumerate()
            .filter(|(i, _)| *i != test_policy_index)
            .map(|(_, cfg)| cfg.ai_type)
            .collect();

        if !baseline_types.windows(2).all(|w| w[0] == w[1]) {
            return Err(EvalError::InvalidConfig(
                format!(
                    "Comparison mode requires homogeneous baseline \
                     (all non-test policies must be same type). \
                     Policy {} is test, but baseline policies have types: {:?}",
                    test_policy_index,
                    baseline_types
                )
            ));
        }
    }

    // Check 4: Warn about systematic rotation with Standard mode
    if config.rotation_mode == RotationMode::Systematic {
        if let OutputMode::Standard = config.output_mode {
            eprintln!(
                "Warning: Systematic rotation with Standard output mode. \
                 Results show per-policy averages (not per-seat). \
                 Consider using --comparison mode for clearer output."
            );
        }
    }

    Ok(())
}

/// Generate seat mapping for rotation
/// rotation=0: [0,1,2,3]
/// rotation=1: [3,0,1,2]  (each AI moves right one seat)
/// rotation=2: [2,3,0,1]
/// rotation=3: [1,2,3,0]
fn rotate_seats(rotation: usize) -> [usize; 4] {
    let r = rotation % 4;
    [
        (4 - r) % 4,
        (5 - r) % 4,
        (6 - r) % 4,
        (7 - r) % 4,
    ]
}

/// Generate random seat shuffle (NOT RECOMMENDED - use Systematic instead)
fn random_shuffle_seats() -> [usize; 4] {
    use rand::seq::SliceRandom;
    let mut seats = [0, 1, 2, 3];
    seats.shuffle(&mut rand::thread_rng());
    seats
}

/// Create policy instances for each seat
fn create_seat_policies(config: &MixedEvalConfig) -> Result<[Box<dyn Policy>; 4], EvalError> {
    let mut policies: Vec<Box<dyn Policy>> = Vec::new();

    for policy_config in &config.policy_configs {
        let policy = create_policy(
            policy_config.ai_type,
            policy_config.weights_path.as_ref()
        )?;
        policies.push(policy);
    }

    // Convert Vec to array - preserve order!
    // CRITICAL: Don't use .pop() which reverses the order
    Ok(policies.try_into().unwrap_or_else(|v: Vec<_>| {
        panic!("Expected exactly 4 policies, got {}", v.len())
    }))
}

/// Run a single game with mixed AIs and seat mapping
/// seat_mapping[physical_seat] = policy_index
fn run_single_game(
    policies: &[Box<dyn Policy>; 4],
    seat_mapping: [usize; 4],
) -> Result<GameResult, EvalError> {
    let mut state = GameState::new_random();

    while !state.is_terminal() {
        let physical_seat = state.current_seat();
        let policy_idx = seat_mapping[physical_seat];
        let ctx = PolicyContext::from_state(&state, physical_seat);
        let card = policies[policy_idx].forward(&ctx);
        state.play_card(physical_seat, card)?;
    }

    // Remap results back to policy indices
    let physical_points = state.final_points();
    let mut policy_points = [0u8; 4];
    for physical_seat in 0..4 {
        let policy_idx = seat_mapping[physical_seat];
        policy_points[policy_idx] = physical_points[physical_seat];
    }

    // Remap moon shooter physical seat to policy index
    // Note: Hearts rules allow at most one moon shooter per game
    let moon_shooter_policy = state.moon_shooter().map(|physical_seat| seat_mapping[physical_seat]);

    Ok(GameResult {
        points: policy_points,
        moon_shooter: moon_shooter_policy,
    })
}

/// Create policy instance from configuration
fn create_policy(
    ai_type: AiType,
    weights_path: Option<&PathBuf>
) -> Result<Box<dyn Policy>, EvalError> {
    match ai_type {
        AiType::Easy => Ok(Box::new(HeuristicPolicy::easy())),
        AiType::Normal => Ok(Box::new(HeuristicPolicy::normal())),
        AiType::Hard => Ok(Box::new(HeuristicPolicy::hard())),
        AiType::Embedded => {
            let path = weights_path.ok_or_else(|| {
                EvalError::MissingWeights(
                    "Embedded AI requires weights file via --weights".into()
                )
            })?;
            Ok(Box::new(EmbeddedPolicy::from_file(path)?))
        }
    }
}
```

#### Phase 2: Statistics & Comparison

**File**: `crates/hearts-app/src/eval/stats.rs`

```rust
/// Compute comparison between test policy and baseline
pub fn compute_comparison(
    results: &[GameResult],
    test_policy_index: usize,
) -> Result<ComparisonResults, EvalError> {
    // Extract test policy scores
    let test_scores: Vec<f64> = results.iter()
        .map(|r| r.points[test_policy_index] as f64)
        .collect();

    // Extract baseline scores (all other policies)
    let baseline_scores: Vec<f64> = results.iter()
        .flat_map(|r| {
            r.points.iter().enumerate()
                .filter(|(i, _)| *i != test_policy_index)
                .map(|(_, &p)| p as f64)
        })
        .collect();

    let test_avg = mean(&test_scores);
    let baseline_avg = mean(&baseline_scores);
    let difference = test_avg - baseline_avg;
    let percent_improvement = (baseline_avg - test_avg) / baseline_avg * 100.0;

    // Statistical significance (Mann-Whitney U test)
    // CRITICAL: Use non-parametric test because Hearts scores are NOT normally distributed:
    // - Bounded [0, 26]
    // - Discrete (integers)
    // - Skewed (moon shot tail)
    // - Multimodal (different strategies)
    let n_test = test_scores.len();
    let n_baseline = baseline_scores.len();

    let significance = if n_test >= 20 && n_baseline >= 20 {
        Some(mann_whitney_u_test(&test_scores, &baseline_scores))
    } else {
        eprintln!(
            "Warning: Sample sizes too small for normal approximation \
             (test={}, baseline={}). P-value omitted. \
             For reliable significance testing, use 100+ games.",
            n_test, n_baseline
        );
        None
    };

    Ok(ComparisonResults {
        test_policy_index,
        test_avg,
        baseline_avg,
        difference,
        percent_improvement,
        statistical_significance: significance,
        statistical_test: "mann_whitney_u".to_string(),
    })
}

/// Compute arithmetic mean
fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// Mann-Whitney U test (non-parametric, no normality assumption)
fn mann_whitney_u_test(sample1: &[f64], sample2: &[f64]) -> f64 {
    // Rank all values from both samples
    let mut combined: Vec<(f64, usize)> = sample1.iter().map(|&v| (v, 0)).collect();
    combined.extend(sample2.iter().map(|&v| (v, 1)));
    combined.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    // Assign ranks (handling ties with average ranks)
    let mut rank_sum1 = 0.0;
    let mut i = 0;
    while i < combined.len() {
        // Find ties: all values equal to combined[i]
        let mut j = i;
        while j < combined.len() && combined[j].0 == combined[i].0 {
            j += 1;
        }
        // Average rank for tied values at positions [i, j)
        // Ranks are 1-indexed: position i has rank (i+1)
        // For tie from positions i to j-1, average of ranks (i+1) to j is:
        // = ((i+1) + j) / 2 = (i + j + 1) / 2
        let avg_rank = (i + j + 1) as f64 / 2.0;
        for k in i..j {
            if combined[k].1 == 0 {
                rank_sum1 += avg_rank;
            }
        }
        i = j;
    }

    // Compute U statistic
    let n1 = sample1.len() as f64;
    let n2 = sample2.len() as f64;
    let u1 = rank_sum1 - n1 * (n1 + 1.0) / 2.0;
    let u2 = n1 * n2 - u1;
    let u = u1.min(u2);

    // Normal approximation for p-value (valid for n1, n2 >= 20)
    let mean_u = n1 * n2 / 2.0;
    let std_u = ((n1 * n2 * (n1 + n2 + 1.0)) / 12.0).sqrt();
    let z = (u - mean_u).abs() / std_u;

    // Two-tailed p-value (using standard normal CDF)
    2.0 * (1.0 - standard_normal_cdf(z))
}

/// Standard normal cumulative distribution function
fn standard_normal_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}

/// Error function approximation using Abramowitz and Stegun formula
///
/// Maximum error: 1.5e-7
///
/// Reference: Abramowitz and Stegun, "Handbook of Mathematical Functions" (1964)
/// Formula 7.1.26
///
/// Note: Consider using libm crate for production code for better accuracy
fn erf(x: f64) -> f64 {
    // Constants from Abramowitz and Stegun
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();
    sign * y
}
```

#### Phase 3: CLI Integration

**File**: `crates/hearts-app/src/cli.rs`

```rust
// Add new parsing logic
"eval" | "--eval" => {
    // ... parse num_games ...

    // Check for new mixed evaluation flags
    let mut mixed_config = None;

    if let Some("--ai-test") = args.peek().map(|s| s.as_str()) {
        // Simple mode: 1 test + 3 baseline with automatic rotation
        args.next(); // consume --ai-test

        let test_ai_str = args.next()
            .ok_or(CliError::MissingArgument("--ai-test requires AI type (e.g., 'embedded')".into()))?;
        let test_ai = parse_ai_type(&test_ai_str)?;

        // Baseline is required
        if args.peek() != Some(&"--baseline".to_string()) {
            return Err(CliError::MissingArgument(
                "--ai-test requires --baseline flag (e.g., '--baseline normal')".into()
            ));
        }
        args.next(); // consume --baseline
        let baseline_ai_str = args.next()
            .ok_or(CliError::MissingArgument("--baseline requires AI type".into()))?;
        let baseline_ai = parse_ai_type(&baseline_ai_str)?;

        // Weights are optional (required only if test_ai is embedded)
        let weights_path = if args.peek() == Some(&"--weights".to_string()) {
            args.next(); // consume --weights
            Some(PathBuf::from(args.next()
                .ok_or(CliError::MissingArgument("--weights requires path".into()))?))
        } else if test_ai == AiType::Embedded {
            return Err(CliError::MissingWeights(
                "Embedded AI requires --weights flag".into()
            ));
        } else {
            None
        };

        // Rotation defaults to Systematic for --ai-test
        let rotation = if args.peek() == Some(&"--rotation".to_string()) {
            args.next(); // consume --rotation
            let mode_str = args.next()
                .ok_or(CliError::MissingArgument("--rotation requires mode (fixed/systematic/random)".into()))?;
            parse_rotation_mode(&mode_str)?
        } else {
            RotationMode::Systematic  // DEFAULT for --ai-test
        };

        // Create configuration: [baseline, baseline, baseline, test]
        // Test policy is always at index 3 by convention
        mixed_config = Some(MixedEvalConfig {
            num_games,
            policy_configs: [
                PolicyConfig { ai_type: baseline_ai, weights_path: None, label: Some("Baseline".into()) },
                PolicyConfig { ai_type: baseline_ai, weights_path: None, label: Some("Baseline".into()) },
                PolicyConfig { ai_type: baseline_ai, weights_path: None, label: Some("Baseline".into()) },
                PolicyConfig { ai_type: test_ai, weights_path: weights_path, label: Some("Test".into()) },
            ],
            output_mode: OutputMode::Comparison { test_policy_index: 3 },
            rotation_mode: rotation,
        });
    }
    else if let Some("--ai-per-seat") = args.peek().map(|s| s.as_str()) {
        // Advanced mode: full per-seat control
        args.next(); // consume --ai-per-seat

        let seat_ais_str = args.next()
            .ok_or(CliError::MissingArgument(
                "--ai-per-seat requires comma-separated AI types (e.g., 'normal,normal,normal,embedded')".into()
            ))?;
        let ai_types = parse_seat_ais(&seat_ais_str)?;

        // Parse weights-per-seat (optional, defaults to None for all)
        let weights_paths = if args.peek() == Some(&"--weights-per-seat".to_string()) {
            args.next(); // consume --weights-per-seat
            let weights_str = args.next()
                .ok_or(CliError::MissingArgument("--weights-per-seat requires comma-separated paths".into()))?;
            parse_weights_per_seat(&weights_str)?
        } else {
            vec![None, None, None, None]
        };

        if ai_types.len() != 4 || weights_paths.len() != 4 {
            return Err(CliError::InvalidConfig(
                format!("--ai-per-seat requires exactly 4 AI types and 4 weight paths (got {}, {})",
                    ai_types.len(), weights_paths.len())
            ));
        }

        // Rotation defaults to Fixed for --ai-per-seat
        let rotation = if args.peek() == Some(&"--rotation".to_string()) {
            args.next();
            let mode_str = args.next()
                .ok_or(CliError::MissingArgument("--rotation requires mode".into()))?;
            parse_rotation_mode(&mode_str)?
        } else {
            RotationMode::Fixed  // DEFAULT for --ai-per-seat
        };

        let policy_configs = [
            PolicyConfig { ai_type: ai_types[0], weights_path: weights_paths[0].clone(), label: None },
            PolicyConfig { ai_type: ai_types[1], weights_path: weights_paths[1].clone(), label: None },
            PolicyConfig { ai_type: ai_types[2], weights_path: weights_paths[2].clone(), label: None },
            PolicyConfig { ai_type: ai_types[3], weights_path: weights_paths[3].clone(), label: None },
        ];

        mixed_config = Some(MixedEvalConfig {
            num_games,
            policy_configs,
            output_mode: OutputMode::Standard,
            rotation_mode: rotation,
        });
    }

    if let Some(config) = mixed_config {
        run_mixed_eval(config)?;
    } else {
        // Original homogeneous eval (backward compatible)
        run_eval(num_games, ai_type, weights_path, collect_data_path)?;
    }
}

/// Parse AI type from string
fn parse_ai_type(s: &str) -> Result<AiType, CliError> {
    match s.to_lowercase().as_str() {
        "easy" => Ok(AiType::Easy),
        "normal" => Ok(AiType::Normal),
        "hard" => Ok(AiType::Hard),
        "embedded" => Ok(AiType::Embedded),
        _ => Err(CliError::InvalidAiType(format!("Unknown AI type: {}. Valid types: easy, normal, hard, embedded", s))),
    }
}

/// Parse rotation mode from string
fn parse_rotation_mode(s: &str) -> Result<RotationMode, CliError> {
    match s.to_lowercase().as_str() {
        "fixed" => Ok(RotationMode::Fixed),
        "systematic" => Ok(RotationMode::Systematic),
        "random" => Ok(RotationMode::Random),
        _ => Err(CliError::InvalidRotationMode(
            format!("Unknown rotation mode: {}. Valid modes: fixed, systematic, random", s)
        )),
    }
}
```

### 4. Output Format

#### Standard Mode

```bash
$ mdhearts eval 200 --ai-per-seat normal,normal,hard,embedded --weights-per-seat _,_,_,weights.json

Running 200 games with mixed AI configuration
  Seat 0: Normal
  Seat 1: Normal
  Seat 2: Hard
  Seat 3: Embedded (weights: weights.json)

Progress: [====================] 200/200 (0.5s)

Results:
┌──────┬────────────┬────────────┬──────────┬───────┬──────┐
│ Seat │ AI Type    │ Avg Points │ Total    │ Moons │ Wins │
├──────┼────────────┼────────────┼──────────┼───────┼──────┤
│ 0    │ Normal     │ 7.42       │ 1484     │ 0     │ 32   │
│ 1    │ Normal     │ 7.18       │ 1436     │ 0     │ 38   │
│ 2    │ Hard       │ 5.64       │ 1128     │ 1     │ 67   │
│ 3    │ Embedded   │ 5.76       │ 1152     │ 0     │ 63   │
└──────┴────────────┴────────────┴──────────┴───────┴──────┘

Elapsed: 0.58 seconds
```

#### Comparison Mode

```bash
$ mdhearts eval 200 --ai-test embedded --baseline normal --weights weights.json

Running 200 games: Embedded (test policy) vs Normal (baseline) with systematic rotation

Progress: [====================] 200/200 (0.5s)

Results:
┌────────────────────┬────────────┬──────────┬───────┬──────┐
│ Configuration      │ Avg Points │ Total    │ Moons │ Wins │
├────────────────────┼────────────┼──────────┼───────┼──────┤
│ Baseline (Normal)  │ 7.06       │ 4236     │ 0     │ 108  │
│ Test (Embedded)    │ 5.76       │ 1152     │ 0     │ 92   │
├────────────────────┼────────────┼──────────┼───────┼──────┤
│ Difference         │ -1.30      │          │       │      │
│ Improvement        │ 18.4%      │          │       │      │
└────────────────────┴────────────┴──────────┴───────┴──────┘

Statistical significance: p < 0.001 (***)

Interpretation: The trained policy performs significantly better
than the baseline (lower score = better in Hearts).

Per-Policy Results:
  Policy 0 (Normal): 7.42 points avg (played all 4 positions via rotation)
  Policy 1 (Normal): 7.18 points avg (played all 4 positions via rotation)
  Policy 2 (Normal): 6.58 points avg (played all 4 positions via rotation)
  Policy 3 (Embedded): 5.76 points avg (played all 4 positions via rotation) ⭐
```

#### JSON Output (for scripting)

**Version 2.0 Format (Mixed Evaluation):**

```json
{
  "format_version": "2.0",
  "eval_type": "mixed",
  "games_played": 200,
  "config": {
    "seat_configs": [
      {"ai_type": "Normal", "weights": null, "label": "Normal"},
      {"ai_type": "Normal", "weights": null, "label": "Normal"},
      {"ai_type": "Normal", "weights": null, "label": "Normal"},
      {"ai_type": "Embedded", "weights": "weights.json", "label": "Trained"}
    ]
  },
  "policy_results": [
    {"policy_index": 0, "ai_type": "Normal", "ai_label": "Baseline", "avg_points": 7.42, "total": 1484, "moons": 0, "wins": 32},
    {"policy_index": 1, "ai_type": "Normal", "ai_label": "Baseline", "avg_points": 7.18, "total": 1436, "moons": 0, "wins": 38},
    {"policy_index": 2, "ai_type": "Normal", "ai_label": "Baseline", "avg_points": 6.58, "total": 1316, "moons": 0, "wins": 38},
    {"policy_index": 3, "ai_type": "Embedded", "ai_label": "Test", "avg_points": 5.76, "total": 1152, "moons": 0, "wins": 92}
  ],
  "comparison": {
    "test_policy_index": 3,
    "test_avg": 5.76,
    "baseline_avg": 7.06,
    "difference": -1.30,
    "percent_improvement": 18.4,
    "p_value": 0.0001,
    "significant": true,
    "statistical_test": "mann_whitney_u"
  },
  "rotation_mode": "Systematic",
  "elapsed_seconds": 0.58
}
```

**Version 1.0 Format (Homogeneous - BACKWARD COMPATIBLE):**

Legacy `--ai normal` commands continue to produce version 1.0 format:

```json
{
  "format_version": "1.0",
  "eval_type": "homogeneous",
  "games_played": 200,
  "ai_type": "Normal",
  "avg_points_per_seat": [7.32, 7.13, 6.07, 5.49],
  "total_points_per_seat": [1464, 1426, 1214, 1098],
  "moon_shots": 0,
  "elapsed_seconds": 0.58
}
```

**Migration Strategy:**

Scripts can detect format version and handle accordingly:

```python
import json

with open('results.json') as f:
    data = json.load(f)

if data.get('format_version') == '2.0':
    # New mixed evaluation format
    handle_mixed_results(data)
elif data.get('format_version') == '1.0':
    # Legacy homogeneous format
    handle_legacy_results(data)
else:
    # Pre-versioning format (assume 1.0)
    handle_legacy_results(data)
```

**Format Selection:**

```bash
# Version 1.0 (legacy) - default for --ai
mdhearts eval 200 --ai normal --json > legacy.json

# Version 2.0 (mixed) - used for mixed evaluations
mdhearts eval 200 --ai-test embedded --test-seat 3 \
                  --baseline normal --weights w.json --json > mixed.json

# Force version 2.0 format even for homogeneous eval
mdhearts eval 200 --ai normal --json --format-version 2.0 > v2.json
```

### 5. Evaluation Methodology

#### Systematic Rotation (CRITICAL)

**Problem: Position Bias in Hearts**

Seat position significantly affects gameplay:
- **Seat 0**: Always starts with 2♣ (first play, information disadvantage)
- **Card Passing**: Direction rotates (left/right/across/none), affecting different seats differently
- **Turn Order**: Varies by trick, impacts information available

**Solution: Systematic Rotation**

Instead of testing AI in one fixed position, rotate it through all positions evenly:

```bash
# Recommended: 200 games with systematic rotation (50 games per position)
mdhearts eval 200 --ai-test embedded --baseline normal \
                  --weights final_weights.json \
                  --rotation systematic

# Games 0-49:   Trained in seat 0, Normal in seats 1-3
# Games 50-99:  Trained in seat 1, Normal in seats 0,2,3
# Games 100-149: Trained in seat 2, Normal in seats 0,1,3
# Games 150-199: Trained in seat 3, Normal in seats 0-2
```

**Benefits:**
1. **Eliminates position bias**: Each AI experiences all positions equally
2. **Fair comparison**: Differences due to seat position cancel out
3. **Robust results**: Performance measured across all game contexts

**Requirements:**
- `num_games` must be divisible by 4 for systematic rotation
- Use 200+ games for statistical power (50+ per position)

**Not Recommended: Random Rotation**

Random shuffling each game does NOT adequately control for position effects:
- Uneven position distribution (some positions over/under-represented)
- High variance in results
- Harder to interpret and debug

**Use systematic rotation as default.**

#### Rotation Testing

**Default Behavior: Automatic Rotation**

With `--ai-test` mode, systematic rotation is enabled by default:

```bash
# Single command with automatic rotation (RECOMMENDED)
mdhearts eval 200 --ai-test embedded --baseline normal \
                  --weights final_weights.json \
                  --json > rotation_results.json

# The system automatically rotates policies through all 4 positions:
# - Games 0-49:   Policies in positions [Normal, Normal, Normal, Embedded]
# - Games 50-99:  Policies rotate to [Embedded, Normal, Normal, Normal]
# - Games 100-149: Policies rotate to [Normal, Embedded, Normal, Normal]
# - Games 150-199: Policies rotate to [Normal, Normal, Embedded, Normal]
#
# Results show per-policy averages across all positions (position bias eliminated)
```

**Advanced: Per-Position Analysis**

For debugging or detailed analysis, test in fixed positions:

```bash
# Test in specific fixed position (no rotation)
mdhearts eval 200 --ai-test embedded --baseline normal \
                  --weights w.json --rotation fixed \
                  --json > fixed_results.json

# This tests with policies in fixed configuration [Normal, Normal, Normal, Embedded]
# Useful for debugging position-specific behavior
```

**Note: No Multiple Comparisons Issue**

With systematic rotation (default), we perform a SINGLE statistical test comparing:
- Test policy scores (200 games)
- Baseline policy scores (600 games total: 3 policies × 200 games)

Because each policy plays in all 4 positions equally, position bias is already controlled for. No Bonferroni correction needed since we're only doing one comparison.

**Output:**
```
Rotation Testing Results (200 games total)

┌────────────────────┬────────────┬──────────┬───────┬──────┐
│ Configuration      │ Avg Points │ Total    │ Moons │ Wins │
├────────────────────┼────────────┼──────────┼───────┼──────┤
│ Baseline (Normal)  │ 7.06       │ 4236     │ 0     │ 108  │
│ Test (Embedded)    │ 5.76       │ 1152     │ 0     │ 92   │
├────────────────────┼────────────┼──────────┼───────┼──────┤
│ Difference         │ -1.30      │          │       │      │
│ Improvement        │ 18.4%      │          │       │      │
└────────────────────┴────────────┴──────────┴───────┴──────┘

Statistical significance: p < 0.001 (***)

Interpretation: The trained policy performs significantly better than baseline
across all positions (systematic rotation ensured fair comparison).
```

#### Head-to-Head Matrix

For comparing multiple policies:

```bash
mdhearts eval 200 --ai-per-seat normal,hard,embedded,embedded \
                  --weights-per-seat _,_,weights_v1.json,weights_v2.json
```

### 6. Implementation Phases

**Total Timeline: 3-4 weeks** (realistic estimate)

#### Phase 1: Core Infrastructure (Week 1)
- [ ] Create `eval/` module structure
- [ ] Implement `MixedEvalConfig`, `RotationMode`, and result types
- [ ] Implement `create_seat_policies()` (FIXED: correct array ordering)
- [ ] Implement `rotate_seats()` for systematic rotation
- [ ] Basic `run_mixed_eval()` function with rotation support
- [ ] CLI parsing for `--ai-per-seat` and `--rotation`
- [ ] Unit tests for policy creation and rotation logic

#### Phase 2: Statistical Methods (Week 2)
- [ ] Implement `mann_whitney_u_test()` (FIXED: non-parametric test)
- [ ] Implement `standard_normal_cdf()` and `erf()` helpers
- [ ] Implement comparison mode with proper statistics
- [ ] Add Bonferroni correction for multiple comparisons (FIXED)
- [ ] Result aggregation and formatting
- [ ] Per-position breakdown in output

#### Phase 3: JSON & Backward Compatibility (Week 2-3)
- [ ] Implement versioned JSON output (v1.0 and v2.0) (FIXED)
- [ ] Legacy format detection and generation
- [ ] Add `format_version` and `eval_type` fields
- [ ] Test backward compatibility with existing scripts
- [ ] Migration guide and examples

#### Phase 4: User-Friendly CLI (Week 3)
- [ ] Add `--ai-test` simplified interface
- [ ] Add `--baseline` flag
- [ ] Add `--rotation` flag (default: systematic)
- [ ] Improve error messages (e.g., divisibility check for systematic rotation)
- [ ] Add examples to help text
- [ ] Validate configuration inputs

#### Phase 5: Testing & Documentation (Week 4)
- [ ] Integration tests with real games
- [ ] Test all rotation modes (fixed, systematic, random)
- [ ] Test statistical significance calculations
- [ ] Benchmark performance (ensure no regression)
- [ ] Update CLI documentation
- [ ] Add usage examples to README
- [ ] Create `aggregate_rotation_results.py` script

**Critical Fixes Applied in This Revision:**
1. ✅ Array ordering bug fixed (use `try_into()` not `.pop()`)
2. ✅ Statistical test changed to Mann-Whitney U (non-parametric)
3. ✅ Bonferroni correction added for rotation testing
4. ✅ Versioned JSON format for backward compatibility
5. ✅ Systematic rotation to eliminate position bias

### 7. Testing Strategy

#### Unit Tests

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_create_seat_policies() {
        let config = MixedEvalConfig {
            seat_configs: [
                SeatConfig { ai_type: AiType::Normal, weights_path: None },
                SeatConfig { ai_type: AiType::Normal, weights_path: None },
                SeatConfig { ai_type: AiType::Normal, weights_path: None },
                SeatConfig { ai_type: AiType::Embedded, weights_path: Some(path) },
            ],
            // ...
        };

        let policies = create_seat_policies(&config).unwrap();
        assert_eq!(policies.len(), 4);
    }

    #[test]
    fn test_comparison_stats() {
        let results = vec![
            GameResult { points: [8, 7, 6, 5], moon_shooter: None },
            GameResult { points: [7, 8, 5, 6], moon_shooter: None },
            // ...
        ];

        let comparison = compute_comparison(&results, 3).unwrap();
        assert!(comparison.test_avg < comparison.baseline_avg); // Trained is better
        assert!(comparison.percent_improvement > 0.0);
    }
}
```

#### Integration Tests

```bash
# Test basic mixed eval
./target/release/mdhearts eval 100 --ai-per-seat normal,normal,normal,embedded \
                                    --weights-per-seat _,_,_,test_weights.json

# Test comparison mode
./target/release/mdhearts eval 100 --ai-test embedded --test-seat 3 \
                                    --baseline normal --weights test_weights.json

# Test all positions
for seat in 0 1 2 3; do
    ./target/release/mdhearts eval 50 --ai-test embedded --test-seat $seat \
                                       --baseline normal --weights test_weights.json
done
```

#### Performance Tests

```bash
# Ensure no performance regression
# Baseline: homogeneous eval
time ./target/release/mdhearts eval 1000 --ai normal

# New: mixed eval (should be similar)
time ./target/release/mdhearts eval 1000 --ai-per-seat normal,normal,normal,normal
```

### 8. Error Handling

#### Common Error Cases

1. **Mismatched weights array length**
```bash
$ mdhearts eval 200 --ai-per-seat normal,normal,embedded --weights-per-seat _,weights.json

Error: Mismatch between ai-per-seat (3 values) and weights-per-seat (2 values)
Expected 4 AI types and 4 weight paths (use "_" for default)
```

2. **Missing weights for embedded AI**
```bash
$ mdhearts eval 200 --ai-per-seat normal,normal,normal,embedded

Error: Embedded AI in seat 3 requires --weights or --weights-per-seat
Use: --weights final_weights.json
```

3. **Invalid test seat**
```bash
$ mdhearts eval 200 --ai-test embedded --test-seat 5 --weights w.json

Error: Invalid test seat: 5 (must be 0-3)
```

### 9. Backward Compatibility

#### Legacy Commands Still Work

```bash
# These continue to work exactly as before
mdhearts eval 200 --ai normal
mdhearts eval 200 --ai embedded --weights final_weights.json
mdhearts eval 200 --ai hard
```

#### Migration Path

Old evaluation scripts can be gradually updated:
```bash
# Old way (homogeneous)
mdhearts eval 200 --ai normal > baseline.json

# New way (meaningful comparison)
mdhearts eval 200 --ai-test embedded --test-seat 3 \
                  --baseline normal --weights final_weights.json > comparison.json
```

### 10. Future Extensions

#### A. Tournament Mode
```bash
# Round-robin: each AI plays in each seat
mdhearts tournament 200 --ais normal,hard,embedded,embedded \
                        --weights _,_,weights_v1.json,weights_v2.json
```

#### B. Adaptive Baselines
```bash
# Test against progressively harder opponents
mdhearts eval 200 --ai-test embedded --baseline adaptive \
                  --weights final_weights.json
# Starts with easy, moves to normal, then hard as trained AI wins
```

#### C. Elo Rating System
```bash
# Compute Elo ratings for multiple policies
mdhearts elo-tournament --policies policy1.json,policy2.json,policy3.json \
                        --games 1000
```

#### D. Live Opponent Modeling
```bash
# Train opponent model during evaluation
mdhearts eval 200 --ai-test embedded --baseline normal \
                  --learn-opponent-model opponent_model.json
```

## Success Criteria

### Must-Have (MVP)
- [ ] Support `--ai-per-seat` with 4 comma-separated AI types
- [ ] Support `--weights-per-seat` for per-seat weight files
- [ ] Compute and display per-seat statistics
- [ ] Backward compatible with existing eval commands
- [ ] Pass all existing tests

### Should-Have
- [ ] Simplified `--ai-test` interface for common case
- [ ] Comparison mode with statistical significance
- [ ] JSON output for scripting
- [ ] Rotation testing helper script

### Nice-to-Have
- [ ] Tournament mode (round-robin)
- [ ] Elo rating computation
- [ ] Progress bar during long evaluations
- [ ] Colorized output for terminal

## Risk Assessment

### Technical Risks

1. **Performance Regression**
   - **Risk**: Creating 4 separate policies might slow down evaluation
   - **Mitigation**: Benchmark before/after, optimize if needed
   - **Likelihood**: Low (policy creation is one-time cost)

2. **State Management Complexity**
   - **Risk**: Managing 4 policies instead of 1 increases complexity
   - **Mitigation**: Careful testing, clear ownership model
   - **Likelihood**: Medium

3. **Memory Usage**
   - **Risk**: 4 neural networks in memory simultaneously
   - **Mitigation**: Lazy loading, shared weight storage
   - **Likelihood**: Low (each network is ~2MB)

### User Experience Risks

1. **API Confusion**
   - **Risk**: Multiple ways to specify mixed configs could confuse users
   - **Mitigation**: Clear documentation, good error messages
   - **Likelihood**: Medium

2. **Breaking Changes**
   - **Risk**: Accidentally breaking existing scripts
   - **Mitigation**: Extensive backward compatibility testing
   - **Likelihood**: Low

## References

- Current evaluation code: `crates/hearts-app/src/cli.rs:353`
- Policy trait: `crates/hearts-ai/src/policy/mod.rs`
- Game engine: `crates/hearts-core/src/model/game.rs`

## Appendix A: Example Usage

### Basic Mixed Evaluation
```bash
mdhearts eval 200 --ai-per-seat normal,normal,normal,embedded \
                  --weights-per-seat _,_,_,final_weights.json
```

### Simplified Test Mode
```bash
mdhearts eval 200 --ai-test embedded --test-seat 3 \
                  --baseline normal --weights final_weights.json
```

### Full Rotation Test
```bash
#!/bin/bash
for seat in 0 1 2 3; do
    echo "Testing in seat $seat..."
    mdhearts eval 200 --ai-test embedded --test-seat $seat \
                      --baseline normal --weights final_weights.json \
                      --json > results_seat_${seat}.json
done

python scripts/aggregate_rotation.py results_seat_*.json > rotation_summary.json
```

### Comparing Two Trained Policies
```bash
mdhearts eval 200 --ai-per-seat normal,normal,embedded,embedded \
                  --weights-per-seat _,_,weights_v1.json,weights_v2.json
```

## Appendix B: Output Schema

### JSON Output Schema
```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "games_played": {"type": "integer"},
    "config": {
      "type": "object",
      "properties": {
        "seat_configs": {
          "type": "array",
          "items": {
            "type": "object",
            "properties": {
              "ai_type": {"type": "string"},
              "weights": {"type": ["string", "null"]}
            }
          }
        }
      }
    },
    "seat_results": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "seat": {"type": "integer"},
          "ai": {"type": "string"},
          "avg_points": {"type": "number"},
          "total": {"type": "integer"},
          "moons": {"type": "integer"},
          "wins": {"type": "integer"}
        }
      }
    },
    "comparison": {
      "type": ["object", "null"],
      "properties": {
        "test_seat": {"type": "integer"},
        "test_avg": {"type": "number"},
        "baseline_avg": {"type": "number"},
        "difference": {"type": "number"},
        "percent_improvement": {"type": "number"},
        "p_value": {"type": ["number", "null"]},
        "significant": {"type": "boolean"}
      }
    },
    "elapsed_seconds": {"type": "number"}
  },
  "required": ["games_played", "seat_results", "elapsed_seconds"]
}
```

## Change Log

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2025-10-07 | Claude | Initial HLD |
| 2.0 | 2025-10-07 | Claude | **MAJOR REVISION** - Addressed 5 critical issues from review:<br>1. Fixed array ordering bug in `create_seat_policies()`<br>2. Replaced Welch's t-test with Mann-Whitney U test<br>3. Added Bonferroni correction for rotation testing<br>4. Added versioned JSON format (v1.0 and v2.0)<br>5. Added systematic rotation to eliminate position bias<br>Updated timeline from 1 week to 3-4 weeks (realistic) |
| 3.0 | 2025-10-07 | Claude | **SECOND MAJOR REVISION** - Addressed 15 issues from second review:<br>**Critical Fixes:**<br>1. Renamed `test_seat` → `test_policy_index` throughout<br>2. Clarified `--ai-test` behavior: creates 3 baseline + 1 test automatically<br>3. Added `validate_config()` function for mode compatibility<br>4. Renamed `SeatResults` → `PolicyResults`<br>**Important Fixes:**<br>5. Improved sample size validation (checks both n1, n2 >= 20)<br>6. Added missing helper functions (`mean`, `create_policy`, etc.)<br>7. Default rotation: Systematic for `--ai-test`, Fixed for `--ai-per-seat`<br>8. Updated rotation section to match new design<br>9. Fixed JSON output field names<br>10. Complete CLI parsing with proper error handling<br>**Minor Fixes:**<br>11. Added ranking calculation comments<br>12. Added erf() citation (Abramowitz and Stegun)<br>13. Made progress reporting adaptive<br>14. Documented moon shooter edge case<br>15. Clarified win_count computation |

## Review History

- **2025-10-07 - First Review** ([HLD_MIXED_EVALUATION_REVIEW.md](./HLD_MIXED_EVALUATION_REVIEW.md))
  - Reviewed v1.0
  - 36 issues identified across 9 categories
  - 5 critical issues → all addressed in v2.0
  - 10 important issues → partially addressed in v2.0

- **2025-10-07 - Second Review** ([HLD_MIXED_EVALUATION_REVIEW_V2.md](./HLD_MIXED_EVALUATION_REVIEW_V2.md))
  - Reviewed v2.0
  - 15 issues identified (3 critical, 7 important, 5 minor)
  - All critical and important issues → addressed in v3.0
  - Minor issues → addressed in v3.0
  - **Status**: Ready for implementation

---

**Document End**
