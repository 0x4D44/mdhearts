# Detailed HTML Coverage Report
$ErrorActionPreference = "Stop"

Write-Host "Generating HTML coverage report..."
cargo llvm-cov --workspace --html --ignore-filename-regex "win32\.rs|platform/mod\.rs" --output-dir target/llvm-cov/html

Write-Host "Report generated at target/llvm-cov/html/index.html"
