# Coverage Script for MDHearts
# Excludes platform-specific UI code to provide a realistic logic coverage metric.

$ErrorActionPreference = "Stop"

Write-Host "Running cargo llvm-cov with exclusions..."
cargo llvm-cov --workspace --summary-only --ignore-filename-regex "win32\.rs|platform/mod\.rs" --output-path lcov.info

Write-Host "Coverage run complete. detailed report in lcov.info"
