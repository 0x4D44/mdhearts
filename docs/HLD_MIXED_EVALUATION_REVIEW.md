# HLD Review: Mixed AI Evaluation System

## Review Metadata
- **Reviewer**: Claude Code
- **Date**: October 7, 2025
- **Original HLD Version**: 1.0
- **Review Type**: Technical & Design

## Executive Summary

The HLD proposes a valuable feature that addresses a real problem in the current evaluation system. However, I've identified **36 issues** ranging from critical bugs to design improvements. The most serious issues relate to:

1. **Statistical methodology** (using inappropriate t-test)
2. **Implementation bugs** (array ordering, error handling)
3. **API design** (confusing syntax, rigid structure)
4. **Backward compatibility** (breaking JSON format changes)

## Critical Issues (Must Fix)

### 1. Array Ordering Bug in Policy Creation ⚠️ CRITICAL

**Location**: Section 4.3, `create_seat_policies()`

**Issue**:
```rust
fn create_seat_policies(...) -> Result<[Box<dyn Policy>; 4], EvalError> {
    let mut policies: Vec<Box<dyn Policy>> = Vec::new();
    for seat_config in &config.seat_configs {
        policies.push(create_policy(...)?);  // Push in order: 0,1,2,3
    }

    // Convert Vec to array
    Ok([
        policies.pop().unwrap(),  // Gets 3
        policies.pop().unwrap(),  // Gets 2
        policies.pop().unwrap(),  // Gets 1
        policies.pop().unwrap(),  // Gets 0
    ])  // Result: [3,2,1,0] - REVERSED!
}
```

**Impact**: Policies assigned to wrong seats. Seat 0 gets seat 3's config, etc.

**Fix**:
```rust
let mut iter = policies.into_iter();
Ok([
    iter.next().unwrap(),
    iter.next().unwrap(),
    iter.next().unwrap(),
    iter.next().unwrap(),
])

// Or better:
Ok(policies.try_into().unwrap_or_else(|v: Vec<_>| {
    panic!("Expected exactly 4 policies, got {}", v.len())
}))
```

### 2. Invalid Statistical Test ⚠️ CRITICAL

**Location**: Section 4.2, `compute_comparison()`

**Issue**: Proposes using Welch's t-test to compare scores.

**Why This Is Wrong**:

Hearts scores are **NOT normally distributed**:
- **Bounded**: Scores are in [0, 26] per game (finite range)
- **Discrete**: Scores are integers, not continuous
- **Skewed**: Distribution has long tail (shooting the moon = 0 points)
- **Multimodal**: Different strategies produce different score distributions

T-tests assume normal distribution. Violating this assumption produces invalid p-values.

**Evidence of Non-Normality**:
```
Example score distribution from 1000 games:
0 pts:  |||||||||||| (120 games - moon shots)
1-3 pts: || (20 games)
4-6 pts: ||||||||||| (110 games)
7-9 pts: ||||||||||||||||||| (190 games)
10-12 pts: ||||||||||||| (130 games)
...
```

This is multimodal, not bell-curved.

**Correct Approach**:

Option A: **Mann-Whitney U test** (non-parametric)
```rust
fn compute_comparison(...) -> ComparisonResults {
    // Use rank-based test (no normality assumption)
    let u_statistic = mann_whitney_u_test(&test_scores, &baseline_scores);
    let p_value = u_statistic_to_pvalue(u_statistic, n1, n2);
    // ...
}
```

Option B: **Bootstrap confidence intervals**
```rust
fn bootstrap_comparison(test_scores: &[f64], baseline_scores: &[f64], n_bootstrap: usize) -> (f64, f64) {
    let mut differences = vec![];
    for _ in 0..n_bootstrap {
        let test_sample = resample(test_scores);
        let baseline_sample = resample(baseline_scores);
        differences.push(mean(&test_sample) - mean(&baseline_sample));
    }
    differences.sort_by(|a, b| a.partial_cmp(b).unwrap());

    // 95% confidence interval
    let lower = differences[(0.025 * n_bootstrap as f64) as usize];
    let upper = differences[(0.975 * n_bootstrap as f64) as usize];
    (lower, upper)
}
```

**Recommendation**: Use Mann-Whitney U test for simplicity, add bootstrap CI as optional detailed analysis.

### 3. Multiple Comparisons Problem

**Location**: Section 5, Rotation Testing

**Issue**: Testing in all 4 positions = 4 separate hypothesis tests. Without correction, inflates Type I error rate.

**Example**:
- Each test has α = 0.05 (5% false positive rate)
- 4 independent tests: P(at least one false positive) = 1 - (0.95)^4 = 18.5%

**Fix**: Apply **Bonferroni correction**
```rust
// Adjust significance level
let alpha_corrected = 0.05 / num_tests as f64;  // 0.0125 for 4 tests

if p_value < alpha_corrected {
    println!("Significant after Bonferroni correction");
}
```

**Or** use more sophisticated methods: Holm-Bonferroni, FDR control, etc.

### 4. Breaking JSON Format Change ⚠️ CRITICAL

**Location**: Section 4.4, Output Format

**Issue**: New JSON format is completely incompatible with old format.

**Old format**:
```json
{
  "ai_type": "Normal",
  "avg_points": [7.32, 7.13, 6.07, 5.49],
  "games": 200
}
```

**New format**:
```json
{
  "games_played": 200,
  "seat_results": [
    {"seat": 0, "ai": "Normal", "avg_points": 7.32},
    ...
  ]
}
```

**Impact**: All existing scripts parsing evaluation output will break.

**Fix**: Version the output or support both formats

**Option 1: Versioned output**
```bash
mdhearts eval 200 --ai normal --output-version 1  # Old format
mdhearts eval 200 --ai normal --output-version 2  # New format (default)
```

**Option 2: Compatibility mode**
```rust
if is_homogeneous_config(&config) {
    // Use old format for backward compatibility
    return serialize_legacy_format(&results);
} else {
    // Use new format for mixed configs
    return serialize_mixed_format(&results);
}
```

**Recommendation**: Use Option 2 (automatic compatibility) with clear documentation.

### 5. Seat Position Bias Not Addressed

**Location**: Section 5, Evaluation Methodology

**Issue**: In Hearts, seat position affects gameplay:
- **Starting position**: Seat with 2♣ always leads first trick
- **Card passing**: Direction varies by hand (left, right, across, none)
- **Information advantage**: Later seats see more cards before playing

**Current design**: Uses random seating → seat effects don't cancel out over 200 games.

**Fix**: Rotate starting position systematically

```rust
fn run_mixed_eval(config: MixedEvalConfig) -> Result<MixedEvalResults, EvalError> {
    for game_idx in 0..config.num_games {
        // Rotate starting seat every 4 games to ensure balance
        let starting_seat = game_idx % 4;
        let game_result = run_single_game(&mut policies, starting_seat)?;
        // ...
    }
}
```

**Better**: Ensure games are divisible by 4 and rotate systematically.

## Important Issues (Should Fix)

### 6. Confusing "_" Syntax for Weights

**Location**: Section 3.1, CLI Interface

**Issue**:
```bash
--weights-per-seat _,_,_,final_weights.json
```

Using "_" to mean "no weights" is non-standard and error-prone.

**Problems**:
- Not idiomatic in CLI design
- Requires escaping in some shells
- Easy to miscount underscores

**Better alternatives**:

**Option A: Named parameters**
```bash
--weights 3:final_weights.json
# Only specify weights for seats that need them
```

**Option B: Keyword syntax**
```bash
--weights-per-seat none,none,none,final_weights.json
```

**Option C: JSON config**
```bash
--config-file eval_config.json
# { "seats": [{"ai": "normal"}, ..., {"ai": "embedded", "weights": "..."}] }
```

**Recommendation**: Option A (most concise for common case).

### 7. Rigid Comparison Structure

**Location**: Section 3.2, Data Structures

**Issue**: `ComparisonResults` assumes exactly 1 test seat vs 3 baseline seats.

```rust
pub struct ComparisonResults {
    pub test_seat: usize,
    pub baseline_avg: f64,  // Average of seats 0,1,2 (excluding test_seat)
    // ...
}
```

**What if we want**:
- 2 trained policies vs 2 baselines?
- 1 trained vs 1 hard AI vs 2 normal AIs?
- Compare seat 0 vs seat 1 (both trained, different versions)?

**Current design can't handle these.**

**Fix**: Generalize to group-based comparison

```rust
pub struct ComparisonResults {
    pub groups: Vec<ComparisonGroup>,
    pub pairwise_comparisons: Vec<PairwiseComparison>,
}

pub struct ComparisonGroup {
    pub label: String,
    pub seats: Vec<usize>,
    pub avg_points: f64,
    pub stats: Statistics,
}

pub struct PairwiseComparison {
    pub group1: String,
    pub group2: String,
    pub difference: f64,
    pub p_value: f64,
    pub effect_size: f64,  // Cohen's d
}
```

This allows flexible comparisons:
```bash
# Compare trained (seat 3) vs all baseline (seats 0-2)
--compare-groups "trained:3" "baseline:0,1,2"

# Compare two trained policies
--compare-groups "v1:0" "v2:3" "baseline:1,2"
```

### 8. Missing Per-Game Results

**Location**: Section 3.2, MixedEvalResults

**Issue**: Only aggregate statistics stored, no per-game data.

**Why this matters**:
- Can't analyze variance or consistency
- Can't debug specific games where trained AI failed
- Can't identify outliers or anomalies

**Example**: Trained AI might:
- Win slightly in 150 games
- Lose catastrophically in 50 games (shooting moon attempts gone wrong)
- Average looks OK, but policy is unstable

**Fix**:
```rust
pub struct MixedEvalResults {
    // ... existing aggregate fields ...

    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_results: Option<Vec<GameResult>>,  // Detailed per-game data
}

pub struct GameResult {
    pub game_index: usize,
    pub seed: u64,  // For reproducibility
    pub points: [u8; 4],
    pub moon_shooter: Option<usize>,
    pub winner: usize,  // Seat with lowest score
}
```

Enable with flag:
```bash
mdhearts eval 200 --ai-test ... --detailed-output
```

### 9. Error Checking Timing

**Location**: Section 3.1, CLI Parsing

**Issue**: When is invalid config detected?

**Example**:
```bash
mdhearts eval 200 --ai-per-seat normal,normal,normal,embedded
# No weights provided for embedded AI
```

**Should this fail**:
- During CLI parsing? (fast fail)
- During policy creation? (after starting evaluation)
- During first game? (after computing advantages, etc.)

**Current design doesn't specify.**

**Fix**: Validate at parse time
```rust
fn parse_mixed_config(args: &Args) -> Result<MixedEvalConfig, ParseError> {
    let config = // ... parse ...

    // Validate before returning
    validate_mixed_config(&config)?;

    Ok(config)
}

fn validate_mixed_config(config: &MixedEvalConfig) -> Result<(), ValidationError> {
    for (i, seat) in config.seat_configs.iter().enumerate() {
        if seat.ai_type == AiType::Embedded && seat.weights_path.is_none() {
            return Err(ValidationError::MissingWeights { seat: i });
        }

        if let Some(path) = &seat.weights_path {
            if !path.exists() {
                return Err(ValidationError::WeightsNotFound {
                    seat: i,
                    path: path.clone()
                });
            }
        }
    }

    Ok(())
}
```

### 10. Duplicate Weights Handling

**Location**: Section 4.3, Policy Management

**Issue**: What if same weights file used for multiple seats?

```bash
# All 4 seats use same trained policy
mdhearts eval 200 --ai-per-seat embedded,embedded,embedded,embedded \
                  --weights 0:w.json,1:w.json,2:w.json,3:w.json  # Same file!
```

**Options**:
- **Load 4 times**: Wastes memory (~8MB total)
- **Load once, share**: Need Rc<Policy> or Arc<Policy>
- **Error**: Too restrictive

**Recommendation**: Share when possible

```rust
use std::sync::Arc;
use std::collections::HashMap;

fn create_seat_policies(config: &MixedEvalConfig) -> Result<[Arc<dyn Policy>; 4], EvalError> {
    let mut policy_cache: HashMap<PathBuf, Arc<dyn Policy>> = HashMap::new();
    let mut policies = vec![];

    for seat_config in &config.seat_configs {
        let policy = match &seat_config.weights_path {
            Some(path) if seat_config.ai_type == AiType::Embedded => {
                // Check cache first
                if let Some(cached) = policy_cache.get(path) {
                    Arc::clone(cached)
                } else {
                    let policy = Arc::new(EmbeddedPolicy::from_file(path)?);
                    policy_cache.insert(path.clone(), Arc::clone(&policy));
                    policy
                }
            }
            _ => Arc::new(create_policy(seat_config)?)
        };
        policies.push(policy);
    }

    // Convert to array...
}
```

**Trade-off**: Policies need `&mut self` for stateful operations. If sharing, either:
- Make Policy immutable (preferred)
- Use interior mutability (RefCell/Mutex)
- Clone state per invocation

## Design Issues (Should Improve)

### 11. Simplified API Is Actually More Complex

**Location**: Section 3.1, Option 3 (Hybrid Approach)

**Issue**: The "simple" mode requires 4 separate flags:

```bash
mdhearts eval 200 --ai-test embedded \
                  --test-seat 3 \
                  --baseline normal \
                  --weights final_weights.json
```

Compare to a hypothetical clearer syntax:
```bash
mdhearts eval 200 --versus normal:3,embedded:1 --weights final_weights.json
# "3 normal AIs, 1 embedded AI"
```

Or even simpler:
```bash
mdhearts eval-versus 200 embedded normal --weights final_weights.json
# Test embedded vs normal (3x baseline) in all positions
```

**Recommendation**: Reconsider the "simple" mode to actually be simple.

### 12. OutputMode Conflates Evaluation and Presentation

**Location**: Section 3.2, MixedEvalConfig

**Issue**: `OutputMode` is in the evaluation config, but it's really about presentation.

```rust
pub struct MixedEvalConfig {
    pub num_games: usize,
    pub seat_configs: [SeatConfig; 4],
    pub output_mode: OutputMode,  // ← Presentation concern in eval config
}
```

**Problem**: Mixing concerns. What if we want to:
- Evaluate once
- Display results multiple ways (standard, comparison, detailed)

Currently can't do this without re-evaluating.

**Fix**: Separate evaluation from presentation

```rust
// Evaluation config: only what affects evaluation
pub struct MixedEvalConfig {
    pub num_games: usize,
    pub seat_configs: [SeatConfig; 4],
}

// Presentation config: only display options
pub struct OutputConfig {
    pub format: OutputFormat,  // JSON, Table, CSV
    pub mode: OutputMode,      // Standard, Comparison, Detailed
    pub comparison_groups: Option<Vec<ComparisonGroup>>,
}

// Separate steps
let results = run_mixed_eval(eval_config)?;
present_results(&results, output_config)?;
```

### 13. No Sample Size Guidance

**Location**: Section 5, Evaluation Methodology

**Issue**: Design suggests 200 games but provides no justification.

**Question**: How many games needed to detect:
- 5% difference?
- 10% difference?
- 20% difference?

With 95% confidence and 80% power?

**Fix**: Add power analysis to documentation

**Example calculation**:
```python
# Assuming:
# - Mean score ≈ 6.5 points
# - Std dev ≈ 3.0 points
# - Want to detect 10% difference (0.65 points)
# - α = 0.05, power = 0.80

from scipy import stats
effect_size = 0.65 / 3.0  # Cohen's d ≈ 0.22
n_per_group = stats.tt_ind_solve_power(effect_size=effect_size,
                                         alpha=0.05,
                                         power=0.80)
# Result: n ≈ 323 per group

# For comparison mode: 1 test seat vs 3 baseline seats
# Test group: 323 games
# Baseline group: 323 * 3 = 969 seat-games = 323 games
# Total: 323 games (test seat gets 323 samples, baseline gets 969)
```

**Recommendation**:
- Small effect (5-10%): 400-800 games
- Medium effect (10-20%): 200-400 games  ← Current default
- Large effect (>20%): 100-200 games

Document this in HLD.

### 14. Missing Variance Metrics

**Location**: Section 4.4, Output Format

**Issue**: Only reports mean scores, not variance.

**Why variance matters**:
- Consistent policy: Low variance (e.g., always gets 6-7 points)
- Risky policy: High variance (e.g., 0 or 15 points)

Both might have same mean but very different risk profiles.

**Fix**: Add variance metrics

```rust
pub struct SeatResults {
    pub avg_points: f64,
    pub std_dev: f64,      // ← Add
    pub median: f64,       // ← Add (more robust than mean)
    pub percentile_25: f64,  // ← Add
    pub percentile_75: f64,  // ← Add
    pub min: u8,
    pub max: u8,
    // ...
}
```

**Display**:
```
Results:
Seat 0 (Normal):    7.42 ± 3.21 (median: 7.0, IQR: 5-10)
Seat 3 (Embedded):  5.76 ± 4.82 (median: 5.0, IQR: 3-7)

Interpretation: Embedded has lower mean but higher variance (riskier)
```

## Implementation Issues

### 15. Unrealistic Timeline

**Location**: Section 6, Implementation Phases

**Issue**: "Week 1" for all 4 phases is too aggressive.

**Actual complexity**:
- Phase 1 (Core): 3-5 days (new module, data structures, basic logic)
- Phase 2 (Stats): 2-3 days (statistical tests, proper testing)
- Phase 3 (CLI): 2-3 days (complex parsing, validation, error messages)
- Phase 4 (Testing): 3-5 days (comprehensive test suite, docs)

**Total**: 10-16 days ≈ **2-3 weeks** for one developer

**Plus**: Code review, bug fixing, iteration → **3-4 weeks realistic**

**Recommendation**: Update timeline to 3-4 weeks.

### 16. No Rollback Strategy

**Location**: Section 6, missing

**Issue**: What if critical bug discovered after merging?

**Need**:
```rust
// Feature flag
const ENABLE_MIXED_EVAL: bool = cfg!(feature = "mixed-eval");

match args {
    "eval" if ENABLE_MIXED_EVAL && has_mixed_flags(&args) => {
        run_mixed_eval(...)?;
    }
    "eval" => {
        run_eval_legacy(...)?;  // Old code path
    }
}
```

**Deployment strategy**:
1. Week 1-3: Development
2. Week 4: Merge behind feature flag (disabled by default)
3. Week 5: Enable for internal testing
4. Week 6: Enable for all users
5. Week 7+: Remove feature flag, delete old code

### 17. Different Embedded Model Architectures

**Location**: Section 8, Edge Cases (missing)

**Issue**: What if two embedded policies have different architectures?

**Example**:
```bash
# weights_v1.json: 270 → 256 → 128 → 52
# weights_v2.json: 270 → 512 → 512 → 256 → 52
mdhearts eval 200 --ai-per-seat embedded,normal,normal,embedded \
                  --weights 0:weights_v1.json,3:weights_v2.json
```

Different architectures → can't share code paths.

**Current policy loading**:
```rust
pub fn from_file(path: &Path) -> Result<Self, PolicyError> {
    let weights: WeightsJson = serde_json::from_reader(file)?;

    // Assumes architecture from weights file
    let model = ActorCritic::from_weights(&weights)?;
    // ...
}
```

This should work fine as long as each policy loads its own architecture from its weights file.

**But**: Need to validate schema compatibility with observation space.

**Fix**: Add schema validation
```rust
pub fn from_file(path: &Path) -> Result<Self, PolicyError> {
    let weights: WeightsJson = serde_json::from_reader(file)?;

    // Validate schema matches current observation space
    validate_schema(&weights.schema_version, &weights.schema_hash)?;

    let model = ActorCritic::from_weights(&weights)?;
    Ok(Self { model })
}
```

### 18. Self-Play with Mixed Config

**Location**: Section 8, Edge Cases (missing)

**Issue**: Current `--self-play` flag for RL data collection. What does it mean with mixed configs?

```bash
# This makes sense
mdhearts eval 200 --ai embedded --self-play --collect-rl data.jsonl

# This doesn't make sense
mdhearts eval 200 --ai-per-seat normal,normal,normal,embedded --self-play
```

"Self-play" implies all 4 seats use same policy. With mixed config, this is contradictory.

**Fix**: Disable `--self-play` with mixed configs
```rust
if config.is_mixed() && args.self_play {
    return Err(CliError::InvalidCombination(
        "--self-play cannot be used with mixed AI configurations"
    ));
}
```

### 19. Collect-RL with Mixed Config

**Location**: Section 8, Edge Cases (missing)

**Issue**: `--collect-rl` currently collects from all 4 seats. With mixed configs:

```bash
mdhearts eval 200 --ai-per-seat normal,normal,normal,embedded \
                  --weights 3:weights.json \
                  --collect-rl data.jsonl
```

Should we:
- Collect from all seats? (includes normal AI decisions)
- Collect only from embedded seat(s)? (makes sense for training)
- Error? (force user to be explicit)

**Recommendation**: Add `--collect-from-seats` flag
```bash
# Only collect from seat 3 (the trained AI)
--collect-rl data.jsonl --collect-from-seats 3

# Collect from all seats
--collect-rl data.jsonl --collect-from-seats 0,1,2,3
```

## Usability Issues

### 20. No Dry-Run Mode

**Location**: Section 9, Missing

**Issue**: Complex configs are error-prone. Users want to preview before running 200 games.

**Fix**:
```bash
mdhearts eval 200 --ai-per-seat normal,hard,embedded,embedded \
                  --weights 2:w1.json,3:w2.json \
                  --dry-run
```

**Output**:
```
Dry-run mode: No games will be played

Configuration:
  Seat 0: Normal AI (default weights)
  Seat 1: Hard AI (default weights)
  Seat 2: Embedded AI (weights: w1.json, schema: v1.1.0)
  Seat 3: Embedded AI (weights: w2.json, schema: v1.1.0)

Would run 200 games (~0.5 seconds estimated)

Output mode: Standard
  - Per-seat averages
  - Moon shot counts
  - Win counts

To proceed, remove --dry-run flag.
```

### 21. No Progress Estimation

**Location**: Section 4.4, Output Format

**Issue**: Long evaluations need progress info.

**Current**: Simple progress bar
```
[====================] 200/200 (0.5s)
```

**Better**:
```
Running mixed evaluation...
[=========           ] 100/200 (50%, 0.25s elapsed, ~0.25s remaining)
  Seat 0 (Normal):   7.42 ± 3.21 (current avg over 100 games)
  Seat 1 (Normal):   7.18 ± 2.98
  Seat 2 (Normal):   6.58 ± 3.45
  Seat 3 (Embedded): 5.76 ± 4.12 ⭐ (currently best)

Current lead: Embedded by 0.82 points (p < 0.05)
```

**Implementation**:
```rust
fn print_progress(results: &[GameResult], current: usize, total: usize) {
    let pct = (current as f64 / total as f64 * 100.0) as usize;
    let bar_width = 20;
    let filled = (bar_width * current) / total;
    let bar: String = (0..bar_width)
        .map(|i| if i < filled { '=' } else { ' ' })
        .collect();

    // Compute current averages
    let avgs = compute_current_averages(results);

    print!("\r[{}] {}/{} ({}%)", bar, current, total, pct);
    for (seat, avg) in avgs.iter().enumerate() {
        print!(" | Seat {}: {:.2}", seat, avg);
    }
    stdout().flush().unwrap();
}
```

### 22. Limited Output Formats

**Location**: Section 4.4, Output Format

**Issue**: Only ASCII table and JSON supported.

**Users might want**:
- **CSV** for Excel/spreadsheets
- **Markdown** for documentation
- **HTML** for web dashboards
- **Plots** (histograms, box plots)

**Fix**: Add format flag
```bash
mdhearts eval 200 --ai-test ... --format csv > results.csv
mdhearts eval 200 --ai-test ... --format markdown > report.md
mdhearts eval 200 --ai-test ... --format html > dashboard.html
```

**CSV example**:
```csv
seat,ai_type,weights,avg_points,std_dev,median,total_points,moons,wins
0,Normal,,7.42,3.21,7.0,1484,0,32
1,Normal,,7.18,2.98,7.0,1436,0,38
2,Normal,,6.58,3.45,6.0,1316,0,38
3,Embedded,final_weights.json,5.76,4.82,5.0,1152,0,92
```

## Security Issues

### 23. Path Traversal Vulnerability

**Location**: Section 3.1, CLI Parsing

**Issue**: Weights paths are user-provided, no validation.

**Attack**:
```bash
mdhearts eval 200 --ai embedded --weights ../../../etc/passwd
```

Could attempt to load arbitrary files.

**Impact**: Low (weights loading will fail on non-JSON, but still reads file)

**Fix**: Validate paths
```rust
fn validate_weights_path(path: &Path) -> Result<PathBuf, ValidationError> {
    // Canonicalize to resolve symlinks and ..
    let canonical = path.canonicalize()
        .map_err(|e| ValidationError::InvalidPath { path: path.to_owned(), source: e })?;

    // Ensure it's within project directory (or explicitly allowed location)
    let allowed_roots = vec![
        std::env::current_dir()?,
        Path::new("/opt/mdhearts/weights"),
        // ...
    ];

    if !allowed_roots.iter().any(|root| canonical.starts_with(root)) {
        return Err(ValidationError::PathOutsideAllowedRoots { path: canonical });
    }

    Ok(canonical)
}
```

### 24. Malicious Weights File

**Location**: Section 4.3, Policy Loading

**Issue**: Weights files are deserialized from JSON. Potential issues:
- **Size**: Malicious 1GB weights file → OOM
- **Format**: Malformed JSON → crash
- **Content**: Exploiting serde deserialization bugs

**Fix**: Add safety checks
```rust
pub fn from_file(path: &Path) -> Result<Self, PolicyError> {
    // Check file size
    let metadata = std::fs::metadata(path)?;
    const MAX_WEIGHTS_SIZE: u64 = 50 * 1024 * 1024;  // 50MB
    if metadata.len() > MAX_WEIGHTS_SIZE {
        return Err(PolicyError::WeightsFileTooLarge {
            size: metadata.len(),
            max: MAX_WEIGHTS_SIZE
        });
    }

    // Load with error handling
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let weights: WeightsJson = serde_json::from_reader(reader)
        .map_err(|e| PolicyError::InvalidWeightsFormat {
            path: path.to_owned(),
            source: e
        })?;

    // Validate structure
    validate_weights_structure(&weights)?;

    // ... rest of loading ...
}
```

## Missing Features

### 25. Game Replay Capability

**Location**: Section 10, Future Extensions (missing)

**Issue**: If trained AI performs poorly in a specific game, how do we debug it?

**Need**: Save game seeds and replay
```bash
# During evaluation, save seeds
mdhearts eval 200 --ai-test ... --save-seeds seeds.txt

# Later, replay specific game
mdhearts replay --seed 1234567890 \
                --ai-per-seat normal,normal,normal,embedded \
                --weights 3:final_weights.json \
                --verbose  # Show decision reasoning
```

**Implementation**:
```rust
pub struct GameResult {
    pub game_index: usize,
    pub seed: u64,  // ← Add this
    pub points: [u8; 4],
    // ...
}

fn run_single_game(policies: &mut [Box<dyn Policy>; 4], seed: u64) -> Result<GameResult, EvalError> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut state = GameState::new_with_rng(&mut rng);
    // ... run game ...
    Ok(GameResult { seed, ... })
}
```

### 26. Head-to-Head Win Records

**Location**: Section 4.2, Statistics (missing)

**Issue**: Average scores don't tell full story.

**Example**: In 200 games:
- Trained: avg 5.76
- Normal: avg 7.06

But:
- How many games did trained WIN (lowest score)?
- How many games did trained come in last?
- What's the win rate?

**Fix**: Add head-to-head metrics
```rust
pub struct ComparisonResults {
    // ... existing fields ...
    pub win_counts: HashMap<String, usize>,  // "trained" → 92, "baseline" → 108
    pub win_rate: f64,  // trained win rate: 92/200 = 46%
    pub head_to_head: Vec<Vec<usize>>,  // Matrix of pairwise wins
}
```

**Display**:
```
Head-to-Head Results:
  Trained wins: 92/200 (46%)
  Normal wins: 108/200 (54%)

  Trained vs Normal: 92-108 (46% win rate)
  Trained beat Normal in last 20 games: 12-8 (60% - improving!)
```

### 27. Meta-Analysis Across Runs

**Location**: Section 10, Future Extensions (missing)

**Issue**: Users will run many evaluations (different weights, iterations, configs). Need to track improvement over time.

**Example workflow**:
```bash
# Iteration 50
mdhearts eval 200 --ai-test embedded --weights checkpoint_50.json --save-run run1.json

# Iteration 100
mdhearts eval 200 --ai-test embedded --weights checkpoint_100.json --save-run run2.json

# Iteration 200
mdhearts eval 200 --ai-test embedded --weights final_weights.json --save-run run3.json

# Compare all runs
mdhearts compare-runs run1.json run2.json run3.json
```

**Output**:
```
Training Progress Analysis:
┌────────────┬──────────┬───────────┬────────────────┐
│ Checkpoint │ Avg Pts  │ vs Normal │ Trend          │
├────────────┼──────────┼───────────┼────────────────┤
│ Iter 50    │ 6.84     │ +0.34     │ Slightly worse │
│ Iter 100   │ 6.42     │ -0.08     │ Improving ↗    │
│ Iter 200   │ 5.76     │ -1.30     │ Best ⭐        │
└────────────┴──────────┴───────────┴────────────────┘

Conclusion: Training is effective. Iter 200 shows 18% improvement.
Recommendation: Use checkpoint 200 for production.
```

### 28. Adaptive/Dynamic Baselines

**Location**: Section 10, Future Extensions

**Issue**: Static baselines don't test full range of opponent skill.

**Idea**: Start with easy, increase difficulty as trained AI improves
```bash
mdhearts eval 200 --ai-test embedded --baseline adaptive
```

**Logic**:
```rust
fn adaptive_baseline_eval(...) {
    let mut baseline = AiType::Easy;

    for game_chunk in (0..num_games).chunks(50) {
        let results = run_games(game_chunk, baseline, trained_ai);
        let win_rate = compute_win_rate(&results);

        if win_rate > 0.6 {
            // Trained AI is dominating, increase difficulty
            baseline = match baseline {
                AiType::Easy => AiType::Normal,
                AiType::Normal => AiType::Hard,
                AiType::Hard => AiType::Hard,  // Max difficulty
            };
            println!("Increasing difficulty to {:?}", baseline);
        }
    }
}
```

**Benefit**: Single command tests across difficulty spectrum.

## Correctness Issues

### 29. Aggregation Weighting

**Location**: Section 4.2, `compute_comparison()`

**Issue**: Baseline average computed as simple mean across seats:
```rust
let baseline_scores: Vec<f64> = results.iter()
    .flat_map(|r| {
        r.points.iter().enumerate()
            .filter(|(i, _)| *i != test_seat)
            .map(|(_, &p)| p as f64)
    })
    .collect();

let baseline_avg = mean(&baseline_scores);
```

This gives equal weight to all baseline seats. But what if seats have different opportunities?

**Example**: In 200 games, if due to random chance:
- Seat 0 gets 2♣ 60 times (leads first trick more)
- Seat 1 gets 2♣ 45 times
- Seat 2 gets 2♣ 48 times

These aren't equivalent contexts.

**Current aggregation assumes**: Seat positions are equivalent (or effects average out)

**If false**: Results may be biased

**Fix**: Either:
1. **Document assumption clearly** in HLD
2. **Rotate starting position systematically** (see Issue #5)
3. **Weight by position** in aggregation

**Recommendation**: Rotate starting position + document assumption.

### 30. Unclear Definition of "Win"

**Location**: Section 3.2, SeatResults

**Issue**:
```rust
pub struct SeatResults {
    pub win_count: usize,  // Times had lowest score in game
    // ...
}
```

In Hearts, "win" = lowest score. But:
- What if tied for lowest?
- What if someone shoots the moon (0 points, others get 26)?

**Current code likely counts**: `seat_score == min(all_scores)`

**Problem with ties**:
- If seats 0 and 3 both score 5 (tied for best), do both get a "win"?
- Or neither?
- Or split 0.5 each?

**Problem with moon shots**:
- Moon shooter gets 0 points (clear win)
- Other 3 get 26 each (all tied for last)

**Fix**: Define precisely
```rust
pub struct SeatResults {
    pub solo_wins: usize,     // Exclusively lowest score
    pub tied_wins: usize,     // Tied for lowest
    pub losses: usize,        // Not lowest
    pub win_rate: f64,        // solo_wins / total_games
}
```

## Documentation Issues

### 31. No Migration Guide

**Location**: Section 9, Backward Compatibility

**Issue**: HLD mentions backward compatibility but no migration guide for users.

**Need**: Document how to migrate from old to new
```markdown
## Migration Guide

### Old Evaluation (Homogeneous)
bash
# Old way: Can't compare policies directly
mdhearts eval 200 --ai normal > baseline.txt
mdhearts eval 200 --ai embedded --weights weights.json > trained.txt
# Manually compare outputs (unreliable)


### New Evaluation (Mixed)
bash
# New way: Direct comparison
mdhearts eval 200 --ai-test embedded --test-seat 3 \
                  --baseline normal --weights weights.json
# Automatic statistical comparison


### Updating Scripts
If your scripts parse old JSON output, update to handle new format:

**Old JSON**:
json
{
  "ai_type": "Normal",
  "avg_points": [7.32, 7.13, 6.07, 5.49]
}


**New JSON** (with `--ai normal`, backward compatible):
json
{
  "ai_type": "Normal",
  "avg_points": [7.32, 7.13, 6.07, 5.49],
  "seat_results": [...]  // ← New field, old fields preserved
}

```

### 32. No Troubleshooting Section

**Location**: Missing from HLD

**Need**: Common errors and solutions
```markdown
## Troubleshooting

### Error: "Embedded AI requires weights"
**Cause**: Specified `--ai embedded` or `--ai-per-seat` with embedded, but no weights provided.
**Fix**: Add `--weights path/to/weights.json`

### Error: "Schema version mismatch"
**Cause**: Weights file was created with different schema version than current binary.
**Fix**: Retrain with current version, or use compatible checkpoint.

### Error: "Weights file not found"
**Cause**: Path to weights file is incorrect.
**Fix**: Check path, use absolute paths to avoid ambiguity.

### Warning: "Sample size too small for significance test"
**Cause**: Fewer than 30 games, p-value may be unreliable.
**Fix**: Increase number of games to ≥100 for meaningful statistics.

### Seats perform differently
**Cause**: Position effects in Hearts (starting position, card passing).
**Fix**: Use rotation testing to average across all positions.
```

### 33. No Performance Benchmarks

**Location**: Missing from HLD

**Issue**: Users want to know: Will this be slower than current eval?

**Need**: Benchmark section
```markdown
## Performance

Measured on [hardware specs]:

### Homogeneous Evaluation
- 1000 games, 4x Normal AI: 0.35s (2857 games/sec)
- 1000 games, 4x Embedded AI: 0.58s (1724 games/sec)

### Mixed Evaluation (Overhead)
- 1000 games, 3x Normal + 1x Embedded: 0.61s (1639 games/sec)
  - Overhead vs homogeneous: +5.2%
  - Overhead is minimal (policy creation happens once)

### Memory Usage
- 4x Normal AI: ~50MB
- 4x Embedded AI (same weights): ~52MB (weight sharing)
- 4x Embedded AI (different weights): ~58MB (4x 2MB models)

### Scalability
- Games scale linearly (O(n))
- Policies are created once (O(1))
- No performance regression for large evaluations
```

## Summary of Issues

| Category | Count | Severity |
|----------|-------|----------|
| Critical | 5 | Must fix before implementation |
| Important | 10 | Should fix for quality |
| Design | 7 | Improve usability/flexibility |
| Implementation | 5 | Affect development process |
| Usability | 3 | User experience improvements |
| Security | 2 | Low risk but good practice |
| Missing Features | 4 | Nice-to-have additions |
| Correctness | 2 | Edge cases and assumptions |
| Documentation | 3 | Essential for users |

**Total: 36 issues identified**

## Recommendations

### For Immediate Implementation

**Must fix before coding**:
1. ✅ Fix array ordering bug (#1)
2. ✅ Replace t-test with Mann-Whitney U (#2)
3. ✅ Add Bonferroni correction for multiple tests (#3)
4. ✅ Plan JSON format versioning (#4)
5. ✅ Implement systematic seat rotation (#5)

**Should fix during implementation**:
6. Reconsider "_" syntax (#6)
7. Generalize comparison structure (#7)
8. Add per-game results (#8)
9. Validate config at parse time (#9)
10. Share duplicate weights (#10)

### For Future Iterations

**Nice-to-have** (can defer):
- Dry-run mode (#20)
- Advanced output formats (#22)
- Game replay (#25)
- Meta-analysis tools (#27)

**Documentation** (critical for users):
- Migration guide (#31)
- Troubleshooting section (#32)
- Performance benchmarks (#33)

## Conclusion

The HLD proposes a valuable feature that solves a real problem. However, significant issues need to be addressed before implementation:

1. **Statistical methodology** must be corrected (non-parametric tests)
2. **Implementation bugs** must be fixed (array ordering, error handling)
3. **API design** should be simplified and clarified
4. **Backward compatibility** needs careful handling

With these fixes, the mixed evaluation system will provide meaningful comparisons between AI policies and significantly improve the training feedback loop.

**Recommendation**: Revise HLD to address critical issues before proceeding with implementation.

---

**Review Complete**
*36 issues identified, 15 critical/important, 21 improvements*
