use hearts_core::game::match_state::MatchState;
use hearts_core::game::serialization::MatchSnapshot;
use hearts_core::model::player::PlayerPosition;
use std::fs;
use std::path::PathBuf;
#[cfg(windows)]
use std::sync::OnceLock;
#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MESSAGEBOX_STYLE, MessageBoxW,
};

pub enum CliOutcome {
    Handled,
    NotHandled,
}

#[derive(Debug)]
pub enum CliError {
    UnknownCommand(String),
    MissingArgument(&'static str),
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidSeat(String),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::UnknownCommand(cmd) => write!(f, "Unknown command: {cmd}"),
            CliError::MissingArgument(arg) => write!(f, "Missing argument: {arg}"),
            CliError::Io(err) => write!(f, "I/O error: {err}"),
            CliError::Json(err) => write!(f, "JSON error: {err}"),
            CliError::InvalidSeat(value) => write!(f, "Invalid seat: {value}"),
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
        "--show-weights" => {
            let norm = crate::bot::debug_weights_string();
            let hard = crate::bot::debug_hard_weights_string();
            let msg = format!(
                "AI Weights (Normal): {}\nAI Weights (Hard):   {}",
                norm, hard
            );
            println!("{}", msg);
            show_info_box("AI Weights", &msg);
            Ok(CliOutcome::Handled)
        }
        "--explain-once" => {
            let seed = args
                .next()
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or(CliError::MissingArgument("--explain-once <seed> <seat>"))?;
            let seat = args
                .next()
                .map(|s| parse_seat(&s))
                .transpose()?
                .ok_or(CliError::MissingArgument("--explain-once <seed> <seat>"))?;
            let difficulty = args.next().and_then(|s| parse_difficulty_opt(&s));

            let mut controller =
                crate::controller::GameController::new_with_seed(Some(seed), PlayerPosition::North);
            if let Some(d) = difficulty {
                controller.set_bot_difficulty(d);
            }
            // Resolve passes if any
            if controller.in_passing_phase() {
                if let Some(cards) = controller.simple_pass_for(seat) {
                    let _ = controller.submit_pass(seat, cards);
                }
                let _ = controller.submit_auto_passes_for_others(seat);
                let _ = controller.resolve_passes();
            }
            // Autoplay until target seat turn
            while !controller.in_passing_phase() && controller.expected_to_play() != seat {
                if controller.autoplay_one(seat).is_none() {
                    break;
                }
            }
            let legal = controller.legal_moves(seat);
            if legal.is_empty() {
                println!("No legal moves for {:?}", seat);
                return Ok(CliOutcome::Handled);
            }
            let explained = controller.explain_candidates_for(seat);
            println!("Explain {:?} (seed {}):", seat, seed);
            println!(
                "  {} candidates (difficulty={:?})",
                explained.len(),
                controller.bot_difficulty()
            );
            // Show Hard planner stats if available
            if matches!(
                controller.bot_difficulty(),
                crate::bot::BotDifficulty::FutureHard
            ) {
                if let Some(stats) = crate::bot::search::last_stats() {
                    println!(
                        "  hard-stats: scanned={} elapsed={}ms",
                        stats.scanned, stats.elapsed_ms
                    );
                }
            }
            for (card, score) in explained.iter() {
                println!("  {} => {}", card, score);
            }
            // Optional verbose breakdown for Hard when debug logs are enabled
            if matches!(
                controller.bot_difficulty(),
                crate::bot::BotDifficulty::FutureHard
            ) && debug_logs_enabled()
            {
                let legal = controller.legal_moves(seat);
                let ctx = controller.bot_context(seat);
                let verbose = crate::bot::PlayPlannerHard::explain_candidates_verbose(&legal, &ctx);
                println!("  hard-verbose (card base cont total):");
                for (c, b, cont, t) in verbose {
                    println!("    {} {} {} {}", c, b, cont, t);
                }
            }
            Ok(CliOutcome::Handled)
        }
        "--explain-batch" => {
            let seat = args.next().map(|s| parse_seat(&s)).transpose()?.ok_or(
                CliError::MissingArgument("--explain-batch <seat> <seed_start> <count>"),
            )?;
            let seed_start = args.next().and_then(|s| s.parse::<u64>().ok()).ok_or(
                CliError::MissingArgument("--explain-batch <seat> <seed_start> <count>"),
            )?;
            let count = args.next().and_then(|s| s.parse::<u64>().ok()).ok_or(
                CliError::MissingArgument("--explain-batch <seat> <seed_start> <count>"),
            )?;
            let difficulty = args.next().and_then(|s| parse_difficulty_opt(&s));

            for i in 0..count {
                let seed = seed_start + i;
                let mut controller = crate::controller::GameController::new_with_seed(
                    Some(seed),
                    PlayerPosition::North,
                );
                if let Some(d) = difficulty {
                    controller.set_bot_difficulty(d);
                }
                if controller.in_passing_phase() {
                    if let Some(cards) = controller.simple_pass_for(seat) {
                        let _ = controller.submit_pass(seat, cards);
                    }
                    let _ = controller.submit_auto_passes_for_others(seat);
                    let _ = controller.resolve_passes();
                }
                while !controller.in_passing_phase() && controller.expected_to_play() != seat {
                    if controller.autoplay_one(seat).is_none() {
                        break;
                    }
                }
                let explained = controller.explain_candidates_for(seat);
                println!("Explain {:?} (seed {}):", seat, seed);
                println!(
                    "  {} candidates (difficulty={:?})",
                    explained.len(),
                    controller.bot_difficulty()
                );
                if matches!(
                    controller.bot_difficulty(),
                    crate::bot::BotDifficulty::FutureHard
                ) {
                    if let Some(stats) = crate::bot::search::last_stats() {
                        println!(
                            "  hard-stats: scanned={} elapsed={}ms",
                            stats.scanned, stats.elapsed_ms
                        );
                    }
                }
                for (card, score) in explained.iter() {
                    println!("  {} => {}", card, score);
                }
                if matches!(
                    controller.bot_difficulty(),
                    crate::bot::BotDifficulty::FutureHard
                ) && debug_logs_enabled()
                {
                    let legal = controller.legal_moves(seat);
                    let ctx = controller.bot_context(seat);
                    let verbose =
                        crate::bot::PlayPlannerHard::explain_candidates_verbose(&legal, &ctx);
                    println!("  hard-verbose (card base cont total):");
                    for (c, b, cont, t) in verbose {
                        println!("    {} {} {} {}", c, b, cont, t);
                    }
                }
            }
            Ok(CliOutcome::Handled)
        }
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
        "--explain-snapshot" => {
            let path = args
                .next()
                .map(PathBuf::from)
                .ok_or(CliError::MissingArgument(
                    "--explain-snapshot <path> <seat>",
                ))?;
            let seat = args.next().map(|s| parse_seat(&s)).transpose()?.ok_or(
                CliError::MissingArgument("--explain-snapshot <path> <seat>"),
            )?;
            let json = fs::read_to_string(&path)?;
            let snapshot = MatchSnapshot::from_json(&json)?;
            let match_state = snapshot.restore();
            let controller = crate::controller::GameController::new_from_match_state(match_state);
            let legal = controller.legal_moves(seat);
            if legal.is_empty() {
                println!("No legal moves for {:?}", seat);
                return Ok(CliOutcome::Handled);
            }
            let explained = controller.explain_candidates_for(seat);
            println!("Explain {:?} from snapshot {}:", seat, path.display());
            println!(
                "  {} candidates (difficulty={:?})",
                explained.len(),
                controller.bot_difficulty()
            );
            if matches!(
                controller.bot_difficulty(),
                crate::bot::BotDifficulty::FutureHard
            ) {
                if let Some(stats) = crate::bot::search::last_stats() {
                    println!(
                        "  hard-stats: scanned={} elapsed={}ms",
                        stats.scanned, stats.elapsed_ms
                    );
                }
            }
            for (card, score) in explained.iter() {
                println!("  {} => {}", card, score);
            }
            if matches!(
                controller.bot_difficulty(),
                crate::bot::BotDifficulty::FutureHard
            ) && debug_logs_enabled()
            {
                let legal = controller.legal_moves(seat);
                let ctx = controller.bot_context(seat);
                let verbose = crate::bot::PlayPlannerHard::explain_candidates_verbose(&legal, &ctx);
                println!("  hard-verbose (card base cont total):");
                for (c, b, cont, t) in verbose {
                    println!("    {} {} {} {}", c, b, cont, t);
                }
            }
            Ok(CliOutcome::Handled)
        }
        "--explain-pass-once" => {
            let seed = args.next().and_then(|s| s.parse::<u64>().ok()).ok_or(
                CliError::MissingArgument("--explain-pass-once <seed> <seat>"),
            )?;
            let seat = args.next().map(|s| parse_seat(&s)).transpose()?.ok_or(
                CliError::MissingArgument("--explain-pass-once <seed> <seat>"),
            )?;

            let controller =
                crate::controller::GameController::new_with_seed(Some(seed), PlayerPosition::North);
            if !controller.in_passing_phase() {
                println!("Round is not in passing phase for seed {}.", seed);
                return Ok(CliOutcome::Handled);
            }
            let hand_vec: Vec<_> = controller.hand(seat);
            let ctx = controller.bot_context(seat);
            if let Some(picks) = crate::bot::PassPlanner::choose(
                &hearts_core::model::hand::Hand::with_cards(hand_vec.clone()),
                &ctx,
            ) {
                println!("Explain-pass {:?} (seed {}):", seat, seed);
                println!(
                    "  Hand: {}",
                    hand_vec
                        .iter()
                        .map(|c| c.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                println!("  Picks: {}, {}, {}", picks[0], picks[1], picks[2]);
            } else {
                println!("Not enough cards to pass for {:?}", seat);
            }
            Ok(CliOutcome::Handled)
        }
        "--explain-pass-batch" => {
            let seat = args.next().map(|s| parse_seat(&s)).transpose()?.ok_or(
                CliError::MissingArgument("--explain-pass-batch <seat> <seed_start> <count>"),
            )?;
            let seed_start = args.next().and_then(|s| s.parse::<u64>().ok()).ok_or(
                CliError::MissingArgument("--explain-pass-batch <seat> <seed_start> <count>"),
            )?;
            let count = args.next().and_then(|s| s.parse::<u64>().ok()).ok_or(
                CliError::MissingArgument("--explain-pass-batch <seat> <seed_start> <count>"),
            )?;

            for i in 0..count {
                let seed = seed_start + i;
                let controller = crate::controller::GameController::new_with_seed(
                    Some(seed),
                    PlayerPosition::North,
                );
                if !controller.in_passing_phase() {
                    println!("[seed {}] Not in passing phase", seed);
                    continue;
                }
                let hand_vec: Vec<_> = controller.hand(seat);
                let ctx = controller.bot_context(seat);
                if let Some(picks) = crate::bot::PassPlanner::choose(
                    &hearts_core::model::hand::Hand::with_cards(hand_vec.clone()),
                    &ctx,
                ) {
                    println!("Explain-pass {:?} (seed {}):", seat, seed);
                    println!(
                        "  Hand: {}",
                        hand_vec
                            .iter()
                            .map(|c| c.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    println!("  Picks: {}, {}, {}", picks[0], picks[1], picks[2]);
                } else {
                    println!("[seed {}] Not enough cards to pass for {:?}", seed, seat);
                }
            }
            Ok(CliOutcome::Handled)
        }
        "--compare-once" => {
            let seed = args
                .next()
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or(CliError::MissingArgument("--compare-once <seed> <seat>"))?;
            let seat = args
                .next()
                .map(|s| parse_seat(&s))
                .transpose()?
                .ok_or(CliError::MissingArgument("--compare-once <seed> <seat>"))?;

            // Normal
            let mut normal =
                crate::controller::GameController::new_with_seed(Some(seed), PlayerPosition::North);
            normal.set_bot_difficulty(crate::bot::BotDifficulty::NormalHeuristic);
            if normal.in_passing_phase() {
                if let Some(cards) = normal.simple_pass_for(seat) {
                    let _ = normal.submit_pass(seat, cards);
                }
                let _ = normal.submit_auto_passes_for_others(seat);
                let _ = normal.resolve_passes();
            }
            while !normal.in_passing_phase() && normal.expected_to_play() != seat {
                if normal.autoplay_one(seat).is_none() {
                    break;
                }
            }
            let normal_expl = normal.explain_candidates_for(seat);
            let normal_top = normal_expl.iter().max_by_key(|(_, s)| *s).map(|(c, _)| *c);

            // Hard
            let mut hard =
                crate::controller::GameController::new_with_seed(Some(seed), PlayerPosition::North);
            hard.set_bot_difficulty(crate::bot::BotDifficulty::FutureHard);
            if hard.in_passing_phase() {
                if let Some(cards) = hard.simple_pass_for(seat) {
                    let _ = hard.submit_pass(seat, cards);
                }
                let _ = hard.submit_auto_passes_for_others(seat);
                let _ = hard.resolve_passes();
            }
            while !hard.in_passing_phase() && hard.expected_to_play() != seat {
                if hard.autoplay_one(seat).is_none() {
                    break;
                }
            }
            let hard_expl = hard.explain_candidates_for(seat);
            let hard_top = hard_expl.iter().max_by_key(|(_, s)| *s).map(|(c, _)| *c);

            println!("Compare {:?} (seed {}):", seat, seed);
            println!(
                "  Normal: top={:?} candidates={}",
                normal_top,
                normal_expl.len()
            );
            println!(
                "  Hard:   top={:?} candidates={}",
                hard_top,
                hard_expl.len()
            );
            if let Some(stats) = crate::bot::search::last_stats() {
                println!(
                    "  Hard stats: scanned={} elapsed={}ms",
                    stats.scanned, stats.elapsed_ms
                );
            }
            if debug_logs_enabled() {
                let legal = hard.legal_moves(seat);
                let ctx = hard.bot_context(seat);
                let verbose = crate::bot::PlayPlannerHard::explain_candidates_verbose(&legal, &ctx);
                println!("  hard-verbose (card base cont total):");
                for (c, b, cont, t) in verbose {
                    println!("    {} {} {} {}", c, b, cont, t);
                }
            }
            Ok(CliOutcome::Handled)
        }
        "--compare-batch" => {
            let seat = args.next().map(|s| parse_seat(&s)).transpose()?.ok_or(
                CliError::MissingArgument("--compare-batch <seat> <seed_start> <count>"),
            )?;
            let seed_start = args.next().and_then(|s| s.parse::<u64>().ok()).ok_or(
                CliError::MissingArgument("--compare-batch <seat> <seed_start> <count>"),
            )?;
            let count = args.next().and_then(|s| s.parse::<u64>().ok()).ok_or(
                CliError::MissingArgument("--compare-batch <seat> <seed_start> <count>"),
            )?;

            // Optional output path: --out <path>
            let mut out_path: Option<PathBuf> = None;
            let mut only_disagree = false;
            // Parse optional flags: --out <path>, --only-disagree (order-agnostic)
            while let Some(flag) = args.next() {
                match flag.as_str() {
                    "--out" => {
                        let path = args
                            .next()
                            .map(PathBuf::from)
                            .ok_or(CliError::MissingArgument(
                                "--compare-batch <seat> <seed_start> <count> --out <path>",
                            ))?;
                        out_path = Some(path);
                    }
                    "--only-disagree" => only_disagree = true,
                    other => return Err(CliError::UnknownCommand(other.to_string())),
                }
            }

            let mut buffer = String::new();
            buffer.push_str("seed,seat,normal_top,hard_top,agree,hard_scanned,hard_elapsed_ms\n");
            for i in 0..count {
                let seed = seed_start + i;
                // Normal
                let mut normal = crate::controller::GameController::new_with_seed(
                    Some(seed),
                    PlayerPosition::North,
                );
                normal.set_bot_difficulty(crate::bot::BotDifficulty::NormalHeuristic);
                if normal.in_passing_phase() {
                    if let Some(cards) = normal.simple_pass_for(seat) {
                        let _ = normal.submit_pass(seat, cards);
                    }
                    let _ = normal.submit_auto_passes_for_others(seat);
                    let _ = normal.resolve_passes();
                }
                while !normal.in_passing_phase() && normal.expected_to_play() != seat {
                    if normal.autoplay_one(seat).is_none() {
                        break;
                    }
                }
                let normal_expl = normal.explain_candidates_for(seat);
                let normal_top = normal_expl
                    .iter()
                    .max_by_key(|(_, s)| *s)
                    .map(|(c, _)| c.to_string())
                    .unwrap_or_else(|| "(none)".to_string());

                // Hard
                let mut hard = crate::controller::GameController::new_with_seed(
                    Some(seed),
                    PlayerPosition::North,
                );
                hard.set_bot_difficulty(crate::bot::BotDifficulty::FutureHard);
                if hard.in_passing_phase() {
                    if let Some(cards) = hard.simple_pass_for(seat) {
                        let _ = hard.submit_pass(seat, cards);
                    }
                    let _ = hard.submit_auto_passes_for_others(seat);
                    let _ = hard.resolve_passes();
                }
                while !hard.in_passing_phase() && hard.expected_to_play() != seat {
                    if hard.autoplay_one(seat).is_none() {
                        break;
                    }
                }
                let hard_expl = hard.explain_candidates_for(seat);
                let hard_top_card = hard_expl.iter().max_by_key(|(_, s)| *s).map(|(c, _)| *c);
                let hard_top = hard_top_card
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "(none)".to_string());
                let agree = hard_top == normal_top;
                let stats = crate::bot::search::last_stats();
                let (scanned, elapsed) = stats
                    .map(|s| (s.scanned, s.elapsed_ms))
                    .unwrap_or((0usize, 0u32));
                if !only_disagree || !agree {
                    use std::fmt::Write as _;
                    let _ = write!(
                        &mut buffer,
                        "{}, {:?}, {}, {}, {}, {}, {}\n",
                        seed, seat, normal_top, hard_top, agree, scanned, elapsed
                    );
                }
            }
            if let Some(path) = out_path {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&path, &buffer)?;
                println!("Wrote compare CSV to {}", path.display());
            } else {
                print!("{}", buffer);
            }
            Ok(CliOutcome::Handled)
        }
        "--explain-json" => {
            let seed = args.next().and_then(|s| s.parse::<u64>().ok()).ok_or(
                CliError::MissingArgument("--explain-json <seed> <seat> <path> [difficulty]"),
            )?;
            let seat = args.next().map(|s| parse_seat(&s)).transpose()?.ok_or(
                CliError::MissingArgument("--explain-json <seed> <seat> <path> [difficulty]"),
            )?;
            let path = args
                .next()
                .map(PathBuf::from)
                .ok_or(CliError::MissingArgument(
                    "--explain-json <seed> <seat> <path> [difficulty]",
                ))?;
            let difficulty = args.next().and_then(|s| parse_difficulty_opt(&s));

            let mut controller =
                crate::controller::GameController::new_with_seed(Some(seed), PlayerPosition::North);
            if let Some(d) = difficulty {
                controller.set_bot_difficulty(d);
            }
            if controller.in_passing_phase() {
                if let Some(cards) = controller.simple_pass_for(seat) {
                    let _ = controller.submit_pass(seat, cards);
                }
                let _ = controller.submit_auto_passes_for_others(seat);
                let _ = controller.resolve_passes();
            }
            while !controller.in_passing_phase() && controller.expected_to_play() != seat {
                if controller.autoplay_one(seat).is_none() {
                    break;
                }
            }
            let explained = controller.explain_candidates_for(seat);
            let diff = format!("{:?}", controller.bot_difficulty());
            let mut stats_obj = serde_json::Value::Null;
            if matches!(
                controller.bot_difficulty(),
                crate::bot::BotDifficulty::FutureHard
            ) {
                if let Some(stats) = crate::bot::search::last_stats() {
                    stats_obj = serde_json::json!({
                        "scanned": stats.scanned,
                        "elapsed_ms": stats.elapsed_ms
                    });
                }
            }
            let weights = match controller.bot_difficulty() {
                crate::bot::BotDifficulty::FutureHard => serde_json::json!({
                    "normal": crate::bot::debug_weights_string(),
                    "hard": crate::bot::debug_hard_weights_string(),
                }),
                _ => serde_json::json!({
                    "normal": crate::bot::debug_weights_string(),
                }),
            };
            let verbose = if matches!(
                controller.bot_difficulty(),
                crate::bot::BotDifficulty::FutureHard
            ) {
                let legal = controller.legal_moves(seat);
                let ctx = controller.bot_context(seat);
                let v = crate::bot::PlayPlannerHard::explain_candidates_verbose(&legal, &ctx);
                serde_json::json!(
                    v.iter()
                        .map(|(c, base, cont, total)| serde_json::json!({
                            "card": c.to_string(),
                            "base": base,
                            "cont": cont,
                            "total": total
                        }))
                        .collect::<Vec<_>>()
                )
            } else {
                serde_json::Value::Null
            };
            let json = serde_json::json!({
                "seed": seed,
                "seat": format!("{:?}", seat),
                "difficulty": diff,
                "candidates": explained.iter().map(|(c,s)| serde_json::json!({"card": c.to_string(), "score": s})).collect::<Vec<_>>(),
                "hard_stats": stats_obj,
                "weights": weights,
                "candidates_verbose": verbose,
            });
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(&path, serde_json::to_string_pretty(&json).unwrap())?;
            println!("Wrote explain JSON to {}", path.display());
            Ok(CliOutcome::Handled)
        }
        "--help" | "-h" => {
            let help = "Available commands:\n  --export-snapshot <path> [seed] [seat]\n  --import-snapshot <path>\n  --show-weights\n  --explain-once <seed> <seat> [difficulty]\n  --explain-batch <seat> <seed_start> <count> [difficulty]\n  --explain-snapshot <path> <seat>\n  --explain-pass-once <seed> <seat>\n  --explain-pass-batch <seat> <seed_start> <count>\n  --compare-once <seed> <seat>\n  --compare-batch <seat> <seed_start> <count> [--out <path>]\n  --explain-json <seed> <seat> <path> [difficulty]\n  --help";
            println!("{help}");
            show_info_box("mdhearts CLI", help);
            Ok(CliOutcome::Handled)
        }
        other => Err(CliError::UnknownCommand(other.to_string())),
    }
}

fn debug_logs_enabled() -> bool {
    std::env::var("MDH_DEBUG_LOGS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
        .unwrap_or(false)
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

fn parse_difficulty_opt(input: &str) -> Option<crate::bot::BotDifficulty> {
    match input.to_ascii_lowercase().as_str() {
        "easy" | "legacy" => Some(crate::bot::BotDifficulty::EasyLegacy),
        "normal" | "default" => Some(crate::bot::BotDifficulty::NormalHeuristic),
        "hard" | "future" => Some(crate::bot::BotDifficulty::FutureHard),
        _ => None,
    }
}

#[cfg(windows)]
pub fn show_error_box(message: &str) {
    if !popups_enabled() {
        eprintln!("{message}");
        println!("mdhearts CLI: {}", message);
        return;
    }
    show_box("mdhearts CLI", message, MB_ICONERROR | MB_OK);
}

#[cfg(not(windows))]
pub fn show_error_box(message: &str) {
    eprintln!("{message}");
    println!("mdhearts CLI: {}", message);
}

#[cfg(windows)]
fn show_info_box(title: &str, message: &str) {
    if !popups_enabled() {
        println!("{}: {}", title, message);
        return;
    }
    show_box(title, message, MB_ICONINFORMATION | MB_OK);
}

#[cfg(not(windows))]
fn show_info_box(title: &str, message: &str) {
    println!("{}: {}", title, message);
}

#[cfg(windows)]
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

#[cfg(windows)]
fn popups_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("MDH_CLI_POPUPS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
            .unwrap_or(false)
    })
}

#[cfg(windows)]
fn encode_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
