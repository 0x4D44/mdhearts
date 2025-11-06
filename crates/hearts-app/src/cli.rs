use crate::endgame_export::EndgameExport;
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
    InvalidValue { flag: &'static str, value: String },
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::UnknownCommand(cmd) => write!(f, "Unknown command: {cmd}"),
            CliError::MissingArgument(arg) => write!(f, "Missing argument: {arg}"),
            CliError::Io(err) => write!(f, "I/O error: {err}"),
            CliError::Json(err) => write!(f, "JSON error: {err}"),
            CliError::InvalidSeat(value) => write!(f, "Invalid seat: {value}"),
            CliError::InvalidValue { flag, value } => {
                write!(f, "Invalid value for {flag}: {value}")
            }
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
        "--show-hard-telemetry" => {
            let mut out_path: Option<PathBuf> = None;
            while let Some(arg) = args.next() {
                if arg == "--out" {
                    let path = args
                        .next()
                        .map(PathBuf::from)
                        .ok_or(CliError::MissingArgument(
                            "--show-hard-telemetry --out <path>",
                        ))?;
                    out_path = Some(path);
                } else {
                    return Err(CliError::UnknownCommand(arg));
                }
            }
            let (path, summary) = crate::telemetry::hard::export(out_path)?;
            println!("Telemetry written to {}", path.display());
            println!(
                "Records: {} | avg entropy {:.4} | cache hit rate {:.2}%",
                summary.record_count,
                summary.avg_entropy,
                summary.cache_hit_rate * 100.0
            );
            Ok(CliOutcome::Handled)
        }
        "--export-play-dataset" => {
            let seat = args.next().map(|s| parse_seat(&s)).transpose()?.ok_or(
                CliError::MissingArgument(
                    "--export-play-dataset <seat> <seed_start> <count> <difficulty> <out>",
                ),
            )?;
            let seed_start = args.next().and_then(|s| s.parse::<u64>().ok()).ok_or(
                CliError::MissingArgument(
                    "--export-play-dataset <seat> <seed_start> <count> <difficulty> <out>",
                ),
            )?;
            let count = args.next().and_then(|s| s.parse::<u64>().ok()).ok_or(
                CliError::MissingArgument(
                    "--export-play-dataset <seat> <seed_start> <count> <difficulty> <out>",
                ),
            )?;
            let difficulty = args.next().and_then(|s| parse_difficulty_opt(&s)).ok_or(
                CliError::MissingArgument(
                    "--export-play-dataset <seat> <seed_start> <count> <difficulty> <out>",
                ),
            )?;
            let out = args
                .next()
                .map(PathBuf::from)
                .ok_or(CliError::MissingArgument(
                    "--export-play-dataset <seat> <seed_start> <count> <difficulty> <out>",
                ))?;
            parse_hard_cli_flags(&mut args)?;
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut file = std::fs::File::create(&out)?;
            let mut written = 0usize;
            for offset in 0..count {
                let seed = seed_start + offset;
                let mut controller = crate::controller::GameController::new_with_seed(
                    Some(seed),
                    PlayerPosition::North,
                );
                controller.set_bot_difficulty(difficulty);
                if controller.in_passing_phase() {
                    if let Some(cards) = controller.simple_pass_for(seat) {
                        let _ = controller.submit_pass(seat, cards);
                    }
                    let _ = controller.submit_auto_passes_for_others(seat);
                    let _ = controller.resolve_passes();
                }
                let mut guard = 0u32;
                while controller.expected_to_play() != seat {
                    if controller.autoplay_one(seat).is_none() {
                        break;
                    }
                    guard += 1;
                    if guard > 600 {
                        break;
                    }
                }
                if let Some(sample) = crate::dataset::collect_play_sample(&controller, seat, seed) {
                    let line = serde_json::to_string(&sample)?;
                    use std::io::Write as _;
                    writeln!(file, "{}", line)?;
                    written += 1;
                }
            }
            println!("Wrote {} play samples to {}", written, out.display());
            Ok(CliOutcome::Handled)
        }
        "--export-endgame" => {
            let seed = args.next().and_then(|s| s.parse::<u64>().ok()).ok_or(
                CliError::MissingArgument("--export-endgame <seed> <seat> <out>"),
            )?;
            let seat = args.next().map(|s| parse_seat(&s)).transpose()?.ok_or(
                CliError::MissingArgument("--export-endgame <seed> <seat> <out>"),
            )?;
            let out = args
                .next()
                .map(PathBuf::from)
                .ok_or(CliError::MissingArgument(
                    "--export-endgame <seed> <seat> <out>",
                ))?;

            let mut controller =
                crate::controller::GameController::new_with_seed(Some(seed), PlayerPosition::North);
            if controller.in_passing_phase() {
                if let Some(cards) = controller.simple_pass_for(seat) {
                    let _ = controller.submit_pass(seat, cards);
                }
                let _ = controller.submit_auto_passes_for_others(seat);
                let _ = controller.resolve_passes();
            }
            let mut guard = 0u32;
            loop {
                if guard > 600 {
                    break;
                }
                guard += 1;
                let to_play = controller.expected_to_play();
                let round = controller.bot_context(seat).round;
                let mut ok_small = true;
                for s in [
                    PlayerPosition::North,
                    PlayerPosition::East,
                    PlayerPosition::South,
                    PlayerPosition::West,
                ] {
                    if round.hand(s).len() > 3 {
                        ok_small = false;
                        break;
                    }
                }
                if ok_small && to_play == seat {
                    break;
                }
                let _ = controller.autoplay_one(to_play.next());
            }
            let export = EndgameExport::capture(&controller, seat, Some(seed));
            if let Some(parent) = out.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let json = serde_json::to_string_pretty(&export)?;
            std::fs::write(&out, json)?;
            println!("Wrote endgame snapshot to {}", out.display());
            Ok(CliOutcome::Handled)
        }
        "--compare-dp-once" => {
            // Usage: --compare-dp-once <seed> <seat> [Hard flags]
            let seed = args
                .next()
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or(CliError::MissingArgument("--compare-dp-once <seed> <seat>"))?;
            let seat = args
                .next()
                .map(|s| parse_seat(&s))
                .transpose()?
                .ok_or(CliError::MissingArgument("--compare-dp-once <seed> <seat>"))?;
            // Collect optional hard flags
            let mut tail_tokens: Vec<String> = Vec::new();
            while let Some(flag) = args.next() {
                if is_shared_cli_flag(flag.as_str()) {
                    let needs = shared_flag_needs_value(flag.as_str());
                    tail_tokens.push(flag);
                    if needs {
                        let v = args
                            .next()
                            .ok_or(CliError::MissingArgument("shared flag value"))?;
                        tail_tokens.push(v);
                    }
                } else {
                    return Err(CliError::UnknownCommand(flag));
                }
            }
            parse_hard_cli_flags(&mut tail_tokens.into_iter())?;

            if let Some((legal_count, off, on)) = seek_dp_flip_for(seed, seat)? {
                println!("dp-legal:{}", legal_count);
                println!("dp-off:{}", off.unwrap_or_else(|| "(none)".to_string()));
                println!("dp-on:{}", on.unwrap_or_else(|| "(none)".to_string()));
            } else {
                println!("dp-no-position");
            }
            Ok(CliOutcome::Handled)
        }
        "--seek-dp-flip" => {
            // Usage: --seek-dp-flip <seat> <seed_start> <count> [--out <path>] [Hard flags]
            let seat = args.next().map(|s| parse_seat(&s)).transpose()?.ok_or(
                CliError::MissingArgument("--seek-dp-flip <seat> <seed_start> <count>"),
            )?;
            let seed_start = args.next().and_then(|s| s.parse::<u64>().ok()).ok_or(
                CliError::MissingArgument("--seek-dp-flip <seat> <seed_start> <count>"),
            )?;
            let count = args.next().and_then(|s| s.parse::<u64>().ok()).ok_or(
                CliError::MissingArgument("--seek-dp-flip <seat> <seed_start> <count>"),
            )?;
            // Optional flags: --out <path> then Hard flags
            let mut out_path: Option<std::path::PathBuf> = None;
            let mut tail_tokens: Vec<String> = Vec::new();
            while let Some(flag) = args.next() {
                match flag.as_str() {
                    "--out" => {
                        let p = args
                            .next()
                            .ok_or(CliError::MissingArgument("--out <path>"))?;
                        out_path = Some(std::path::PathBuf::from(p));
                    }
                    other if is_shared_cli_flag(other) => {
                        tail_tokens.push(other.to_string());
                        if shared_flag_needs_value(other) {
                            let v = args
                                .next()
                                .ok_or(CliError::MissingArgument("shared flag value"))?;
                            tail_tokens.push(v);
                        }
                    }
                    other => return Err(CliError::UnknownCommand(other.to_string())),
                }
            }
            parse_hard_cli_flags(&mut tail_tokens.into_iter())?;

            let mut rows = Vec::new();
            rows.push("seed,seat,legal,top_off,top_on".to_string());
            for i in 0..count {
                let seed = seed_start + i;
                if let Some((legal_count, off, on)) = seek_dp_flip_for(seed, seat)? {
                    rows.push(format!(
                        "{}, {:?}, {}, {}, {}",
                        seed,
                        seat,
                        legal_count,
                        off.unwrap_or_else(|| "(none)".to_string()),
                        on.unwrap_or_else(|| "(none)".to_string())
                    ));
                }
            }
            if let Some(path) = out_path {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&path, rows.join("\n")).map_err(CliError::Io)?;
                println!("Wrote DP flip seek CSV to {}", path.display());
            } else {
                for line in rows {
                    println!("{}", line);
                }
            }
            Ok(CliOutcome::Handled)
        }
        "--show-weights" => {
            // Optional: --out <path>
            let mut out: Option<std::path::PathBuf> = None;
            if let Some(flag) = args.next() {
                if flag == "--out" {
                    let p = args
                        .next()
                        .ok_or(CliError::MissingArgument("--show-weights --out <path>"))?;
                    out = Some(std::path::PathBuf::from(p));
                } else {
                    return Err(CliError::UnknownCommand(flag));
                }
            }
            let norm = crate::bot::debug_weights_string();
            let hard = crate::bot::debug_hard_weights_string();
            let msg = format!(
                "AI Weights (Normal): {}\nAI Weights (Hard):   {}",
                norm, hard
            );
            if let Some(path) = out {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&path, &msg).map_err(CliError::Io)?;
                println!("Wrote weights to {}", path.display());
            } else {
                println!("{}", msg);
                show_info_box("AI Weights", &msg);
            }
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
            parse_hard_cli_flags(&mut args)?;

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
                if let Some(flags) = hard_flags_summary() {
                    println!("  hard-flags: {}", flags);
                }
                if let Some(stats) = crate::bot::search::last_stats() {
                    println!(
                        "  hard-stats: scanned={} elapsed={}ms",
                        stats.scanned, stats.elapsed_ms
                    );
                    if debug_logs_enabled() {
                        println!(
                            "    tier={:?} leverage={} util={}% limits: topk={} nextM={} ab={}",
                            stats.tier,
                            stats.leverage_score,
                            stats.utilization,
                            stats.limits_in_effect.phaseb_topk,
                            stats.limits_in_effect.next_probe_m,
                            stats.limits_in_effect.ab_margin,
                        );
                        println!(
                            "    cont_cap={} wide_boost_feed_permil={} wide_boost_self_permil={} next3_tiny_hits={} endgame_dp_hits={} planner_nudge_hits={}",
                            stats.cont_cap,
                            stats.wide_boost_feed_permil,
                            stats.wide_boost_self_permil,
                            stats.next3_tiny_hits,
                            stats.endgame_dp_hits,
                            stats.planner_nudge_hits
                        );
                        if let Some(ref summary) = stats.planner_nudge_trace {
                            if !summary.is_empty() {
                                println!(
                                    "    planner_nudge_guard={}",
                                    format_nudge_trace_summary(Some(summary))
                                );
                            }
                        }
                    }
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
                if std::env::var("MDH_HARD_VERBOSE_CONT")
                    .map(|v| {
                        v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on")
                    })
                    .unwrap_or(false)
                {
                    let parts =
                        crate::bot::PlayPlannerHard::explain_candidates_verbose_parts(&legal, &ctx);
                    println!("  hard-verbose-parts:");
                    for (c, b, p, t) in parts {
                        let cont_sum = p.feed
                            + p.self_capture
                            + p.next_start
                            + p.next_probe
                            + p.qs_risk
                            + p.ctrl_hearts
                            + p.ctrl_handoff
                            + p.moon_relief
                            + p.capped_delta;
                        println!(
                            "    {} {} {} {} | feed={} self={} start={} probe={} qs={} hearts={} handoff={} moon_relief={} cap={}",
                            c,
                            b,
                            cont_sum,
                            t,
                            p.feed,
                            p.self_capture,
                            p.next_start,
                            p.next_probe,
                            p.qs_risk,
                            p.ctrl_hearts,
                            p.ctrl_handoff,
                            p.moon_relief,
                            p.capped_delta
                        );
                    }
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
            parse_hard_cli_flags(&mut args)?;

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
                    if let Some(flags) = hard_flags_summary() {
                        println!("  hard-flags: {}", flags);
                    }
                    if let Some(stats) = crate::bot::search::last_stats() {
                        println!(
                            "  hard-stats: scanned={} elapsed={}ms",
                            stats.scanned, stats.elapsed_ms
                        );
                        if debug_logs_enabled() {
                            println!(
                                "    tier={:?} leverage={} util={}% limits: topk={} nextM={} ab={}",
                                stats.tier,
                                stats.leverage_score,
                                stats.utilization,
                                stats.limits_in_effect.phaseb_topk,
                                stats.limits_in_effect.next_probe_m,
                                stats.limits_in_effect.ab_margin,
                            );
                            println!(
                                "    cont_cap={} wide_boost_feed_permil={} wide_boost_self_permil={} endgame_dp_hits={} planner_nudge_hits={}",
                                stats.cont_cap,
                                stats.wide_boost_feed_permil,
                                stats.wide_boost_self_permil,
                                stats.endgame_dp_hits,
                                stats.planner_nudge_hits
                            );
                            if let Some(ref summary) = stats.planner_nudge_trace {
                                if !summary.is_empty() {
                                    println!(
                                        "    planner_nudge_guard={}",
                                        format_nudge_trace_summary(Some(summary))
                                    );
                                }
                            }
                        }
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
                    if std::env::var("MDH_HARD_VERBOSE_CONT")
                        .map(|v| {
                            v == "1"
                                || v.eq_ignore_ascii_case("true")
                                || v.eq_ignore_ascii_case("on")
                        })
                        .unwrap_or(false)
                    {
                        let parts = crate::bot::PlayPlannerHard::explain_candidates_verbose_parts(
                            &legal, &ctx,
                        );
                        println!("  hard-verbose-parts:");
                        for (c, b, p, t) in parts {
                            let cont_sum = p.feed
                                + p.self_capture
                                + p.next_start
                                + p.next_probe
                                + p.qs_risk
                                + p.ctrl_hearts
                                + p.ctrl_handoff
                                + p.moon_relief
                                + p.capped_delta;
                            println!(
                                "    {} {} {} {} | feed={} self={} start={} probe={} qs={} hearts={} handoff={} moon_relief={} cap={}",
                                c,
                                b,
                                cont_sum,
                                t,
                                p.feed,
                                p.self_capture,
                                p.next_start,
                                p.next_probe,
                                p.qs_risk,
                                p.ctrl_hearts,
                                p.ctrl_handoff,
                                p.moon_relief,
                                p.capped_delta
                            );
                        }
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
            parse_hard_cli_flags(&mut args)?;
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
                if let Some(flags) = hard_flags_summary() {
                    println!("  hard-flags: {}", flags);
                }
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
            parse_hard_cli_flags(&mut args)?;

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
            if let Some(flags) = hard_flags_summary() {
                println!("  hard-flags: {}", flags);
            }
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

            let remaining: Vec<String> = args.collect();
            let mut idx = 0usize;
            let mut diffs: Vec<crate::bot::BotDifficulty> = Vec::new();
            while idx < remaining.len()
                && diffs.len() < 2
                && !remaining[idx].starts_with("--")
                && !remaining[idx].is_empty()
            {
                let token = &remaining[idx];
                let diff = parse_difficulty_opt(token).ok_or_else(|| CliError::InvalidValue {
                    flag: "difficulty",
                    value: token.clone(),
                })?;
                diffs.push(diff);
                idx += 1;
            }

            let diff_a = diffs
                .get(0)
                .copied()
                .unwrap_or(crate::bot::BotDifficulty::NormalHeuristic);
            let diff_b = diffs
                .get(1)
                .copied()
                .unwrap_or(crate::bot::BotDifficulty::FutureHard);

            let mut out_path: Option<PathBuf> = None;
            let mut only_disagree = false;
            let mut shared_tail: Vec<String> = Vec::new();
            while idx < remaining.len() {
                let flag = &remaining[idx];
                match flag.as_str() {
                    "--out" => {
                        idx += 1;
                        let path = remaining
                            .get(idx)
                            .ok_or(CliError::MissingArgument(
                                "--compare-batch <seat> <seed_start> <count> --out <path>",
                            ))?
                            .clone();
                        out_path = Some(PathBuf::from(path));
                    }
                    "--only-disagree" => {
                        only_disagree = true;
                    }
                    other if is_shared_cli_flag(other) => {
                        shared_tail.push(other.to_string());
                        if shared_flag_needs_value(other) {
                            idx += 1;
                            let val = remaining
                                .get(idx)
                                .ok_or(CliError::MissingArgument("shared flag value"))?
                                .clone();
                            shared_tail.push(val);
                        }
                    }
                    other => return Err(CliError::UnknownCommand(other.to_string())),
                }
                idx += 1;
            }
            parse_hard_cli_flags(&mut shared_tail.into_iter())?;

            let mut buffer = String::new();
            buffer.push_str(
                "seed,seat,diffA,diffB,diffA_top,diffB_top,agree,diffB_scanned,diffB_elapsed_ms\\n",
            );
            for i in 0..count {
                let seed = seed_start + i;

                let mut ctrl_a = crate::controller::GameController::new_with_seed(
                    Some(seed),
                    PlayerPosition::North,
                );
                ctrl_a.set_bot_difficulty(diff_a);
                if ctrl_a.in_passing_phase() {
                    if let Some(cards) = ctrl_a.simple_pass_for(seat) {
                        let _ = ctrl_a.submit_pass(seat, cards);
                    }
                    let _ = ctrl_a.submit_auto_passes_for_others(seat);
                    let _ = ctrl_a.resolve_passes();
                }
                while !ctrl_a.in_passing_phase() && ctrl_a.expected_to_play() != seat {
                    if ctrl_a.autoplay_one(seat).is_none() {
                        break;
                    }
                }
                let diff_a_top = ctrl_a
                    .explain_candidates_for(seat)
                    .iter()
                    .max_by_key(|(_, s)| *s)
                    .map(|(c, _)| c.to_string())
                    .unwrap_or_else(|| "(none)".to_string());

                let mut ctrl_b = crate::controller::GameController::new_with_seed(
                    Some(seed),
                    PlayerPosition::North,
                );
                ctrl_b.set_bot_difficulty(diff_b);
                if ctrl_b.in_passing_phase() {
                    if let Some(cards) = ctrl_b.simple_pass_for(seat) {
                        let _ = ctrl_b.submit_pass(seat, cards);
                    }
                    let _ = ctrl_b.submit_auto_passes_for_others(seat);
                    let _ = ctrl_b.resolve_passes();
                }
                while !ctrl_b.in_passing_phase() && ctrl_b.expected_to_play() != seat {
                    if ctrl_b.autoplay_one(seat).is_none() {
                        break;
                    }
                }
                let diff_b_top_card = ctrl_b
                    .explain_candidates_for(seat)
                    .iter()
                    .max_by_key(|(_, s)| *s)
                    .map(|(c, _)| *c);
                let diff_b_top = diff_b_top_card
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "(none)".to_string());
                let agree = diff_a_top == diff_b_top;
                let (scanned, elapsed) = if matches!(
                    diff_b,
                    crate::bot::BotDifficulty::FutureHard
                        | crate::bot::BotDifficulty::SearchLookahead
                ) {
                    crate::bot::search::last_stats()
                        .map(|s| (s.scanned, s.elapsed_ms))
                        .unwrap_or((0usize, 0u32))
                } else {
                    (0usize, 0u32)
                };
                if !only_disagree || !agree {
                    use std::fmt::Write as _;
                    let _ = write!(
                        &mut buffer,
                        "{}, {:?}, {:?}, {:?}, {}, {}, {}, {}, {}\\n",
                        seed, seat, diff_a, diff_b, diff_a_top, diff_b_top, agree, scanned, elapsed
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
                    let mut stats_map = serde_json::Map::new();
                    stats_map.insert("scanned".into(), stats.scanned.into());
                    stats_map.insert("elapsed_ms".into(), stats.elapsed_ms.into());
                    stats_map.insert("tier".into(), format!("{:?}", stats.tier).into());
                    stats_map.insert("leverage".into(), stats.leverage_score.into());
                    stats_map.insert("utilization".into(), stats.utilization.into());
                    stats_map.insert(
                        "limits".into(),
                        serde_json::json!({
                            "topk": stats.limits_in_effect.phaseb_topk,
                            "nextM": stats.limits_in_effect.next_probe_m,
                            "ab": stats.limits_in_effect.ab_margin
                        }),
                    );
                    stats_map.insert("cont_cap".into(), stats.cont_cap.into());
                    stats_map.insert(
                        "wide_boost_feed_permil".into(),
                        stats.wide_boost_feed_permil.into(),
                    );
                    stats_map.insert(
                        "wide_boost_self_permil".into(),
                        stats.wide_boost_self_permil.into(),
                    );
                    stats_map.insert("next3_tiny_hits".into(), stats.next3_tiny_hits.into());
                    stats_map.insert("endgame_dp_hits".into(), stats.endgame_dp_hits.into());
                    stats_map.insert("planner_nudge_hits".into(), stats.planner_nudge_hits.into());
                    if let Some(summary) = stats.planner_nudge_trace.as_ref() {
                        if !summary.is_empty() {
                            let arr: Vec<_> = summary
                                .iter()
                                .map(|(reason, count)| {
                                    serde_json::json!({
                                        "reason": reason,
                                        "count": count
                                    })
                                })
                                .collect();
                            stats_map.insert("planner_nudge_guard".into(), arr.into());
                        }
                    }
                    stats_obj = serde_json::Value::Object(stats_map);
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
        "--bench-check" => {
            // Usage: --bench-check <difficulty> <seat> <seed_start> <count>
            let diff_str = args.next().ok_or(CliError::MissingArgument(
                "--bench-check <difficulty> <seat> <seed_start> <count>",
            ))?;
            let difficulty =
                parse_difficulty_opt(&diff_str).ok_or(CliError::UnknownCommand(diff_str))?;
            let seat = args.next().map(|s| parse_seat(&s)).transpose()?.ok_or(
                CliError::MissingArgument("--bench-check <difficulty> <seat> <seed_start> <count>"),
            )?;
            let seed_start = args.next().and_then(|s| s.parse::<u64>().ok()).ok_or(
                CliError::MissingArgument("--bench-check <difficulty> <seat> <seed_start> <count>"),
            )?;
            let count = args.next().and_then(|s| s.parse::<u64>().ok()).ok_or(
                CliError::MissingArgument("--bench-check <difficulty> <seat> <seed_start> <count>"),
            )?;

            // Optional Hard flags
            parse_hard_cli_flags(&mut args)?;

            let mut samples: Vec<u128> = Vec::new();
            for i in 0..count {
                let seed = seed_start + i;
                let mut controller = crate::controller::GameController::new_with_seed(
                    Some(seed),
                    PlayerPosition::North,
                );
                controller.set_bot_difficulty(difficulty);
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
                let t0 = std::time::Instant::now();
                let _ = controller.explain_candidates_for(seat);
                let dt = t0.elapsed();
                samples.push(dt.as_micros());
            }
            samples.sort();
            let n = samples.len() as u128;
            let sum: u128 = samples.iter().copied().sum();
            let avg = if n > 0 { (sum / n) as u64 } else { 0 };
            let p95 = if samples.is_empty() {
                0u64
            } else {
                let idx = ((samples.len() as f64) * 0.95).ceil() as usize - 1;
                samples[idx].min(u128::from(u64::MAX)) as u64
            };
            println!(
                "bench-check difficulty={:?} seat={:?} count={} avg_us={} p95_us={}",
                difficulty,
                seat,
                samples.len(),
                avg,
                p95
            );
            let thr = match difficulty {
                crate::bot::BotDifficulty::FutureHard => std::env::var("MDH_BENCH_WARN_US_HARD")
                    .ok()
                    .and_then(|s| s.parse::<u64>().ok()),
                _ => std::env::var("MDH_BENCH_WARN_US_HEUR")
                    .ok()
                    .and_then(|s| s.parse::<u64>().ok()),
            };
            if let Some(threshold) = thr {
                if p95 > threshold {
                    eprintln!(
                        "WARNING: p95_us={} exceeds threshold {} (difficulty={:?})",
                        p95, threshold, difficulty
                    );
                }
            }
            Ok(CliOutcome::Handled)
        }
        "--match-batch" => {
            // Usage: --match-batch <seat> <seed_start> <count> [difficultyA difficultyB] [--out <path>] [Hard flags]
            let seat = args.next().map(|s| parse_seat(&s)).transpose()?.ok_or(
                CliError::MissingArgument("--match-batch <seat> <seed_start> <count>"),
            )?;
            let seed_start = args.next().and_then(|s| s.parse::<u64>().ok()).ok_or(
                CliError::MissingArgument("--match-batch <seat> <seed_start> <count>"),
            )?;
            let count = args.next().and_then(|s| s.parse::<u64>().ok()).ok_or(
                CliError::MissingArgument("--match-batch <seat> <seed_start> <count>"),
            )?;

            // Optional difficulties (default A=normal, B=hard)
            let diff_a = args
                .next()
                .and_then(|s| parse_difficulty_opt(&s))
                .unwrap_or(crate::bot::BotDifficulty::NormalHeuristic);
            let diff_b = args
                .next()
                .and_then(|s| parse_difficulty_opt(&s))
                .unwrap_or(crate::bot::BotDifficulty::FutureHard);

            // Optional flags: --out <path>, --stats, plus Hard flags
            let mut out_path: Option<std::path::PathBuf> = None;
            let mut telemetry_out: Option<std::path::PathBuf> = None;
            let mut include_stats: bool = false;
            let mut tail_tokens: Vec<String> = Vec::new();
            while let Some(flag) = args.next() {
                match flag.as_str() {
                    "--out" => {
                        let p = args
                            .next()
                            .ok_or(CliError::MissingArgument("--out <path>"))?;
                        out_path = Some(std::path::PathBuf::from(p));
                    }
                    "--telemetry-out" => {
                        let p = args
                            .next()
                            .ok_or(CliError::MissingArgument("--telemetry-out <path>"))?;
                        telemetry_out = Some(std::path::PathBuf::from(p));
                    }
                    "--stats" => {
                        include_stats = true;
                    }
                    other if is_shared_cli_flag(other) => {
                        tail_tokens.push(other.to_string());
                        if shared_flag_needs_value(other) {
                            let v = args
                                .next()
                                .ok_or(CliError::MissingArgument("shared flag value"))?;
                            tail_tokens.push(v);
                        }
                    }
                    other => return Err(CliError::UnknownCommand(other.to_string())),
                }
            }
            parse_hard_cli_flags(&mut tail_tokens.into_iter())?;

            let mut rows = Vec::new();
            if include_stats {
                rows.push(
                    "seed,seat,diffA,diffB,a_pen,b_pen,delta,scanned,elapsed_ms,dp_hits,nudge_hits,nudge_guard"
                        .to_string(),
                );
            } else {
                rows.push("seed,seat,diffA,diffB,a_pen,b_pen,delta".to_string());
            }
            for i in 0..count {
                let seed = seed_start + i;
                let a_pen = simulate_one_round(seed, seat, diff_a)?;
                let b_pen = simulate_one_round(seed, seat, diff_b)?;
                let delta = (b_pen as i32) - (a_pen as i32);
                if include_stats {
                    let stats = crate::bot::search::last_stats();
                    let (nudges, nudge_trace) = collect_nudge_metrics(stats.as_ref());
                    let (scanned, elapsed, dp) = stats
                        .map(|s| (s.scanned, s.elapsed_ms, s.endgame_dp_hits))
                        .unwrap_or((0usize, 0u32, 0usize));
                    rows.push(format!(
                        "{}, {:?}, {:?}, {:?}, {}, {}, {}, {}, {}, {}, {}, {}",
                        seed,
                        seat,
                        diff_a,
                        diff_b,
                        a_pen,
                        b_pen,
                        delta,
                        scanned,
                        elapsed,
                        dp,
                        nudges,
                        nudge_trace
                    ));
                } else {
                    rows.push(format!(
                        "{}, {:?}, {:?}, {:?}, {}, {}, {}",
                        seed, seat, diff_a, diff_b, a_pen, b_pen, delta
                    ));
                }
            }
            if let Some(path) = out_path {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&path, rows.join("\n")).map_err(CliError::Io)?;
                println!("Wrote match CSV to {}", path.display());
            } else {
                for line in rows {
                    println!("{}", line);
                }
            }
            if let Some(path) = telemetry_out {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(CliError::Io)?;
                }
                let (path, summary) =
                    crate::telemetry::hard::export(Some(path)).map_err(CliError::Io)?;
                println!(
                    "Telemetry written to {} (records: {})",
                    path.display(),
                    summary.record_count
                );
            }
            Ok(CliOutcome::Handled)
        }
        "--match-mixed" => {
            // Usage: --match-mixed <seat> <seed_start> <count> <mix> [--out <path>] [Hard flags]
            // <mix> is a 4-char string in order N,E,S,W using: e|n|h (easy|normal|hard)
            let seat = args.next().map(|s| parse_seat(&s)).transpose()?.ok_or(
                CliError::MissingArgument("--match-mixed <seat> <seed_start> <count> <mix>"),
            )?;
            let seed_start = args.next().and_then(|s| s.parse::<u64>().ok()).ok_or(
                CliError::MissingArgument("--match-mixed <seat> <seed_start> <count> <mix>"),
            )?;
            let count = args.next().and_then(|s| s.parse::<u64>().ok()).ok_or(
                CliError::MissingArgument("--match-mixed <seat> <seed_start> <count> <mix>"),
            )?;
            let mix = args.next().ok_or(CliError::MissingArgument(
                "--match-mixed requires <mix> (e|n|h for N,E,S,W)",
            ))?;
            if mix.len() != 4 {
                return Err(CliError::UnknownCommand(
                    "--match-mixed <mix> must be 4 chars (N,E,S,W)".to_string(),
                ));
            }
            let chars: Vec<char> = mix.chars().collect();
            let map_char = |c: char| -> Option<crate::bot::BotDifficulty> {
                match c {
                    'e' | 'E' => Some(crate::bot::BotDifficulty::EasyLegacy),
                    'n' | 'N' => Some(crate::bot::BotDifficulty::NormalHeuristic),
                    'h' | 'H' => Some(crate::bot::BotDifficulty::FutureHard),
                    _ => None,
                }
            };
            let mut diffs = [crate::bot::BotDifficulty::NormalHeuristic; 4];
            for (i, c) in chars.into_iter().enumerate() {
                let d = map_char(c).ok_or(CliError::UnknownCommand(
                    "--match-mixed invalid mix char (use e|n|h)".to_string(),
                ))?;
                diffs[i] = d;
            }

            // Optional flags: --out <path>, --stats, plus Hard flags
            let mut out_path: Option<std::path::PathBuf> = None;
            let mut include_stats: bool = false;
            let mut tail_tokens: Vec<String> = Vec::new();
            while let Some(flag) = args.next() {
                match flag.as_str() {
                    "--out" => {
                        let p = args
                            .next()
                            .ok_or(CliError::MissingArgument("--out <path>"))?;
                        out_path = Some(std::path::PathBuf::from(p));
                    }
                    "--stats" => {
                        include_stats = true;
                    }
                    other if is_shared_cli_flag(other) => {
                        tail_tokens.push(other.to_string());
                        if shared_flag_needs_value(other) {
                            let v = args
                                .next()
                                .ok_or(CliError::MissingArgument("shared flag value"))?;
                            tail_tokens.push(v);
                        }
                    }
                    other => return Err(CliError::UnknownCommand(other.to_string())),
                }
            }
            parse_hard_cli_flags(&mut tail_tokens.into_iter())?;

            let mut rows = Vec::new();
            if include_stats {
                rows.push(
                    "seed,seat,mix,pen,scanned,elapsed_ms,dp_hits,nudge_hits,nudge_guard"
                        .to_string(),
                );
            } else {
                rows.push("seed,seat,mix,pen".to_string());
            }
            for i in 0..count {
                let seed = seed_start + i;
                let pen = simulate_one_round_mixed(seed, seat, diffs)?;
                if include_stats {
                    let stats = crate::bot::search::last_stats();
                    let (nudges, nudge_trace) = collect_nudge_metrics(stats.as_ref());
                    let (scanned, elapsed, dp) = stats
                        .map(|s| (s.scanned, s.elapsed_ms, s.endgame_dp_hits))
                        .unwrap_or((0usize, 0u32, 0usize));
                    rows.push(format!(
                        "{}, {:?}, {}, {}, {}, {}, {}, {}, {}",
                        seed, seat, mix, pen, scanned, elapsed, dp, nudges, nudge_trace
                    ));
                } else {
                    rows.push(format!("{}, {:?}, {}, {}", seed, seat, mix, pen));
                }
            }
            if let Some(path) = out_path {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&path, rows.join("\n")).map_err(CliError::Io)?;
                println!("Wrote mixed-match CSV to {}", path.display());
            } else {
                for line in rows {
                    println!("{}", line);
                }
            }
            Ok(CliOutcome::Handled)
        }
        "--match-mixed-file" => {
            // Usage: --match-mixed-file <seat> <mix> --seeds-file <path> [--out <path>] [Hard flags]
            let seat = args.next().map(|s| parse_seat(&s)).transpose()?.ok_or(
                CliError::MissingArgument("--match-mixed-file <seat> <mix> --seeds-file <path>"),
            )?;
            let mix = args.next().ok_or(CliError::MissingArgument(
                "--match-mixed-file requires <mix> (e|n|h for N,E,S,W)",
            ))?;
            if mix.len() != 4 {
                return Err(CliError::UnknownCommand(
                    "--match-mixed-file <mix> must be 4 chars (N,E,S,W)".to_string(),
                ));
            }

            // Optional flags: --seeds-file <path>, --out <path>, --stats, then Hard flags
            let mut seeds_path: Option<std::path::PathBuf> = None;
            let mut out_path: Option<std::path::PathBuf> = None;
            let mut include_stats: bool = false;
            let mut tail_tokens: Vec<String> = Vec::new();
            while let Some(flag) = args.next() {
                match flag.as_str() {
                    "--seeds-file" => {
                        let p = args
                            .next()
                            .ok_or(CliError::MissingArgument("--seeds-file <path>"))?;
                        seeds_path = Some(std::path::PathBuf::from(p));
                    }
                    "--out" => {
                        let p = args
                            .next()
                            .ok_or(CliError::MissingArgument("--out <path>"))?;
                        out_path = Some(std::path::PathBuf::from(p));
                    }
                    "--stats" => {
                        include_stats = true;
                    }
                    other if is_shared_cli_flag(other) => {
                        tail_tokens.push(other.to_string());
                        if shared_flag_needs_value(other) {
                            let v = args
                                .next()
                                .ok_or(CliError::MissingArgument("shared flag value"))?;
                            tail_tokens.push(v);
                        }
                    }
                    other => return Err(CliError::UnknownCommand(other.to_string())),
                }
            }
            let seeds_path = seeds_path.ok_or(CliError::MissingArgument("--seeds-file <path>"))?;
            parse_hard_cli_flags(&mut tail_tokens.into_iter())?;

            // Map mix characters to difficulties
            let chars: Vec<char> = mix.chars().collect();
            let map_char = |c: char| -> Option<crate::bot::BotDifficulty> {
                match c {
                    'e' | 'E' => Some(crate::bot::BotDifficulty::EasyLegacy),
                    'n' | 'N' => Some(crate::bot::BotDifficulty::NormalHeuristic),
                    'h' | 'H' => Some(crate::bot::BotDifficulty::FutureHard),
                    _ => None,
                }
            };
            let mut diffs = [crate::bot::BotDifficulty::NormalHeuristic; 4];
            for (i, c) in chars.into_iter().enumerate() {
                let d = map_char(c).ok_or(CliError::UnknownCommand(
                    "--match-mixed-file invalid mix char (use e|n|h)".to_string(),
                ))?;
                diffs[i] = d;
            }

            let mut rows = Vec::new();
            if include_stats {
                rows.push(
                    "seed,seat,mix,pen,scanned,elapsed_ms,dp_hits,nudge_hits,nudge_guard"
                        .to_string(),
                );
            } else {
                rows.push("seed,seat,mix,pen".to_string());
            }
            let content = std::fs::read_to_string(&seeds_path).map_err(CliError::Io)?;
            for line in content.lines() {
                let s = line.trim();
                if s.is_empty() {
                    continue;
                }
                if let Ok(seed) = s.parse::<u64>() {
                    let pen = simulate_one_round_mixed(seed, seat, diffs)?;
                    if include_stats {
                        let stats = crate::bot::search::last_stats();
                        let (nudges, nudge_trace) = collect_nudge_metrics(stats.as_ref());
                        let (scanned, elapsed, dp) = stats
                            .map(|s| (s.scanned, s.elapsed_ms, s.endgame_dp_hits))
                            .unwrap_or((0usize, 0u32, 0usize));
                        rows.push(format!(
                            "{}, {:?}, {}, {}, {}, {}, {}, {}, {}",
                            seed, seat, mix, pen, scanned, elapsed, dp, nudges, nudge_trace
                        ));
                    } else {
                        rows.push(format!("{}, {:?}, {}, {}", seed, seat, mix, pen));
                    }
                }
            }
            if let Some(path) = out_path {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&path, rows.join("\n")).map_err(CliError::Io)?;
                println!("Wrote mixed-match CSV to {}", path.display());
            } else {
                for line in rows {
                    println!("{}", line);
                }
            }
            Ok(CliOutcome::Handled)
        }
        "--help" | "-h" => {
            let help = "Available commands:\n  --export-snapshot <path> [seed] [seat]\n  --import-snapshot <path>\n  --show-weights\n  --explain-once <seed> <seat> [difficulty] [Hard flags]\n  --explain-batch <seat> <seed_start> <count> [difficulty] [Hard flags]\n  --explain-snapshot <path> <seat> [Hard flags]\n  --explain-pass-once <seed> <seat>\n  --explain-pass-batch <seat> <seed_start> <count>\n  --compare-once <seed> <seat> [Hard flags]\n  --compare-batch <seat> <seed_start> <count> [--out <path>] [--only-disagree] [Hard flags]\n  --explain-json <seed> <seat> <path> [difficulty] [Hard flags]\n  --bench-check <difficulty> <seat> <seed_start> <count> [Hard flags]\n  --match-batch <seat> <seed_start> <count> [difficultyA difficultyB] [--out <path>] [Hard flags]\n  --match-mixed <seat> <seed_start> <count> <mix> [--out <path>] [--stats] [Hard flags]\n  --match-mixed-file <seat> <mix> --seeds-file <path> [--out <path>] [--stats] [Hard flags]\n\nHard flags (can follow many commands):\n  --hard-deterministic             Enable deterministic mode (step-capped)\n  --hard-steps <n>                 Deterministic step cap for Hard\n  --hard-phaseb-topk <k>           Top-K candidates for continuation scoring\n  --hard-branch-limit <n>          Candidate branch limit (base ordering)\n  --hard-next-branch-limit <n>     Next-trick probe branch limit\n  --hard-time-cap-ms <ms>          Wall-clock cap (non-deterministic mode)\n  --hard-cutoff <margin>           Early cutoff margin for choose()\n  --hard-cont-boost-gap <n>        Gap threshold to boost continuation in near ties\n  --hard-cont-boost-factor <n>     Multiplier applied to continuation in near ties\n  --hard-det | --hard-det-enable   Enable determinization sampling (env-gated)\n  --hard-det-k <n>                 Number of determinization samples (K)\n  --hard-det-probe                 Widen next-trick probe under determinization\n  --hard-verbose                   Print verbose continuation parts (requires MDH_DEBUG_LOGS=1)\n  --help";
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
        "search" | "lookahead" => Some(crate::bot::BotDifficulty::SearchLookahead),
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

const HARD_FLAGS_NEED_VALUE: [&str; 8] = [
    "--hard-steps",
    "--hard-phaseb-topk",
    "--hard-branch-limit",
    "--hard-next-branch-limit",
    "--hard-time-cap-ms",
    "--hard-cutoff",
    "--hard-cont-boost-gap",
    "--hard-cont-boost-factor",
];

const THINK_FLAGS_NEED_VALUE: [&str; 2] = ["--think-limit-ms", "--think-limit"];

fn shared_flag_needs_value(flag: &str) -> bool {
    HARD_FLAGS_NEED_VALUE.contains(&flag) || THINK_FLAGS_NEED_VALUE.contains(&flag)
}

fn is_shared_cli_flag(flag: &str) -> bool {
    flag.starts_with("--hard-") || flag.starts_with("--think-")
}

fn parse_hard_cli_flags(args: &mut impl Iterator<Item = String>) -> Result<(), CliError> {
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--think-limit-ms" => {
                let raw = args
                    .next()
                    .ok_or(CliError::MissingArgument("--think-limit-ms <ms>"))?;
                let ms = raw.parse::<u64>().map_err(|_| CliError::InvalidValue {
                    flag: "--think-limit-ms",
                    value: raw.clone(),
                })?;
                unsafe { std::env::set_var("MDH_THINK_LIMIT_MS", ms.to_string()) }
            }
            "--think-limit" => {
                let raw = args.next().ok_or(CliError::MissingArgument(
                    "--think-limit <5s|10s|15s|unlimited>",
                ))?;
                let preset = match raw.trim().to_ascii_lowercase().as_str() {
                    "5" | "5s" => Some(5_000),
                    "10" | "10s" => Some(10_000),
                    "15" | "15s" => Some(15_000),
                    "unlimited" | "inf" | "infinite" => Some(0),
                    _ => None,
                }
                .ok_or_else(|| CliError::InvalidValue {
                    flag: "--think-limit",
                    value: raw.clone(),
                })?;
                unsafe { std::env::set_var("MDH_THINK_LIMIT_MS", preset.to_string()) }
            }
            "--think-limit-unlimited" | "--think-unlimited" => unsafe {
                std::env::set_var("MDH_THINK_LIMIT_MS", "0")
            },
            "--think-limit-reset" | "--think-limit-default" => unsafe {
                std::env::remove_var("MDH_THINK_LIMIT_MS");
            },
            "--hard-stage1" => unsafe { std::env::set_var("MDH_FEATURE_HARD_STAGE1", "1") },
            "--hard-stage2" => unsafe { std::env::set_var("MDH_FEATURE_HARD_STAGE2", "1") },
            "--hard-stage12" | "--hard-all" => unsafe {
                std::env::set_var("MDH_FEATURE_HARD_STAGE12", "1")
            },
            "--hard-stage12-off" | "--hard-all-off" => unsafe {
                std::env::remove_var("MDH_FEATURE_HARD_STAGE12");
                std::env::remove_var("MDH_FEATURE_HARD_STAGE1");
                std::env::remove_var("MDH_FEATURE_HARD_STAGE2");
            },
            "--hard-deterministic" => unsafe { std::env::set_var("MDH_HARD_DETERMINISTIC", "1") },
            "--hard-steps" => {
                let v = args
                    .next()
                    .ok_or(CliError::MissingArgument("--hard-steps <n>"))?;
                unsafe { std::env::set_var("MDH_HARD_TEST_STEPS", v) }
            }
            "--hard-det" | "--hard-det-enable" => unsafe {
                std::env::set_var("MDH_HARD_DET_ENABLE", "1")
            },
            "--hard-det-k" => {
                let v = args
                    .next()
                    .ok_or(CliError::MissingArgument("--hard-det-k <n>"))?;
                unsafe { std::env::set_var("MDH_HARD_DET_SAMPLE_K", v) }
            }
            "--hard-det-probe" => unsafe { std::env::set_var("MDH_HARD_DET_PROBE_WIDE_LIKE", "1") },
            "--hard-phaseb-topk" => {
                let v = args
                    .next()
                    .ok_or(CliError::MissingArgument("--hard-phaseb-topk <k>"))?;
                unsafe { std::env::set_var("MDH_HARD_PHASEB_TOPK", v) }
            }
            "--hard-branch-limit" => {
                let v = args
                    .next()
                    .ok_or(CliError::MissingArgument("--hard-branch-limit <n>"))?;
                unsafe { std::env::set_var("MDH_HARD_BRANCH_LIMIT", v) }
            }
            "--hard-next-branch-limit" => {
                let v = args
                    .next()
                    .ok_or(CliError::MissingArgument("--hard-next-branch-limit <n>"))?;
                unsafe { std::env::set_var("MDH_HARD_NEXT_BRANCH_LIMIT", v) }
            }
            "--hard-time-cap-ms" => {
                let v = args
                    .next()
                    .ok_or(CliError::MissingArgument("--hard-time-cap-ms <ms>"))?;
                unsafe { std::env::set_var("MDH_HARD_TIME_CAP_MS", v) }
            }
            "--hard-cutoff" => {
                let v = args
                    .next()
                    .ok_or(CliError::MissingArgument("--hard-cutoff <margin>"))?;
                unsafe { std::env::set_var("MDH_HARD_EARLY_CUTOFF_MARGIN", v) }
            }
            "--hard-cont-boost-gap" => {
                let v = args
                    .next()
                    .ok_or(CliError::MissingArgument("--hard-cont-boost-gap <n>"))?;
                unsafe { std::env::set_var("MDH_HARD_CONT_BOOST_GAP", v) }
            }
            "--hard-cont-boost-factor" => {
                let v = args
                    .next()
                    .ok_or(CliError::MissingArgument("--hard-cont-boost-factor <n>"))?;
                unsafe { std::env::set_var("MDH_HARD_CONT_BOOST_FACTOR", v) }
            }
            "--hard-verbose" => unsafe { std::env::set_var("MDH_HARD_VERBOSE_CONT", "1") },
            other if other.starts_with("--") => {
                return Err(CliError::UnknownCommand(other.to_string()));
            }
            _ => break,
        }
    }
    Ok(())
}

fn seek_dp_flip_for(
    seed: u64,
    target_seat: PlayerPosition,
) -> Result<Option<(usize, Option<String>, Option<String>)>, CliError> {
    let mut controller =
        crate::controller::GameController::new_with_seed(Some(seed), PlayerPosition::North);
    controller.set_bot_difficulty(crate::bot::BotDifficulty::FutureHard);
    // Resolve passing
    if controller.in_passing_phase() {
        if let Some(cards) = controller.simple_pass_for(target_seat) {
            let _ = controller.submit_pass(target_seat, cards);
        }
        let _ = controller.submit_auto_passes_for_others(target_seat);
        let _ = controller.resolve_passes();
    }
    // Autoplay until it is target's turn and all hands have <= 3 cards
    let mut guard = 0u32;
    loop {
        if guard > 300 {
            return Ok(None);
        }
        guard += 1;
        let to_play = controller.expected_to_play();
        // Check hand sizes
        let round = controller.bot_context(target_seat).round;
        let mut ok_small = true;
        for s in [
            PlayerPosition::North,
            PlayerPosition::East,
            PlayerPosition::South,
            PlayerPosition::West,
        ] {
            if round.hand(s).len() > 3 {
                ok_small = false;
                break;
            }
        }
        if ok_small && to_play == target_seat {
            break;
        }
        let _ = controller.autoplay_one(to_play.next());
    }
    // Build legal set and context
    let legal = controller.legal_moves(target_seat);
    if legal.is_empty() {
        return Ok(None);
    }
    let ctx = controller.bot_context(target_seat);
    // Toggle DP off, pick choose() top (Hard choose path uses DP when enabled)
    unsafe {
        std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
    }
    let off_top = crate::bot::PlayPlannerHard::choose(&legal, &ctx).map(|c| c.to_string());
    // DP on, pick choose() top
    unsafe {
        std::env::set_var("MDH_HARD_ENDGAME_DP_ENABLE", "1");
    }
    let on_top = crate::bot::PlayPlannerHard::choose(&legal, &ctx).map(|c| c.to_string());
    if off_top != on_top {
        return Ok(Some((legal.len(), off_top, on_top)));
    }
    Ok(None)
}

fn simulate_one_round(
    seed: u64,
    seat: PlayerPosition,
    difficulty: crate::bot::BotDifficulty,
) -> Result<u8, CliError> {
    let mut controller =
        crate::controller::GameController::new_with_seed(Some(seed), PlayerPosition::North);
    controller.set_bot_difficulty(difficulty);
    if controller.in_passing_phase() {
        if let Some(cards) = controller.simple_pass_for(seat) {
            let _ = controller.submit_pass(seat, cards);
        }
        let _ = controller.submit_auto_passes_for_others(seat);
        let _ = controller.resolve_passes();
    }
    loop {
        let totals = controller.penalties_this_round();
        let sum: u32 = totals.iter().map(|&v| v as u32).sum();
        if sum >= 26 {
            break;
        }
        let to_play = controller.expected_to_play();
        let _ = controller.autoplay_one(to_play.next());
    }
    let totals = controller.penalties_this_round();
    Ok(totals[seat.index()])
}

fn simulate_one_round_mixed(
    seed: u64,
    seat: PlayerPosition,
    diffs: [crate::bot::BotDifficulty; 4],
) -> Result<u8, CliError> {
    let mut controller =
        crate::controller::GameController::new_with_seed(Some(seed), PlayerPosition::North);
    // Passing: apply our seat difficulty for pass; others auto-pass with their seat difficulty
    if controller.in_passing_phase() {
        controller.set_bot_difficulty(diffs[seat.index()]);
        if let Some(cards) = controller.simple_pass_for(seat) {
            let _ = controller.submit_pass(seat, cards);
        }
        let _ = controller.submit_auto_passes_for_others(seat);
        let _ = controller.resolve_passes();
    }
    loop {
        let totals = controller.penalties_this_round();
        let sum: u32 = totals.iter().map(|&v| v as u32).sum();
        if sum >= 26 {
            break;
        }
        let to_play = controller.expected_to_play();
        controller.set_bot_difficulty(diffs[to_play.index()]);
        let _ = controller.autoplay_one(to_play.next());
    }
    let totals = controller.penalties_this_round();
    Ok(totals[seat.index()])
}

fn format_nudge_trace_summary(trace: Option<&Vec<(String, usize)>>) -> String {
    match trace {
        Some(summary) if !summary.is_empty() => summary
            .iter()
            .map(|(reason, count)| format!("{reason}:{count}"))
            .collect::<Vec<_>>()
            .join("|"),
        _ => String::new(),
    }
}

fn collect_nudge_metrics(stats: Option<&crate::bot::search::Stats>) -> (usize, String) {
    if let Some(s) = stats {
        let hits = s.planner_nudge_hits;
        let summary = match s.planner_nudge_trace.as_ref() {
            Some(trace) => format_nudge_trace_summary(Some(trace)),
            None => String::new(),
        };
        if hits > 0 || !summary.is_empty() {
            return (hits, summary);
        }
    }
    let hits = crate::bot::play::take_hard_nudge_hits();
    let summary =
        format_nudge_trace_summary(crate::bot::play::take_hard_nudge_trace_summary().as_ref());
    (hits, summary)
}

fn hard_flags_summary() -> Option<String> {
    let det = std::env::var("MDH_HARD_DETERMINISTIC").unwrap_or_default();
    let steps = std::env::var("MDH_HARD_TEST_STEPS").unwrap_or_default();
    let topk = std::env::var("MDH_HARD_PHASEB_TOPK").unwrap_or_default();
    let bl = std::env::var("MDH_HARD_BRANCH_LIMIT").unwrap_or_default();
    let nbl = std::env::var("MDH_HARD_NEXT_BRANCH_LIMIT").unwrap_or_default();
    let cap = std::env::var("MDH_HARD_TIME_CAP_MS").unwrap_or_default();
    let cutoff = std::env::var("MDH_HARD_EARLY_CUTOFF_MARGIN").unwrap_or_default();
    let boost_gap = std::env::var("MDH_HARD_CONT_BOOST_GAP").unwrap_or_default();
    let boost_factor = std::env::var("MDH_HARD_CONT_BOOST_FACTOR").unwrap_or_default();
    let stg1 = std::env::var("MDH_FEATURE_HARD_STAGE1").unwrap_or_default();
    let stg2 = std::env::var("MDH_FEATURE_HARD_STAGE2").unwrap_or_default();
    let stg12 = std::env::var("MDH_FEATURE_HARD_STAGE12").unwrap_or_default();
    let parts = vec![
        ("det", det),
        ("steps", steps),
        ("topk", topk),
        ("bl", bl),
        ("nbl", nbl),
        ("capms", cap),
        ("cutoff", cutoff),
        ("boost_gap", boost_gap),
        ("boost_factor", boost_factor),
        ("stg1", stg1),
        ("stg2", stg2),
        ("stg12", stg12),
    ];
    let items: Vec<_> = parts
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, v)| format!("{}={}", k, v))
        .collect();
    if items.is_empty() {
        None
    } else {
        Some(items.join(" "))
    }
}
