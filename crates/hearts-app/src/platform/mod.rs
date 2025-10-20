#[cfg(windows)]
pub mod win32;
#[cfg(windows)]
pub mod winui;

#[cfg(windows)]
pub fn run() -> windows::core::Result<()> {
    win32::run()
}

#[cfg(not(windows))]
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("mdhearts GUI is only supported on Windows. Running in CLI mode.");
    Ok(())
}

