use std::fs::{self, File};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::Level;
use tracing_appender::non_blocking::{self, WorkerGuard};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::{EnvFilter, fmt};

use crate::config::{LoggingConfig, ResolvedOutputs};

pub struct LoggingGuard {
    _guard: WorkerGuard,
    pub telemetry_path: PathBuf,
}

pub fn init_logging(
    logging: &LoggingConfig,
    outputs: &ResolvedOutputs,
    run_id: &str,
) -> Result<Option<LoggingGuard>> {
    if !logging.enable_structured {
        return Ok(None);
    }

    unsafe {
        std::env::set_var("MDH_DEBUG_LOGS", "1");
        std::env::set_var("MDH_BENCH_RUN_ID", run_id);
        if logging.pass_details {
            std::env::set_var("MDH_PASS_DETAILS", "1");
        }
        if logging.moon_details {
            std::env::set_var("MDH_MOON_DETAILS", "1");
        }
    }

    let telemetry_dir = outputs
        .summary_md
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    fs::create_dir_all(&telemetry_dir).with_context(|| {
        format!(
            "creating telemetry directory at {}",
            telemetry_dir.display()
        )
    })?;

    let telemetry_path = telemetry_dir.join("telemetry.jsonl");
    let file = File::create(&telemetry_path)
        .with_context(|| format!("creating telemetry file at {}", telemetry_path.display()))?;

    let (writer, guard) = non_blocking::NonBlockingBuilder::default()
        .lossy(false)
        .finish(file);

    let level = logging.level().unwrap_or(Level::INFO);
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level.as_str()));

    let subscriber = fmt::Subscriber::builder()
        .with_env_filter(filter)
        .json()
        .with_current_span(false)
        .with_span_events(FmtSpan::NONE)
        .with_writer(writer)
        .finish();

    // Ignore error if a global subscriber is already set (e.g., when running in tests)
    let _ = tracing::subscriber::set_global_default(subscriber);

    Ok(Some(LoggingGuard {
        _guard: guard,
        telemetry_path,
    }))
}
