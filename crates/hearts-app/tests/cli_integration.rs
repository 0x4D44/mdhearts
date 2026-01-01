use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

#[test]
#[allow(deprecated)]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("mdhearts").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Available commands:"));
}

#[test]
#[allow(deprecated)]
fn test_cli_version() {
    let mut cmd = Command::cargo_bin("mdhearts").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("mdhearts"));
}

#[test]
#[allow(deprecated)]
fn test_export_snapshot() {
    let temp_dir = std::env::temp_dir().join("mdhearts_test_cli");
    fs::create_dir_all(&temp_dir).unwrap();
    let file_path = temp_dir.join("snapshot.json");

    // Ensure cleanup
    if file_path.exists() {
        let _ = fs::remove_file(&file_path);
    }

    let mut cmd = Command::cargo_bin("mdhearts").unwrap();
    cmd.args([
        "--export-snapshot",
        file_path.to_str().unwrap(),
        "123",
        "north",
    ])
    .assert()
    .success();

    assert!(file_path.exists());
    let content = fs::read_to_string(&file_path).unwrap();
    // serde_json::to_string_pretty output format
    assert!(content.contains("\"seed\": 123"));
}

#[test]
#[allow(deprecated)]
fn test_explain_once() {
    let mut cmd = Command::cargo_bin("mdhearts").unwrap();
    cmd.args(["--explain-once", "123", "north", "normal"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Explain North (seed 123):"));
}

#[test]
#[allow(deprecated)]
fn test_invalid_arg() {
    let mut cmd = Command::cargo_bin("mdhearts").unwrap();
    // The current CLI implementation returns 0 even for unknown commands
    cmd.arg("--invalid-flag")
        .assert()
        .success()
        .stdout(predicate::str::contains("Unknown command: --invalid-flag"));
}
