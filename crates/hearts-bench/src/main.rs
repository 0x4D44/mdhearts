use std::path::PathBuf;

use clap::Parser;

use hearts_bench::config::{BenchmarkConfig, ResolvedOutputs};
use hearts_bench::logging::init_logging;
use hearts_bench::tournament::TournamentRunner;

/// Tournament benchmarking harness for Hearts bots.
#[derive(Debug, Parser)]
#[command(
    name = "hearts-bench",
    author,
    version,
    about = "Deterministic Hearts tournament harness"
)]
struct Cli {
    /// Path to the YAML configuration file.
    #[arg(short, long, value_name = "FILE", default_value = "bench/bench.yaml")]
    config: PathBuf,

    /// Override the run identifier (substitutes {run_id} templates).
    #[arg(long, value_name = "RUN_ID")]
    run_id: Option<String>,

    /// Override the number of hands to play.
    #[arg(long, value_name = "HANDS")]
    hands: Option<usize>,

    /// Override the RNG seed for deal generation.
    #[arg(long, value_name = "SEED")]
    seed: Option<u64>,

    /// Override the number of seat permutations per deal.
    #[arg(long, value_name = "COUNT")]
    permutations: Option<usize>,

    /// Exit after validating the configuration (no tournament is run).
    #[arg(long)]
    validate_only: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut config = BenchmarkConfig::from_path(&cli.config)?;

    if let Some(run_id) = cli.run_id {
        config.run_id = run_id;
    }

    if let Some(hands) = cli.hands {
        config.deals.hands = hands;
    }

    if let Some(seed) = cli.seed {
        config.deals.seed = Some(seed);
    }

    if let Some(permutations) = cli.permutations {
        config.deals.permutations = permutations;
    }

    config.validate()?;

    let outputs: ResolvedOutputs = config.resolved_outputs();
    let agent_count = config.agents.len();
    let run_id = config.run_id.clone();
    let hands = config.deals.hands;
    let permutations = config.deals.permutations;

    println!(
        "Loaded configuration '{run_id}' with {agent_count} agent{} ({hands} hands, {permutations} permutations)",
        if agent_count == 1 { "" } else { "s" }
    );

    let _logging_guard = init_logging(&config.logging, &outputs, &run_id)?;
    let runner = TournamentRunner::new(config, outputs)?;

    if cli.validate_only {
        println!("Validation-only mode: tournament execution skipped.");
        return Ok(());
    }

    let summary = runner.run()?;
    println!(
        "Tournament complete for '{run_id}': {} hands × {} permutations → {} rows at {}",
        summary.hands_played,
        summary.permutations,
        summary.rows_written,
        summary.jsonl_path.display()
    );
    println!("Summary table: {}", summary.summary_path.display());
    if let Some(plot_path) = summary.plot_path.as_ref() {
        println!("PPH delta plot: {}", plot_path.display());
    }
    if let Some(telemetry_path) = summary.telemetry_path.as_ref() {
        println!("Telemetry log: {}", telemetry_path.display());
    }

    Ok(())
}
