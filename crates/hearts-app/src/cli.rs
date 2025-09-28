use hearts_core::game::match_state::MatchState;
use hearts_core::game::serialization::MatchSnapshot;
use hearts_core::model::player::PlayerPosition;
use serde_json;
use std::fs;
use std::path::PathBuf;
use windows::Win32::Foundation::HWND;
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
        "--help" | "-h" => {
            let help = "Available commands:\n  --export-snapshot <path> [seed] [seat]\n  --import-snapshot <path>\n  --help";
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
    show_box("mdhearts CLI", message, MB_ICONERROR | MB_OK);
}

fn show_info_box(title: &str, message: &str) {
    show_box(title, message, MB_ICONINFORMATION | MB_OK);
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
