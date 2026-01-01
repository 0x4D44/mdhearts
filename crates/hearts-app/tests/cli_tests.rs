use hearts_app::cli::{CliOutcome, run_cli_with_args};
use std::env;

#[test]
fn test_cli_help() {
    let args = vec!["--help".to_string()];
    let result = run_cli_with_args(args.into_iter());
    assert!(matches!(result, Ok(CliOutcome::Handled)));
}

#[test]
fn test_cli_unknown_command() {
    let args = vec!["--invalid-command".to_string()];
    let result = run_cli_with_args(args.into_iter());
    assert!(result.is_err());
}

#[test]
fn test_cli_show_weights() {
    let args = vec!["--show-weights".to_string()];
    let result = run_cli_with_args(args.into_iter());
    assert!(matches!(result, Ok(CliOutcome::Handled)));
}

#[test]
fn test_cli_bench_check() {
    // --bench-check <difficulty> <seat> <seed_start> <count>
    let args = vec![
        "--bench-check".to_string(),
        "normal".to_string(),
        "north".to_string(),
        "100".to_string(),
        "1".to_string(),
    ];
    let result = run_cli_with_args(args.into_iter());
    assert!(matches!(result, Ok(CliOutcome::Handled)));
}

#[test]
fn test_export_snapshot() {
    let temp_dir = env::temp_dir().join("mdhearts_cli_test_snap");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let path = temp_dir.join("snap.json");

    let args = vec![
        "--export-snapshot".to_string(),
        path.to_string_lossy().to_string(),
        "123".to_string(),
        "west".to_string(),
    ];
    let result = run_cli_with_args(args.into_iter());
    assert!(matches!(result, Ok(CliOutcome::Handled)));
    assert!(path.exists());

    // Import it back
    let args_import = vec![
        "--import-snapshot".to_string(),
        path.to_string_lossy().to_string(),
    ];
    let result_import = run_cli_with_args(args_import.into_iter());
    assert!(matches!(result_import, Ok(CliOutcome::Handled)));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_export_seed() {
    let temp_dir = env::temp_dir().join("mdhearts_cli_test_seed");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let path = temp_dir.join("seed.json");

    let args = vec![
        "--export-seed".to_string(),
        path.to_string_lossy().to_string(),
        "456".to_string(),
        "east".to_string(),
    ];
    let result = run_cli_with_args(args.into_iter());
    assert!(matches!(result, Ok(CliOutcome::Handled)));
    assert!(path.exists());

    // Import it back (requires --legacy-ok because seed snapshot is legacy format-ish or handled as such)
    // Actually export-seed creates a MatchSnapshot with no round data.
    let args_import = vec![
        "--import-snapshot".to_string(),
        path.to_string_lossy().to_string(),
        "--legacy-ok".to_string(),
    ];
    let result_import = run_cli_with_args(args_import.into_iter());
    assert!(matches!(result_import, Ok(CliOutcome::Handled)));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_explain_once() {
    // --explain-once <seed> <seat> [difficulty]
    let args = vec![
        "--explain-once".to_string(),
        "777".to_string(),
        "south".to_string(),
        "normal".to_string(),
    ];
    let result = run_cli_with_args(args.into_iter());
    assert!(matches!(result, Ok(CliOutcome::Handled)));
}

#[test]
fn test_explain_pass_once() {
    // --explain-pass-once <seed> <seat>
    // Needs a seed where passing happens. Round 1 has passing.
    let args = vec![
        "--explain-pass-once".to_string(),
        "1".to_string(), // Round 1, passing Left
        "north".to_string(),
    ];
    let result = run_cli_with_args(args.into_iter());
    assert!(matches!(result, Ok(CliOutcome::Handled)));
}

#[test]
fn test_compare_batch() {
    // --compare-batch <seat> <seed_start> <count>
    let temp_dir = env::temp_dir().join("mdhearts_cli_test_compare");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let path = temp_dir.join("compare.csv");

    let args = vec![
        "--compare-batch".to_string(),
        "west".to_string(),
        "1000".to_string(),
        "1".to_string(),
        "--out".to_string(),
        path.to_string_lossy().to_string(),
    ];
    let result = run_cli_with_args(args.into_iter());
    assert!(matches!(result, Ok(CliOutcome::Handled)));
    assert!(path.exists());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_export_play_dataset() {
    let temp_dir = env::temp_dir().join("mdhearts_cli_test_dataset");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let path = temp_dir.join("dataset.jsonl");

    // --export-play-dataset <seat> <seed_start> <count> <difficulty> <out>
    let args = vec![
        "--export-play-dataset".to_string(),
        "north".to_string(),
        "100".to_string(),
        "1".to_string(),
        "normal".to_string(),
        path.to_string_lossy().to_string(),
    ];
    let result = run_cli_with_args(args.into_iter());
    assert!(matches!(result, Ok(CliOutcome::Handled)));
    assert!(path.exists());

    let _ = std::fs::remove_dir_all(&temp_dir);
}
