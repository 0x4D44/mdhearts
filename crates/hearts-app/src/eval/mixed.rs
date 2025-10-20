// Mixed AI evaluation engine
#![allow(dead_code)]

use super::stats::compute_comparison;
use super::types::*;
use crate::bot::{BotFeatures, UnseenTracker};
use crate::cli::AiType;
use crate::policy::{EmbeddedPolicy, HeuristicPolicy, Policy, PolicyContext};
use hearts_core::belief::Belief;
use hearts_core::game::match_state::MatchState;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::round::RoundPhase;
use std::path::PathBuf;
use std::time::Instant;

/// Error type for evaluation operations
#[derive(Debug)]
pub enum EvalError {
    InvalidConfig(String),
    MissingWeights(String),
    PolicyCreation(String),
    GameExecution(String),
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::InvalidConfig(msg) => write!(f, "Invalid configuration: {}", msg),
            EvalError::MissingWeights(msg) => write!(f, "Missing weights: {}", msg),
            EvalError::PolicyCreation(msg) => write!(f, "Policy creation failed: {}", msg),
            EvalError::GameExecution(msg) => write!(f, "Game execution failed: {}", msg),
        }
    }
}

impl std::error::Error for EvalError {}

/// Run evaluation with mixed AI configuration
pub fn run_mixed_eval(config: MixedEvalConfig) -> Result<MixedEvalResults, EvalError> {
    let start_time = Instant::now();
    let bot_features = BotFeatures::from_env();

    // 1. Validate configuration
    validate_config(&config)?;

    println!("Configuration validated successfully");
    println!(
        "Running {} games with {:?} rotation mode",
        config.num_games, config.rotation_mode
    );

    // 2. Initialize policies for each seat
    let mut policies = create_policies(&config)?;

    // 3. Run games with rotation
    let mut results = Vec::new();
    let progress_interval = (config.num_games / 10).max(1); // 10% increments

    for game_idx in 0..config.num_games {
        // Apply rotation based on mode
        let seat_mapping = match config.rotation_mode {
            RotationMode::Fixed => [0, 1, 2, 3], // No rotation
            RotationMode::Systematic => {
                // Rotate every num_games/4 games
                let rotation = (game_idx / (config.num_games / 4)) % 4;
                rotate_seats(rotation)
            }
            RotationMode::Random => random_shuffle_seats(),
        };

        let game_result = run_single_game(&mut policies, seat_mapping)?;
        results.push(game_result);

        // Progress reporting (adaptive based on num_games)
        if (game_idx + 1) % progress_interval == 0 {
            print_progress(&results, game_idx + 1, config.num_games);
        }
    }

    // 4. Aggregate statistics
    let mut aggregated = aggregate_results(&results, &config)?;

    // 5. Compute comparisons if requested
    if let OutputMode::Comparison { test_policy_index } = config.output_mode {
        aggregated.comparison = Some(compute_comparison(&results, test_policy_index)?);
    }

    // 6. Set elapsed time
    aggregated.elapsed_seconds = start_time.elapsed().as_secs_f64();

    Ok(aggregated)
}

/// Validate configuration for internal consistency
fn validate_config(config: &MixedEvalConfig) -> Result<(), EvalError> {
    // Check 1: Systematic rotation requires num_games divisible by 4
    if config.rotation_mode == RotationMode::Systematic && config.num_games % 4 != 0 {
        return Err(EvalError::InvalidConfig(format!(
            "Systematic rotation requires num_games divisible by 4 (got {}). Use {} or {} games instead.",
            config.num_games,
            (config.num_games / 4) * 4,       // Round down
            ((config.num_games / 4) + 1) * 4  // Round up
        )));
    }

    // Check 2: Comparison mode requires valid test_policy_index
    if let OutputMode::Comparison { test_policy_index } = config.output_mode {
        if test_policy_index >= 4 {
            return Err(EvalError::InvalidConfig(format!(
                "test_policy_index {} out of range [0,3]",
                test_policy_index
            )));
        }

        // Check 3: Comparison mode requires homogeneous baseline
        let baseline_types: Vec<_> = config
            .policy_configs
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != test_policy_index)
            .map(|(_, cfg)| cfg.ai_type)
            .collect();

        if !baseline_types.windows(2).all(|w| w[0] == w[1]) {
            return Err(EvalError::InvalidConfig(format!(
                "Comparison mode requires homogeneous baseline (all non-test policies must be same type). \
                 Policy {} is test, but baseline policies have types: {:?}",
                test_policy_index, baseline_types
            )));
        }
    }

    // Check 4: Warn about systematic rotation with Standard mode
    if config.rotation_mode == RotationMode::Systematic {
        if let OutputMode::Standard = config.output_mode {
            eprintln!(
                "Warning: Systematic rotation with Standard output mode. \
                 Results show per-policy averages (not per-seat). \
                 Consider using Comparison mode for clearer output."
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
    [(4 - r) % 4, (5 - r) % 4, (6 - r) % 4, (7 - r) % 4]
}

/// Generate random seat shuffle (NOT RECOMMENDED - use Systematic instead)
fn random_shuffle_seats() -> [usize; 4] {
    use rand::seq::SliceRandom;
    let mut seats = [0, 1, 2, 3];
    seats.shuffle(&mut rand::thread_rng());
    seats
}

/// Create policy instances for each seat
fn create_policies(config: &MixedEvalConfig) -> Result<[Box<dyn Policy>; 4], EvalError> {
    let mut policies: Vec<Box<dyn Policy>> = Vec::new();

    for policy_config in &config.policy_configs {
        let policy = create_policy(policy_config.ai_type, policy_config.weights_path.as_ref())?;
        policies.push(policy);
    }

    // Convert Vec to array - preserve order!
    // CRITICAL: Don't use .pop() which reverses the order
    Ok(policies
        .try_into()
        .unwrap_or_else(|v: Vec<_>| panic!("Expected exactly 4 policies, got {}", v.len())))
}

/// Create policy instance from configuration
fn create_policy(
    ai_type: AiType,
    weights_path: Option<&PathBuf>,
) -> Result<Box<dyn Policy>, EvalError> {
    match ai_type {
        AiType::Easy => Ok(Box::new(HeuristicPolicy::easy())),
        AiType::Normal => Ok(Box::new(HeuristicPolicy::normal())),
        AiType::Hard => Ok(Box::new(HeuristicPolicy::hard())),
        AiType::Embedded => {
            let path = weights_path.ok_or_else(|| {
                EvalError::MissingWeights(
                    "Embedded AI requires weights file via --weights".to_string(),
                )
            })?;
            EmbeddedPolicy::from_file(path)
                .map(|p| Box::new(p) as Box<dyn Policy>)
                .map_err(|e| EvalError::PolicyCreation(format!("Failed to load weights: {}", e)))
        }
    }
}

/// Run a single game with mixed AIs and seat mapping
/// seat_mapping[physical_seat] = policy_index
fn run_single_game(
    policies: &mut [Box<dyn Policy>; 4],
    seat_mapping: [usize; 4],
) -> Result<GameResult, EvalError> {
    // Start a new match with random seed
    let seed = rand::random::<u64>();
    let mut match_state = MatchState::with_seed(PlayerPosition::North, seed);
    let mut trackers = [
        UnseenTracker::new(),
        UnseenTracker::new(),
        UnseenTracker::new(),
        UnseenTracker::new(),
    ];

    // Initialize trackers for the first round
    for tracker in &mut trackers {
        tracker.reset_for_round(match_state.round());
    }

    // Play until the match is complete (one round, terminal when all cards played)
    loop {
        // Check if round is complete
        if match_state.is_round_ready_for_scoring() {
            break;
        }

        // Handle passing phase
        if matches!(match_state.round().phase(), RoundPhase::Passing(_)) {
            // Collect passes from all policies
            for physical_seat in PlayerPosition::LOOP.iter().copied() {
                let policy_idx = seat_mapping[physical_seat.index()];

                // Get card choices from policy (narrow scope for borrow)
                let cards = {
                    let round = match_state.round();
                    let hand = round.hand(physical_seat);
                    let mut belief_holder: Option<Belief> = None;
                    let belief_ref = if bot_features.belief_enabled() {
                        belief_holder = Some(Belief::from_state(round, physical_seat));
                        belief_holder.as_ref()
                    } else {
                        None
                    };
                    let ctx = PolicyContext {
                        seat: physical_seat,
                        hand,
                        round,
                        scores: match_state.scores(),
                        passing_direction: match_state.passing_direction(),
                        tracker: &trackers[physical_seat.index()],
                        belief: belief_ref,
                        features: bot_features,
                        telemetry: None,
                    };
                    policies[policy_idx].choose_pass(&ctx)
                };

                // Track passed cards
                trackers[physical_seat.index()].note_pass_selection(physical_seat, &cards);

                match_state
                    .round_mut()
                    .submit_pass(physical_seat, cards)
                    .map_err(|e| {
                        EvalError::GameExecution(format!("Pass submission failed: {:?}", e))
                    })?;
            }

            // Resolve passes
            match_state.round_mut().resolve_passes().map_err(|e| {
                EvalError::GameExecution(format!("Pass resolution failed: {:?}", e))
            })?;

            continue;
        }

        // Play phase - determine whose turn it is and get card choice
        let (expected_seat, card) = {
            let round = match_state.round();
            let current_trick = round.current_trick();
            let expected_seat = if let Some(last_play) = current_trick.plays().last() {
                last_play.position.next()
            } else {
                current_trick.leader()
            };

            let policy_idx = seat_mapping[expected_seat.index()];
            let hand = round.hand(expected_seat);

            let mut belief_holder: Option<Belief> = None;
            let belief_ref = if bot_features.belief_enabled() {
                belief_holder = Some(Belief::from_state(round, expected_seat));
                belief_holder.as_ref()
            } else {
                None
            };

            let ctx = PolicyContext {
                seat: expected_seat,
                hand,
                round,
                scores: match_state.scores(),
                passing_direction: match_state.passing_direction(),
                tracker: &trackers[expected_seat.index()],
                belief: belief_ref,
                features: bot_features,
                telemetry: None,
            };

            let card = policies[policy_idx].choose_play(&ctx);
            (expected_seat, card)
        };

        // Track played card
        trackers[expected_seat.index()].note_card_played(expected_seat, card);

        // Play the card
        match_state
            .round_mut()
            .play_card(expected_seat, card)
            .map_err(|e| EvalError::GameExecution(format!("Invalid card play: {:?}", e)))?;
    }

    // Extract final scores and remap to policy indices
    let physical_penalties = match_state.round_penalties();
    let mut policy_points = [0u8; 4];
    for physical_seat in 0..4 {
        let policy_idx = seat_mapping[physical_seat];
        policy_points[policy_idx] = physical_penalties[physical_seat];
    }

    // Detect moon shooter (if any)
    let moon_shooter_policy = (0..4)
        .find(|&i| {
            physical_penalties[i] == 0 && physical_penalties.iter().all(|&p| p == 0 || p == 26)
        })
        .map(|physical_seat_idx| seat_mapping[physical_seat_idx]);

    Ok(GameResult {
        points: policy_points,
        moon_shooter: moon_shooter_policy,
    })
}

/// Aggregate per-game results into per-policy statistics
fn aggregate_results(
    results: &[GameResult],
    config: &MixedEvalConfig,
) -> Result<MixedEvalResults, EvalError> {
    let num_games = results.len();

    // Initialize counters for each policy
    let mut total_points = [0usize; 4];
    let mut moon_counts = [0usize; 4];
    let mut win_counts = [0usize; 4];

    for game_result in results {
        // Accumulate points
        for (policy_idx, total) in total_points.iter_mut().enumerate() {
            *total += game_result.points[policy_idx] as usize;
        }

        // Count moon shots
        if let Some(shooter_idx) = game_result.moon_shooter {
            moon_counts[shooter_idx] += 1;
        }

        // Count wins (lowest score)
        let min_score = *game_result.points.iter().min().unwrap();
        for (policy_idx, win_count) in win_counts.iter_mut().enumerate() {
            if game_result.points[policy_idx] == min_score {
                *win_count += 1;
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
            ai_label: config_for_policy
                .label
                .clone()
                .unwrap_or_else(|| format!("{:?}", config_for_policy.ai_type)),
            avg_points: total_points[policy_idx] as f64 / num_games as f64,
            total_points: total_points[policy_idx],
            moon_count: moon_counts[policy_idx],
            win_count: win_counts[policy_idx],
        });
    }

    Ok(MixedEvalResults {
        games_played: num_games,
        policy_results: policy_results
            .try_into()
            .unwrap_or_else(|_| panic!("Expected exactly 4 policy results")),
        comparison: None, // Filled in later if needed
        rotation_mode: config.rotation_mode.clone(),
        elapsed_seconds: 0.0, // Filled in by caller
    })
}

/// Print progress update
fn print_progress(results: &[GameResult], current: usize, total: usize) {
    let pct = (current as f64 / total as f64 * 100.0) as usize;
    println!(
        "Progress: {}/{} games ({}%) - {} results collected",
        current,
        total,
        pct,
        results.len()
    );
}
