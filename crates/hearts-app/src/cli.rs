use hearts_core::game::match_state::MatchState;
use hearts_core::game::serialization::MatchSnapshot;
use hearts_core::model::player::PlayerPosition;
use std::fs;
use std::path::PathBuf;
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
            let s = crate::bot::debug_weights_string();
            let msg = format!("AI Weights: {s}");
            println!("{msg}");
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

            let mut controller = crate::controller::GameController::new_with_seed(Some(seed), PlayerPosition::North);
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
            for (card, score) in explained.iter() {
                println!("  {} => {}", card, score);
            }
            Ok(CliOutcome::Handled)
        }
        "--explain-batch" => {
            let seat = args
                .next()
                .map(|s| parse_seat(&s))
                .transpose()?
                .ok_or(CliError::MissingArgument("--explain-batch <seat> <seed_start> <count>"))?;
            let seed_start = args
                .next()
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or(CliError::MissingArgument("--explain-batch <seat> <seed_start> <count>"))?;
            let count = args
                .next()
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or(CliError::MissingArgument("--explain-batch <seat> <seed_start> <count>"))?;

            for i in 0..count {
                let seed = seed_start + i;
                let mut controller = crate::controller::GameController::new_with_seed(Some(seed), PlayerPosition::North);
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
                for (card, score) in explained.iter() {
                    println!("  {} => {}", card, score);
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
                .ok_or(CliError::MissingArgument("--explain-snapshot <path> <seat>"))?;
            let seat = args
                .next()
                .map(|s| parse_seat(&s))
                .transpose()?
                .ok_or(CliError::MissingArgument("--explain-snapshot <path> <seat>"))?;
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
            for (card, score) in explained.iter() {
                println!("  {} => {}", card, score);
            }
            Ok(CliOutcome::Handled)
        }
        "--help" | "-h" => {
            let help = "Available commands:\n  --export-snapshot <path> [seed] [seat]\n  --import-snapshot <path>\n  --show-weights\n  --explain-once <seed> <seat>\n  --explain-batch <seat> <seed_start> <count>\n  --explain-snapshot <path> <seat>\n  --help";
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

#[cfg(windows)]
pub fn show_error_box(message: &str) {
    eprintln!("{message}");
    show_box("mdhearts CLI", message, MB_ICONERROR | MB_OK);
}

#[cfg(not(windows))]
pub fn show_error_box(message: &str) {
    eprintln!("{message}");
    println!("mdhearts CLI: {}", message);
}

#[cfg(windows)]
fn show_info_box(title: &str, message: &str) {
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
fn encode_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
