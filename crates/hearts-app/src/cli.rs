use hearts_core::game::match_state::MatchState;
use hearts_core::game::serialization::MatchSnapshot;
use hearts_core::model::player::PlayerPosition;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MESSAGEBOX_STYLE, MessageBoxW,
};

pub enum CliOutcome {
    Handled,
    NotHandled,
}

/// AI type selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiType {
    Easy,
    Normal,
    Hard,
    Embedded,
}

impl AiType {
    pub fn from_str(s: &str) -> Result<Self, CliError> {
        match s.to_ascii_lowercase().as_str() {
            "easy" | "legacy" => Ok(AiType::Easy),
            "normal" | "default" => Ok(AiType::Normal),
            "hard" | "future" => Ok(AiType::Hard),
            "embedded" | "ml" => Ok(AiType::Embedded),
            other => Err(CliError::InvalidAiType(other.to_string())),
        }
    }
}

#[derive(Debug)]
pub enum CliError {
    UnknownCommand(String),
    MissingArgument(&'static str),
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidSeat(String),
    InvalidAiType(String),
    InvalidNumber(String),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::UnknownCommand(cmd) => write!(f, "Unknown command: {cmd}"),
            CliError::MissingArgument(arg) => write!(f, "Missing argument: {arg}"),
            CliError::Io(err) => write!(f, "I/O error: {err}"),
            CliError::Json(err) => write!(f, "JSON error: {err}"),
            CliError::InvalidSeat(value) => write!(f, "Invalid seat: {value}"),
            CliError::InvalidAiType(value) => {
                write!(
                    f,
                    "Invalid AI type: {value}. Valid types: easy, normal, hard, embedded"
                )
            }
            CliError::InvalidNumber(value) => write!(f, "Invalid number: {value}"),
        }
    }
}

impl From<std::io::Error> for CliError {
    fn from(value: std::io::Error) -> Self {
        CliError::Io(value)
    }
}

impl From<serde_json::Error> for CliError {
    fn from(value: serde_json::Error) -> Self {
        CliError::Json(value)
    }
}

pub fn run_cli() -> Result<CliOutcome, CliError> {
    let mut args = std::env::args().skip(1);
    let Some(cmd) = args.next() else {
        return Ok(CliOutcome::NotHandled);
    };

    match cmd.as_str() {
        "--export-snapshot" => {
            let path = args
                .next()
                .map(PathBuf::from)
                .ok_or(CliError::MissingArgument(
                    "--export-snapshot <path> [seed] [seat]",
                ))?;
            let seed = args.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            let seat = args
                .next()
                .map(|s| parse_seat(&s))
                .transpose()?
                .unwrap_or(PlayerPosition::North);
            export_snapshot(path, seed, seat)?;
            Ok(CliOutcome::Handled)
        }
        "--import-snapshot" => {
            let path = args
                .next()
                .map(PathBuf::from)
                .ok_or(CliError::MissingArgument("--import-snapshot <path>"))?;
            import_snapshot(path)?;
            Ok(CliOutcome::Handled)
        }
        "eval" | "--eval" => {
            let games_arg = args.next().ok_or(CliError::MissingArgument(
                "eval <games> [--ai <type>] [--weights <path>] [--collect-data <path>] [--self-play] [--collect-rl <path>] [--reward-mode <mode>] [--ai-test <type>] [--ai-per-seat <types>] [--weights-per-seat <paths>]",
            ))?;
            let num_games = games_arg
                .parse::<usize>()
                .map_err(|_| CliError::InvalidNumber(games_arg))?;

            // Parse optional flags
            let mut ai_type = AiType::Normal;
            let mut weights_path: Option<PathBuf> = None;
            let mut collect_data_path: Option<PathBuf> = None;
            let mut self_play = false;
            let mut collect_rl_path: Option<PathBuf> = None;
            let mut reward_mode_str = "shaped".to_string();
            let mut ai_test: Option<AiType> = None;
            let mut ai_per_seat: Option<String> = None;
            let mut weights_per_seat: Option<String> = None;

            while let Some(flag) = args.next() {
                match flag.as_str() {
                    "--ai" => {
                        let ai_str = args
                            .next()
                            .ok_or(CliError::MissingArgument("--ai <type>"))?;
                        ai_type = AiType::from_str(&ai_str)?;
                    }
                    "--weights" => {
                        let path_str = args
                            .next()
                            .ok_or(CliError::MissingArgument("--weights <path>"))?;
                        weights_path = Some(PathBuf::from(path_str));
                    }
                    "--collect-data" => {
                        let path_str = args
                            .next()
                            .ok_or(CliError::MissingArgument("--collect-data <path>"))?;
                        collect_data_path = Some(PathBuf::from(path_str));
                    }
                    "--self-play" => {
                        self_play = true;
                    }
                    "--collect-rl" => {
                        let path_str = args
                            .next()
                            .ok_or(CliError::MissingArgument("--collect-rl <path>"))?;
                        collect_rl_path = Some(PathBuf::from(path_str));
                    }
                    "--reward-mode" => {
                        reward_mode_str = args
                            .next()
                            .ok_or(CliError::MissingArgument("--reward-mode <mode>"))?;
                    }
                    "--ai-test" => {
                        let ai_str = args
                            .next()
                            .ok_or(CliError::MissingArgument("--ai-test <type>"))?;
                        ai_test = Some(AiType::from_str(&ai_str)?);
                    }
                    "--ai-per-seat" => {
                        let types_str = args.next().ok_or(CliError::MissingArgument(
                            "--ai-per-seat <type1>,<type2>,<type3>,<type4>",
                        ))?;
                        ai_per_seat = Some(types_str);
                    }
                    "--weights-per-seat" => {
                        let paths_str = args.next().ok_or(CliError::MissingArgument(
                            "--weights-per-seat <path1>,<path2>,<path3>,<path4>",
                        ))?;
                        weights_per_seat = Some(paths_str);
                    }
                    _ => {
                        return Err(CliError::UnknownCommand(format!("Unknown flag: {}", flag)));
                    }
                }
            }

            // Determine evaluation mode
            if ai_test.is_some() || ai_per_seat.is_some() {
                // Mixed evaluation mode
                run_mixed_eval_cli(
                    num_games,
                    ai_type,
                    weights_path,
                    ai_test,
                    ai_per_seat,
                    weights_per_seat,
                )?;
            } else if self_play {
                use crate::rl::StepRewardMode;
                let reward_mode =
                    StepRewardMode::from_str(&reward_mode_str).map_err(CliError::UnknownCommand)?;
                run_self_play_eval(num_games, weights_path, collect_rl_path, reward_mode)?;
            } else {
                run_eval(num_games, ai_type, weights_path, collect_data_path)?;
            }
            Ok(CliOutcome::Handled)
        }
        "play" | "--play" => {
            // Check for --ai flag
            let ai_type = if let Some(next_arg) = args.next() {
                if next_arg == "--ai" {
                    let ai_str = args
                        .next()
                        .ok_or(CliError::MissingArgument("--ai <type>"))?;
                    AiType::from_str(&ai_str)?
                } else {
                    AiType::Normal // Default
                }
            } else {
                AiType::Normal // Default
            };

            println!("Starting game with AI type: {:?}", ai_type);
            // For now, just launch normal GUI mode
            // (The AI type will be used when the controller is updated)
            Ok(CliOutcome::NotHandled)
        }
        "--schema-info" => {
            use crate::rl::observation::{SCHEMA_HASH, SCHEMA_VERSION};
            println!(
                "{{\"schema_version\":\"{}\",\"schema_hash\":\"{}\"}}",
                SCHEMA_VERSION, SCHEMA_HASH
            );
            Ok(CliOutcome::Handled)
        }
        "--help" | "-h" => {
            let help = concat!(
                "Available commands:\n",
                "  eval <games> [--ai <type>] [--weights <path>] [--collect-data <path>]\n",
                "    Run headless evaluation mode\n",
                "  play [--ai <type>]\n",
                "    Start GUI game\n",
                "  --export-snapshot <path> [seed] [seat]\n",
                "    Export game snapshot to JSON\n",
                "  --import-snapshot <path>\n",
                "    Import game snapshot from JSON\n",
                "  --schema-info\n",
                "    Print observation schema version and hash\n",
                "  --help\n",
                "    Show this help message\n",
                "\nOptions:\n",
                "  --ai <type>           AI type: easy, normal, hard, embedded\n",
                "  --weights <path>      Custom weights JSON (for embedded AI)\n",
                "  --collect-data <path> Save training data to JSONL file"
            );
            println!("{help}");
            show_info_box("mdhearts CLI", help);
            Ok(CliOutcome::Handled)
        }
        other => Err(CliError::UnknownCommand(other.to_string())),
    }
}

fn export_snapshot(path: PathBuf, seed: u64, seat: PlayerPosition) -> Result<(), CliError> {
    let state = MatchState::with_seed(seat, seed);
    let json = MatchSnapshot::to_json(&state)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, json)?;

    let message = format!(
        "Snapshot saved to {}\nSeed: {}\nSeat: {:?}",
        path.display(),
        seed,
        seat
    );
    println!("{message}");
    show_info_box("Snapshot Exported", &message);
    Ok(())
}

fn import_snapshot(path: PathBuf) -> Result<(), CliError> {
    if !path.exists() {
        return Err(CliError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Snapshot not found: {}", path.display()),
        )));
    }

    let json = fs::read_to_string(&path)?;
    let snapshot = MatchSnapshot::from_json(&json)?;
    let restored = snapshot.clone().restore();
    let message = format!(
        "Snapshot loaded from {}\nSeed: {}\nPassing: {}\nScores: {:?}",
        path.display(),
        restored.seed(),
        restored.passing_direction().as_str(),
        restored.scores().standings()
    );
    println!("{message}");
    show_info_box("Snapshot Loaded", &message);
    Ok(())
}

fn parse_seat(input: &str) -> Result<PlayerPosition, CliError> {
    let normalized = input.to_ascii_lowercase();
    match normalized.as_str() {
        "north" | "n" => Ok(PlayerPosition::North),
        "east" | "e" => Ok(PlayerPosition::East),
        "south" | "s" => Ok(PlayerPosition::South),
        "west" | "w" => Ok(PlayerPosition::West),
        other => Err(CliError::InvalidSeat(other.to_string())),
    }
}

pub fn show_error_box(message: &str) {
    eprintln!("{message}");
    if message_boxes_enabled() {
        show_box("mdhearts CLI", message, MB_ICONERROR | MB_OK);
    }
}

fn show_info_box(title: &str, message: &str) {
    if message_boxes_enabled() {
        show_box(title, message, MB_ICONINFORMATION | MB_OK);
    }
}

fn message_boxes_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("MDH_CLI_POPUPS")
            .map(|value| {
                let value = value.trim();
                value == "1"
                    || value.eq_ignore_ascii_case("true")
                    || value.eq_ignore_ascii_case("yes")
                    || value.eq_ignore_ascii_case("on")
            })
            .unwrap_or(false)
    })
}

fn show_box(title: &str, message: &str, flags: MESSAGEBOX_STYLE) {
    let title_wide = encode_wide(title);
    let message_wide = encode_wide(message);
    unsafe {
        MessageBoxW(
            Some(HWND::default()),
            windows::core::PCWSTR(message_wide.as_ptr()),
            windows::core::PCWSTR(title_wide.as_ptr()),
            flags,
        );
    }
}

fn encode_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Run mixed AI evaluation mode (--ai-test or --ai-per-seat)
fn run_mixed_eval_cli(
    num_games: usize,
    baseline_ai: AiType,
    baseline_weights: Option<PathBuf>,
    ai_test: Option<AiType>,
    ai_per_seat: Option<String>,
    weights_per_seat: Option<String>,
) -> Result<(), CliError> {
    use crate::eval::mixed::{EvalError, run_mixed_eval};
    use crate::eval::types::{MixedEvalConfig, OutputMode, PolicyConfig, RotationMode};

    // Build policy configs based on mode
    let (policy_configs, output_mode) = if let Some(test_ai) = ai_test {
        // --ai-test mode: 3 baseline + 1 test (comparison mode)
        println!(
            "Running {} games in comparison mode: 3x {:?} vs 1x {:?}",
            num_games, baseline_ai, test_ai
        );

        let baseline_config = PolicyConfig {
            ai_type: baseline_ai,
            weights_path: baseline_weights.clone(),
            label: Some(format!("{:?} (baseline)", baseline_ai)),
        };

        let test_config = PolicyConfig {
            ai_type: test_ai,
            weights_path: if test_ai == AiType::Embedded {
                baseline_weights.clone() // Use same weights path if provided
            } else {
                None
            },
            label: Some(format!("{:?} (test)", test_ai)),
        };

        (
            [
                baseline_config.clone(),
                baseline_config.clone(),
                baseline_config,
                test_config,
            ],
            OutputMode::Comparison {
                test_policy_index: 3,
            },
        )
    } else if let Some(types_str) = ai_per_seat {
        // --ai-per-seat mode: custom mix
        println!("Running {} games in mixed mode: {}", num_games, types_str);

        // Parse AI types
        let type_parts: Vec<&str> = types_str.split(',').collect();
        if type_parts.len() != 4 {
            return Err(CliError::InvalidAiType(format!(
                "Expected 4 AI types, got {}",
                type_parts.len()
            )));
        }

        let ai_types: Result<Vec<AiType>, CliError> = type_parts
            .iter()
            .map(|s| AiType::from_str(s.trim()))
            .collect();
        let ai_types = ai_types?;

        // Parse weights if provided
        let weights: Vec<Option<PathBuf>> = if let Some(weights_str) = weights_per_seat {
            let weight_parts: Vec<&str> = weights_str.split(',').collect();
            if weight_parts.len() != 4 {
                return Err(CliError::InvalidAiType(format!(
                    "Expected 4 weight paths, got {}",
                    weight_parts.len()
                )));
            }

            weight_parts
                .iter()
                .map(|s| {
                    let trimmed = s.trim();
                    if trimmed == "_" {
                        None
                    } else {
                        Some(PathBuf::from(trimmed))
                    }
                })
                .collect()
        } else {
            vec![None, None, None, None]
        };

        // Validate embedded AIs have weights
        for (i, (ai_type, weight)) in ai_types.iter().zip(weights.iter()).enumerate() {
            if *ai_type == AiType::Embedded && weight.is_none() {
                return Err(CliError::InvalidAiType(format!(
                    "Policy {} is embedded but no weights specified. Use '_' for non-embedded AIs.",
                    i
                )));
            }
        }

        // Build configs
        let configs: Vec<PolicyConfig> = ai_types
            .iter()
            .zip(weights.iter())
            .enumerate()
            .map(|(i, (ai_type, weight))| PolicyConfig {
                ai_type: *ai_type,
                weights_path: weight.clone(),
                label: Some(format!("Policy{} ({:?})", i, ai_type)),
            })
            .collect();

        (
            [
                configs[0].clone(),
                configs[1].clone(),
                configs[2].clone(),
                configs[3].clone(),
            ],
            OutputMode::Standard,
        )
    } else {
        return Err(CliError::InvalidAiType(
            "Either --ai-test or --ai-per-seat must be specified".to_string(),
        ));
    };

    // Create config
    let config = MixedEvalConfig {
        num_games,
        policy_configs,
        output_mode,
        rotation_mode: RotationMode::Systematic,
    };

    // Run evaluation
    let results = run_mixed_eval(config).map_err(|e| match e {
        EvalError::InvalidConfig(msg) => CliError::InvalidAiType(msg),
        EvalError::MissingWeights(msg) => CliError::InvalidAiType(msg),
        EvalError::PolicyCreation(msg) => CliError::InvalidAiType(msg),
        EvalError::GameExecution(msg) => {
            CliError::Io(std::io::Error::new(std::io::ErrorKind::Other, msg))
        }
    })?;

    // Print results
    println!("\n=== Mixed Evaluation Results ===");
    println!("Games played: {}", results.games_played);
    println!("Rotation mode: {:?}", results.rotation_mode);
    println!("Elapsed time: {:.2}s", results.elapsed_seconds);
    println!();

    for policy_result in &results.policy_results {
        println!(
            "Policy {}: {}",
            policy_result.policy_index, policy_result.ai_label
        );
        println!("  Avg points: {:.2}", policy_result.avg_points);
        println!("  Total points: {}", policy_result.total_points);
        println!("  Wins: {}", policy_result.win_count);
        println!("  Moons: {}", policy_result.moon_count);
        println!();
    }

    if let Some(comparison) = &results.comparison {
        println!("=== Comparison Results ===");
        println!("Test policy: Policy {}", comparison.test_policy_index);
        println!("Test avg: {:.2} points", comparison.test_avg);
        println!("Baseline avg: {:.2} points", comparison.baseline_avg);
        println!(
            "Difference: {:.2} points (negative = better)",
            comparison.difference
        );
        println!("Improvement: {:.1}%", comparison.percent_improvement);

        if let Some(p_value) = comparison.statistical_significance {
            println!("Statistical test: {}", comparison.statistical_test);
            println!("P-value: {:.4}", p_value);
            if p_value < 0.05 {
                println!("Result: SIGNIFICANT (p < 0.05)");
            } else {
                println!("Result: Not significant (p >= 0.05)");
            }
        } else {
            println!("Statistical significance: Not enough games for test");
        }
    }

    Ok(())
}

/// Run headless evaluation mode
fn run_eval(
    num_games: usize,
    ai_type: AiType,
    weights_path: Option<PathBuf>,
    collect_data_path: Option<PathBuf>,
) -> Result<(), CliError> {
    use crate::bot::{BotFeatures, UnseenTracker};
    use crate::policy::{EmbeddedPolicy, HeuristicPolicy, Policy, PolicyContext};
    use crate::rl::{Experience, ExperienceCollector};
    use hearts_core::belief::Belief;
    use hearts_core::model::round::RoundPhase;
    use serde_json::json;

    if let Some(ref path) = weights_path {
        println!(
            "Running {} games with AI type: {:?}, custom weights: {}",
            num_games,
            ai_type,
            path.display()
        );
    } else {
        println!("Running {} games with AI type: {:?}", num_games, ai_type);
    }

    // Create experience collector if requested
    let mut collector = if let Some(ref path) = collect_data_path {
        println!("Collecting training data to: {}", path.display());
        Some(
            ExperienceCollector::new(path)
                .map_err(|e| CliError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?,
        )
    } else {
        None
    };

    // Create policy based on AI type
    let mut policy: Box<dyn Policy> = match ai_type {
        AiType::Easy => Box::new(HeuristicPolicy::easy()),
        AiType::Normal => Box::new(HeuristicPolicy::normal()),
        AiType::Hard => Box::new(HeuristicPolicy::hard()),
        AiType::Embedded => {
            if let Some(path) = weights_path {
                Box::new(EmbeddedPolicy::from_file(path).map_err(|e| {
                    CliError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
                })?)
            } else {
                Box::new(EmbeddedPolicy::new())
            }
        }
    };

    let mut total_points = [0u32; 4];
    let mut moon_counts = [0usize; 4];
    let start_time = std::time::Instant::now();

    // ObservationBuilder for experience collection
    let obs_builder = if collector.is_some() {
        Some(crate::rl::observation::ObservationBuilder::new())
    } else {
        None
    };

    let bot_features = BotFeatures::from_env();

    for game_idx in 0..num_games {
        let seed = game_idx as u64;
        let mut match_state = MatchState::with_seed(PlayerPosition::South, seed);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(match_state.round());

        let mut step_id = 0;
        let mut pending_experiences: Vec<(usize, u8, Vec<f32>)> = Vec::new(); // (step_id, action, obs)

        // Play one complete round
        loop {
            // Handle passing phase
            if matches!(match_state.round().phase(), RoundPhase::Passing(_)) {
                for seat in PlayerPosition::LOOP {
                    let hand = match_state.round().hand(seat);
                    let scores = match_state.scores();
                    let passing_dir = match_state.passing_direction();

                    let mut belief_holder: Option<Belief> = None;
                    let belief_ref = if bot_features.belief_enabled() {
                        belief_holder = Some(Belief::from_state(match_state.round(), seat));
                        belief_holder.as_ref()
                    } else {
                        None
                    };

                    let ctx = PolicyContext {
                        hand,
                        round: match_state.round(),
                        scores,
                        seat,
                        tracker: &tracker,
                        passing_direction: passing_dir,
                        belief: belief_ref,
                        features: bot_features,
                    };

                    let pass_cards = policy.choose_pass(&ctx);
                    let _ = match_state.round_mut().submit_pass(seat, pass_cards);
                }
                // Resolve passes to transition to playing phase
                let _ = match_state.round_mut().resolve_passes();
                continue;
            }

            // Handle playing phase
            if matches!(match_state.round().phase(), RoundPhase::Playing) {
                // Determine which player should play next
                let current_player = {
                    let trick = match_state.round().current_trick();
                    trick
                        .plays()
                        .last()
                        .map(|p| p.position.next())
                        .unwrap_or(trick.leader())
                };
                let hand = match_state.round().hand(current_player);
                let scores = match_state.scores();
                let passing_dir = match_state.passing_direction();

                let mut belief_holder: Option<Belief> = None;
                let belief_ref = if bot_features.belief_enabled() {
                    belief_holder = Some(Belief::from_state(match_state.round(), current_player));
                    belief_holder.as_ref()
                } else {
                    None
                };

                let ctx = PolicyContext {
                    hand,
                    round: match_state.round(),
                    scores,
                    seat: current_player,
                    tracker: &tracker,
                    passing_direction: passing_dir,
                    belief: belief_ref,
                    features: bot_features,
                };

                // Build observation if collecting data
                if let Some(ref builder) = obs_builder {
                    let obs = builder.build(&ctx);
                    let obs_vec = obs.as_array().to_vec();

                    // Get action
                    let card = policy.choose_play(&ctx);

                    // Store for later (reward assignment at end of round)
                    pending_experiences.push((step_id, card.to_id(), obs_vec));
                    step_id += 1;

                    tracker.note_card_played(current_player, card);

                    match match_state.round_mut().play_card(current_player, card) {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Error playing card: {:?}", e);
                            break;
                        }
                    }
                } else {
                    // No data collection - just play
                    let card = policy.choose_play(&ctx);
                    tracker.note_card_played(current_player, card);

                    match match_state.round_mut().play_card(current_player, card) {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Error playing card: {:?}", e);
                            break;
                        }
                    }
                }

                // Check if round is complete (13 tricks)
                if match_state.round().tricks_completed() >= 13 {
                    break;
                }
            } else {
                break;
            }
        }

        // Assign rewards and record experiences
        if let Some(ref mut coll) = collector {
            let round_points = match_state.round().penalty_totals();

            // Compute rewards (negative points)
            let rewards: [f32; 4] = [
                -(round_points[0] as f32),
                -(round_points[1] as f32),
                -(round_points[2] as f32),
                -(round_points[3] as f32),
            ];

            // Record all experiences with their rewards
            for (sid, action, obs_vec) in pending_experiences.iter() {
                // For simplicity, assign full reward to all steps
                // (More sophisticated credit assignment could be added later)
                let exp = Experience {
                    observation: obs_vec.clone(),
                    action: *action,
                    reward: rewards[PlayerPosition::South.index()],
                    done: *sid == step_id - 1,
                    game_id: game_idx,
                    step_id: *sid,
                    seat: PlayerPosition::South.index() as u8,
                };

                if let Err(e) = coll.record(exp) {
                    eprintln!("Warning: Failed to record experience: {}", e);
                }
            }

            pending_experiences.clear();
        }

        // Tally scores
        let round_points = match_state.round().penalty_totals();
        for (idx, &points) in round_points.iter().enumerate() {
            total_points[idx] += points as u32;
        }

        // Detect moon shooting
        if let Some(shooter_idx) = round_points.iter().position(|&p| p == 26) {
            let others_total: u8 = round_points
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != shooter_idx)
                .map(|(_, &p)| p)
                .sum();

            if others_total == 0 {
                moon_counts[shooter_idx] += 1;
            }
        }

        // Output progress every 10% or on last game
        if (game_idx + 1) % (num_games / 10).max(1) == 0 || game_idx == num_games - 1 {
            let progress = json!({
                "completed": game_idx + 1,
                "total": num_games,
                "avg_points": [
                    total_points[0] as f64 / (game_idx + 1) as f64,
                    total_points[1] as f64 / (game_idx + 1) as f64,
                    total_points[2] as f64 / (game_idx + 1) as f64,
                    total_points[3] as f64 / (game_idx + 1) as f64,
                ],
                "moon_counts": moon_counts,
            });
            println!("{}", serde_json::to_string(&progress).unwrap());
        }
    }

    let elapsed = start_time.elapsed();

    // Final summary
    let summary = json!({
        "games": num_games,
        "ai_type": format!("{:?}", ai_type),
        "elapsed_seconds": elapsed.as_secs_f64(),
        "total_points": total_points,
        "avg_points": [
            total_points[0] as f64 / num_games as f64,
            total_points[1] as f64 / num_games as f64,
            total_points[2] as f64 / num_games as f64,
            total_points[3] as f64 / num_games as f64,
        ],
        "moon_counts": moon_counts,
    });

    println!("\nFinal Summary:");
    println!("{}", serde_json::to_string_pretty(&summary).unwrap());

    // Flush and report experience collection
    if let Some(ref mut coll) = collector {
        coll.flush()
            .map_err(|e| CliError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        println!(
            "\nCollected {} experiences to {}",
            coll.count(),
            collect_data_path.as_ref().unwrap().display()
        );
    }

    Ok(())
}

/// Run self-play evaluation mode with RL experience collection
fn run_self_play_eval(
    num_games: usize,
    weights_path: Option<PathBuf>,
    collect_rl_path: Option<PathBuf>,
    reward_mode: crate::rl::StepRewardMode,
) -> Result<(), CliError> {
    use crate::bot::UnseenTracker;
    use crate::policy::{EmbeddedPolicy, Policy, PolicyContext};
    use crate::rl::{ObservationBuilder, RLExperience, RLExperienceCollector, RewardComputer};
    use hearts_core::model::round::RoundPhase;

    println!("Running {} games in self-play mode", num_games);
    if let Some(ref path) = weights_path {
        println!("Using custom weights: {}", path.display());
    }
    if let Some(ref path) = collect_rl_path {
        println!("Collecting RL experiences to: {}", path.display());
    }
    println!("Reward mode: {:?}", reward_mode);

    // Load policy for all 4 players (same policy)
    let mut policy: Box<dyn Policy> =
        if let Some(path) = weights_path {
            Box::new(EmbeddedPolicy::from_file(path).map_err(|e| {
                CliError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            })?)
        } else {
            Box::new(EmbeddedPolicy::new())
        };

    // Create experience collector if requested
    let mut collector = if let Some(ref path) = collect_rl_path {
        Some(
            RLExperienceCollector::new(path)
                .map_err(|e| CliError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?,
        )
    } else {
        None
    };

    let obs_builder = ObservationBuilder::new();
    let reward_computer = RewardComputer::new(reward_mode);

    struct PendingExperience {
        observation: Vec<f32>,
        action: u8,
        value: f32,
        log_prob: f32,
        seat_idx: usize,
        step_id: usize,
        prev_hand_size: usize,
        prev_tricks: usize,
        prev_penalties: [u8; 4],
    }

    fn emit_pending(
        coll: &mut RLExperienceCollector,
        reward_computer: &RewardComputer,
        state: &MatchState,
        pending: PendingExperience,
        done: bool,
        game_id: usize,
    ) {
        let seat = PlayerPosition::from_index(pending.seat_idx).expect("valid seat index");
        let step_reward = reward_computer.compute_step_reward(
            state,
            seat,
            pending.prev_hand_size,
            pending.prev_tricks,
            pending.prev_penalties,
        );
        let terminal_reward = reward_computer.compute_terminal_reward(state, seat);
        let reward = if done { terminal_reward } else { step_reward };

        let experience = RLExperience {
            observation: pending.observation,
            action: pending.action,
            reward,
            done,
            game_id,
            step_id: pending.step_id,
            seat: pending.seat_idx as u8,
            value: pending.value,
            log_prob: pending.log_prob,
        };

        if let Err(e) = coll.record(experience) {
            eprintln!("Warning: Failed to record experience: {}", e);
        }
    }

    let mut total_points = [0u32; 4];
    let start_time = std::time::Instant::now();

    for game_idx in 0..num_games {
        let seed = game_idx as u64;
        let mut match_state = MatchState::with_seed(PlayerPosition::South, seed);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(match_state.round());

        let mut pending: [Option<PendingExperience>; 4] = [None, None, None, None];
        let mut step_ids = [0usize; 4];

        // Play one complete round
        loop {
            // Handle passing phase
            if matches!(match_state.round().phase(), RoundPhase::Passing(_)) {
                for seat in PlayerPosition::LOOP {
                    let hand = match_state.round().hand(seat);
                    let scores = match_state.scores();
                    let passing_dir = match_state.passing_direction();

                    let ctx = PolicyContext {
                        hand,
                        round: match_state.round(),
                        scores,
                        seat,
                        tracker: &tracker,
                        passing_direction: passing_dir,
                    };

                    let pass_cards = policy.choose_pass(&ctx);
                    let _ = match_state.round_mut().submit_pass(seat, pass_cards);
                }
                let _ = match_state.round_mut().resolve_passes();
                continue;
            }

            // Handle playing phase
            if matches!(match_state.round().phase(), RoundPhase::Playing) {
                let current_player = {
                    let trick = match_state.round().current_trick();
                    trick
                        .plays()
                        .last()
                        .map(|p| p.position.next())
                        .unwrap_or(trick.leader())
                };

                let seat_idx = current_player.index();

                if let Some(ref mut coll) = collector {
                    if let Some(prev) = pending[seat_idx].take() {
                        emit_pending(coll, &reward_computer, &match_state, prev, false, game_idx);
                    }
                } else {
                    pending[seat_idx] = None;
                }

                let hand = match_state.round().hand(current_player);
                let scores = match_state.scores();
                let passing_dir = match_state.passing_direction();

                let ctx = PolicyContext {
                    hand,
                    round: match_state.round(),
                    scores,
                    seat: current_player,
                    tracker: &tracker,
                    passing_direction: passing_dir,
                };

                // Build observation
                let obs = obs_builder.build(&ctx);
                let observation = obs.as_array().to_vec();

                // Get action with value and log_prob
                let (card, value, log_prob) = policy.forward_with_critic(&ctx);

                // Track state before action
                let prev_hand_size = hand.len();
                let prev_tricks = match_state.round().tricks_completed();
                let prev_penalties = match_state.round().penalty_totals();
                let step_id = step_ids[seat_idx];
                let card_id = card.to_id();

                if collector.is_some() {
                    pending[seat_idx] = Some(PendingExperience {
                        observation,
                        action: card_id,
                        value,
                        log_prob,
                        seat_idx,
                        step_id,
                        prev_hand_size,
                        prev_tricks,
                        prev_penalties,
                    });
                }

                // Execute action
                tracker.note_card_played(current_player, card);
                match match_state.round_mut().play_card(current_player, card) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Error playing card: {:?}", e);
                        break;
                    }
                }
                step_ids[seat_idx] += 1;

                // Check if round is complete
                if match_state.round().tricks_completed() >= 13 {
                    break;
                }
            } else {
                break;
            }
        }

        if let Some(ref mut coll) = collector {
            for entry in pending.iter_mut() {
                if let Some(prev) = entry.take() {
                    emit_pending(coll, &reward_computer, &match_state, prev, true, game_idx);
                }
            }
        }

        // Tally scores
        let round_points = match_state.round().penalty_totals();
        for (idx, &points) in round_points.iter().enumerate() {
            total_points[idx] += points as u32;
        }

        // Output progress
        if (game_idx + 1) % (num_games / 10).max(1) == 0 || game_idx == num_games - 1 {
            println!(
                "Progress: {}/{} games ({:.1}%)",
                game_idx + 1,
                num_games,
                (game_idx + 1) as f64 / num_games as f64 * 100.0
            );
        }
    }

    let elapsed = start_time.elapsed();

    // Final summary
    println!("\n=== Self-Play Evaluation Summary ===");
    println!("Games: {}", num_games);
    println!("Elapsed: {:.2}s", elapsed.as_secs_f64());
    println!("Average points per seat:");
    for (idx, &points) in total_points.iter().enumerate() {
        println!("  Seat {}: {:.2}", idx, points as f64 / num_games as f64);
    }

    // Flush and report experience collection
    if let Some(ref mut coll) = collector {
        coll.flush()
            .map_err(|e| CliError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        println!(
            "\nCollected {} RL experiences to {}",
            coll.count(),
            collect_rl_path.as_ref().unwrap().display()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_type_parsing() {
        assert_eq!(AiType::from_str("easy").unwrap(), AiType::Easy);
        assert_eq!(AiType::from_str("legacy").unwrap(), AiType::Easy);
        assert_eq!(AiType::from_str("normal").unwrap(), AiType::Normal);
        assert_eq!(AiType::from_str("default").unwrap(), AiType::Normal);
        assert_eq!(AiType::from_str("hard").unwrap(), AiType::Hard);
        assert_eq!(AiType::from_str("future").unwrap(), AiType::Hard);
        assert_eq!(AiType::from_str("embedded").unwrap(), AiType::Embedded);
        assert_eq!(AiType::from_str("ml").unwrap(), AiType::Embedded);

        // Case insensitive
        assert_eq!(AiType::from_str("EASY").unwrap(), AiType::Easy);
        assert_eq!(AiType::from_str("Embedded").unwrap(), AiType::Embedded);

        // Invalid
        assert!(matches!(
            AiType::from_str("invalid"),
            Err(CliError::InvalidAiType(_))
        ));
    }

    #[test]
    fn self_play_collects_non_zero_rewards() {
        use tempfile::NamedTempFile;

        let temp_shaped = NamedTempFile::new().unwrap();
        run_self_play_eval(
            1,
            None,
            Some(temp_shaped.path().to_path_buf()),
            crate::rl::StepRewardMode::Shaped,
        )
        .unwrap();

        let shaped_data = std::fs::read_to_string(temp_shaped.path()).unwrap();
        let shaped_has_reward = shaped_data.lines().any(|line| {
            let exp: crate::rl::RLExperience = serde_json::from_str(line).unwrap();
            exp.reward.abs() > 1e-6
        });
        assert!(
            shaped_has_reward,
            "shaped mode should emit at least one non-zero reward"
        );

        let temp_per_trick = NamedTempFile::new().unwrap();
        run_self_play_eval(
            1,
            None,
            Some(temp_per_trick.path().to_path_buf()),
            crate::rl::StepRewardMode::PerTrick,
        )
        .unwrap();

        let per_trick_data = std::fs::read_to_string(temp_per_trick.path()).unwrap();
        let per_trick_has_reward = per_trick_data.lines().any(|line| {
            let exp: crate::rl::RLExperience = serde_json::from_str(line).unwrap();
            exp.reward.abs() > 1e-6
        });
        assert!(
            per_trick_has_reward,
            "per_trick mode should emit at least one non-zero reward"
        );
    }

    #[test]
    #[ignore] // Slow test: runs full game
    fn eval_runs_without_panic() {
        let result = run_eval(1, AiType::Normal, None, None);
        assert!(result.is_ok());
    }

    #[test]
    #[ignore] // Slow test: runs full game with ML inference
    fn eval_embedded_ai_runs() {
        let result = run_eval(1, AiType::Embedded, None, None);
        assert!(result.is_ok());
    }
}
