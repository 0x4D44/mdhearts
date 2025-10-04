#![cfg_attr(not(test), windows_subsystem = "windows")]
#![deny(warnings)]

use std::backtrace::Backtrace;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Once;
use std::time::{SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
use windows::core::{PCWSTR, w};

mod bot;
mod cli;
mod controller;
mod platform;

fn install_panic_hook() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            let thread = std::thread::current();
            let thread_name = thread.name().unwrap_or("unknown");
            let mut message = format!("Thread '{thread_name}' panicked");
            if let Some(loc) = info.location() {
                message.push_str(&format!(
                    " at {}:{}:{}",
                    loc.file(),
                    loc.line(),
                    loc.column()
                ));
            }
            let detail = if let Some(s) = info.payload().downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = info.payload().downcast_ref::<String>() {
                s.clone()
            } else {
                String::from("(no panic payload)")
            };
            let backtrace = Backtrace::force_capture();
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            let mut log_path = current_log_path();
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) {
                let _ = writeln!(
                    file,
                    "[{secs}.{nanos:09}] {message}
{detail}
{backtrace}",
                    secs = timestamp.as_secs(),
                    nanos = timestamp.subsec_nanos(),
                    message = message,
                    detail = detail,
                    backtrace = backtrace
                );
            } else {
                log_path = PathBuf::from("mdhearts-panic.log");
            }
            let display = format!(
                "{message}\n\n{detail}\n\nDetails saved to: {}",
                log_path.display()
            );
            let wide: Vec<u16> = display.encode_utf16().chain(std::iter::once(0)).collect();
            unsafe {
                MessageBoxW(
                    Some(HWND::default()),
                    PCWSTR(wide.as_ptr()),
                    w!("MDHearts Panic"),
                    MB_ICONERROR | MB_OK,
                );
            }
        }));
    });
}

fn current_log_path() -> PathBuf {
    match std::env::current_exe() {
        Ok(mut exe) => {
            exe.set_file_name("mdhearts-panic.log");
            exe
        }
        Err(_) => PathBuf::from("mdhearts-panic.log"),
    }
}

fn main() -> windows::core::Result<()> {
    install_panic_hook();
    match cli::run_cli() {
        Ok(cli::CliOutcome::Handled) => Ok(()),
        Ok(cli::CliOutcome::NotHandled) => platform::run(),
        Err(err) => {
            cli::show_error_box(&format!("{}", err));
            Ok(())
        }
    }
}
