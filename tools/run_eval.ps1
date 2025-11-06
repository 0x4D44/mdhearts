param(
  [string]$SeatStartWest = '1000',
  [string]$SeatStartSouth = '1080',
  [string]$SeatStartEast = '2000',
  [string]$SeatStartNorth = '1100',
  [int]$CountWest = 150,
  [int]$CountSouth = 150,
  [int]$CountEast = 150,
  [int]$CountNorth = 200,
  [int]$HardSteps = 120,
  [int]$ThinkLimitMs = 10000,
  [switch]$Verbose
)

$ErrorActionPreference = 'Stop'

# Ensure deterministic hard settings for reproducibility
$env:MDH_DEBUG_LOGS = '0'
$env:MDH_HARD_DETERMINISTIC = '1'
$env:MDH_HARD_TEST_STEPS = [string]$HardSteps

$thinkArgs = if ($ThinkLimitMs -le 0) {
  @('--think-limit-unlimited')
} else {
  @('--think-limit-ms', [string]$ThinkLimitMs)
}

function Write-Info($msg) { if ($Verbose) { Write-Host $msg } }

function Ensure-Dir($path) {
  $dir = Split-Path -Parent $path
  if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
}

$timestamp = Get-Date -Format 'yyyy-MM-dd_HHmmss'

# Paths for outputs
$cmpWest = "designs/tuning/compare_west_${SeatStartWest}_${CountWest}_det_$timestamp.csv"
$cmpSouth = "designs/tuning/compare_south_${SeatStartSouth}_${CountSouth}_det_$timestamp.csv"
$cmpEast = "designs/tuning/compare_east_${SeatStartEast}_${CountEast}_det_$timestamp.csv"
$cmpNorth = "designs/tuning/compare_north_${SeatStartNorth}_${CountNorth}_det_$timestamp.csv"
$matchWest = "designs/tuning/match_west_${SeatStartWest}_${CountWest}_det_$timestamp.csv"
$matchSouth = "designs/tuning/match_south_${SeatStartSouth}_${CountSouth}_det_$timestamp.csv"
$matchEast = "designs/tuning/match_east_${SeatStartEast}_${CountEast}_det_$timestamp.csv"
$matchNorth = "designs/tuning/match_north_${SeatStartNorth}_${CountNorth}_det_$timestamp.csv"

Ensure-Dir $cmpWest; Ensure-Dir $cmpSouth; Ensure-Dir $cmpEast; Ensure-Dir $cmpNorth
Ensure-Dir $matchWest; Ensure-Dir $matchSouth; Ensure-Dir $matchEast; Ensure-Dir $matchNorth

# Compare batches (only disagreements)
Write-Info "Running compare-batch (only-disagree)…"
cargo run -q -p hearts-app -- --compare-batch west  $SeatStartWest  $CountWest  --only-disagree --out $cmpWest  @thinkArgs | Out-Null
cargo run -q -p hearts-app -- --compare-batch south $SeatStartSouth $CountSouth --only-disagree --out $cmpSouth @thinkArgs | Out-Null
cargo run -q -p hearts-app -- --compare-batch east  $SeatStartEast  $CountEast  --only-disagree --out $cmpEast  @thinkArgs | Out-Null
cargo run -q -p hearts-app -- --compare-batch north $SeatStartNorth $CountNorth --only-disagree --out $cmpNorth @thinkArgs | Out-Null

# Match batches (Normal vs Hard)
Write-Info "Running match-batch (Normal vs Hard)…"
cargo run -q -p hearts-app -- --match-batch west  $SeatStartWest  $CountWest  normal hard --out $matchWest  @thinkArgs | Out-Null
cargo run -q -p hearts-app -- --match-batch south $SeatStartSouth $CountSouth normal hard --out $matchSouth @thinkArgs | Out-Null
cargo run -q -p hearts-app -- --match-batch east  $SeatStartEast  $CountEast  normal hard --out $matchEast  @thinkArgs | Out-Null
cargo run -q -p hearts-app -- --match-batch north $SeatStartNorth $CountNorth normal hard --out $matchNorth @thinkArgs | Out-Null

# Summarize results
function Get-Lines($path) { (Get-Content $path | Measure-Object -Line).Lines }
function Summarize-Match($path) {
  $lines = Get-Content $path | Select-Object -Skip 1 | Where-Object { $_.Trim().Length -gt 0 }
  $totalA = 0; $totalB = 0; $n = 0
  foreach ($l in $lines) {
    $p = $l.Split(','); if ($p.Length -ge 7) { $totalA += [int]$p[4].Trim(); $totalB += [int]$p[5].Trim(); $n++ }
  }
  if ($n -gt 0) { [pscustomobject]@{ path=$path; n=$n; avg_a=[math]::Round($totalA/$n,2); avg_b=[math]::Round($totalB/$n,2); avg_delta=[math]::Round(($totalB-$totalA)/$n,2) } }
}

$summary = @()
$summary += "Compare (only-disagree) line counts:"
$summary += "- $cmpWest  lines=$(Get-Lines $cmpWest)"
$summary += "- $cmpSouth lines=$(Get-Lines $cmpSouth)"
$summary += "- $cmpEast  lines=$(Get-Lines $cmpEast)"
$summary += "- $cmpNorth lines=$(Get-Lines $cmpNorth)"
$summary += ""
$summary += "Match (Normal vs Hard) averages:"
foreach ($f in @($matchWest,$matchSouth,$matchEast,$matchNorth)) {
  $m = Summarize-Match $f
  if ($null -ne $m) { $summary += "- $($m.path): n=$($m.n) avg_a=$($m.avg_a) avg_b=$($m.avg_b) avg_delta=$($m.avg_delta)" }
}

$outMd = "designs/tuning/eval_summary_$timestamp.md"
Ensure-Dir $outMd
Set-Content -Encoding UTF8 $outMd -Value ("# Eval Summary ($timestamp)`n`n" + ($summary -join "`n") + "`n")
Write-Host "WROTE $outMd"
