// Data structures for mixed AI evaluation
#![allow(dead_code)]

use crate::cli::AiType;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for a single policy (not a physical seat)
/// With rotation, policies move between seats, so this configures the AI itself
#[derive(Debug, Clone)]
pub struct PolicyConfig {
    pub ai_type: AiType,
    pub weights_path: Option<PathBuf>,
    pub label: Option<String>, // User-friendly name for reporting
}

/// How to handle seat rotation to eliminate position bias
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RotationMode {
    /// Fixed seating: AI stays in assigned seat
    Fixed,
    /// Systematic rotation: rotate AIs through all positions evenly
    /// (num_games must be divisible by 4)
    Systematic,
    /// Random seating: shuffle AIs randomly each game (NOT RECOMMENDED)
    Random,
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

/// Configuration for mixed AI evaluation
#[derive(Debug, Clone)]
pub struct MixedEvalConfig {
    pub num_games: usize,
    pub policy_configs: [PolicyConfig; 4], // One config per policy
    pub output_mode: OutputMode,
    pub rotation_mode: RotationMode,
}

/// Result from a single game (after remapping to policy indices)
#[derive(Debug, Clone)]
pub struct GameResult {
    /// Points earned by each policy in this game
    /// points[policy_index] = points earned
    pub points: [u8; 4],
    /// Index of policy that shot the moon (if any)
    pub moon_shooter: Option<usize>,
}

/// Per-policy results (not per-seat, since policies rotate through seats)
#[derive(Debug, Clone, Serialize)]
pub struct PolicyResults {
    pub policy_index: usize, // Index in the policy array [0-3]
    pub ai_type: AiType,     // Type of AI
    pub ai_label: String,    // User-friendly name
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
    pub test_policy_index: usize, // Which policy is being tested
    pub test_avg: f64,
    pub baseline_avg: f64,                     // Average of non-test policies
    pub difference: f64,                       // test_avg - baseline_avg (negative = better)
    pub percent_improvement: f64,              // (baseline - test) / baseline * 100
    pub statistical_significance: Option<f64>, // p-value if enough games
    pub statistical_test: String,              // Name of test used (e.g., "mann_whitney_u")
}

/// Results from mixed evaluation
#[derive(Debug, Clone, Serialize)]
pub struct MixedEvalResults {
    pub games_played: usize,
    pub policy_results: [PolicyResults; 4], // Per-policy statistics
    pub comparison: Option<ComparisonResults>,
    pub rotation_mode: RotationMode, // Important for interpreting results
    pub elapsed_seconds: f64,
}
