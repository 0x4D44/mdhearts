#![windows_subsystem = "windows"]
#![deny(warnings)]

mod cli;
mod controller;
mod platform;

fn main() -> windows::core::Result<()> {
    match cli::run_cli() {
        Ok(cli::CliOutcome::Handled) => Ok(()),
        Ok(cli::CliOutcome::NotHandled) => platform::run(),
        Err(err) => {
            cli::show_error_box(&format!("{}", err));
            Ok(())
        }
    }
}
