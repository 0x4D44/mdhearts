pub mod win32;

pub fn run() -> windows::core::Result<()> {
    win32::run()
}
