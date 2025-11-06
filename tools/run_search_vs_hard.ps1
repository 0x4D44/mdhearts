param(
  [string]$SeatStartWest = '1000',
  [string]$SeatStartSouth = '1080',
  [string]$SeatStartEast = '2000',
  [string]$SeatStartNorth = '1100',
  [int]$CountWest = 200,
  [int]$CountSouth = 200,
  [int]$CountEast = 200,
  [int]$CountNorth = 200,
  [int]$HardSteps = 160,
  [int[]]$ThinkLimitsMs = @(5000, 10000, 15000, 0),
  [switch]$Verbose,
  [switch]$VerifyTimeoutTelemetry
)

$ErrorActionPreference = 'Stop'

function Write-Info($msg) { if ($Verbose) { Write-Host $msg } }

function Ensure-Dir($path) {
  $dir = Split-Path -Parent $path
  if ([string]::IsNullOrWhiteSpace($dir)) { return }
  if (-not (Test-Path $dir)) {
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
  }
}

function Ensure-Folder($dir) {
  if (-not (Test-Path $dir)) {
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
  }
}

function Get-ThinkArgs([int]$limitMs) {
  if ($limitMs -le 0) { return @('--think-limit-unlimited') }
  return @('--think-limit-ms', [string]$limitMs)
}

function Invoke-CompareBatch($seat, $start, $count, $outPath, $thinkArgs, $hardArgs) {
  Ensure-Dir $outPath
  $args = @(
    'run','-q','-p','hearts-app','--','--compare-batch',
    $seat,$start,$count,'search','hard','--only-disagree','--out',$outPath
  ) + $hardArgs + $thinkArgs
  Write-Info ('cargo ' + ($args -join ' '))
  & cargo @args | Out-Null
  if ($LASTEXITCODE -ne 0) {
    throw "compare-batch failed for seat $seat"
  }
}

function Count-Lines($path) {
  if (-not (Test-Path $path)) { return 0 }
  (Get-Content $path | Measure-Object -Line).Lines - 1
}

function Invoke-TelemetrySmoke($seat, $start, $outDir, $hardArgs) {
  $thinkArgs = @('--think-limit-ms','1')
  $matchCsv = Join-Path $outDir "telemetry_smoke_$seat.csv"
  $telemetryOut = Join-Path $outDir 'telemetry_smoke.jsonl'
  $prev = $env:MDH_TEST_FORCE_AUTOP_TIMEOUT
  $env:MDH_TEST_FORCE_AUTOP_TIMEOUT = '1'
  try {
    Invoke-MatchBatch $seat $start 1 $matchCsv $thinkArgs $hardArgs $telemetryOut
  } finally {
    if ($null -ne $prev) { $env:MDH_TEST_FORCE_AUTOP_TIMEOUT = $prev } else { Remove-Item Env:MDH_TEST_FORCE_AUTOP_TIMEOUT -ErrorAction SilentlyContinue }
  }
  if (-not (Test-Path $telemetryOut)) { throw "telemetry export missing ($telemetryOut)" }
  $records = Get-Content $telemetryOut | Where-Object { $_.Trim().Length -gt 0 } | ForEach-Object { $_ | ConvertFrom-Json }
  if ($records.Count -eq 0) { throw 'no telemetry records captured for timeout smoke' }
  $timeout = $records | Where-Object { $_.timed_out -eq $true -and $_.fallback }
  if ($timeout.Count -eq 0) { throw 'timeout smoke did not record fallback telemetry' }
  return [pscustomobject]@{ TelemetryPath = $telemetryOut; TimeoutCount = $timeout.Count }
}

function Summarize-Match($path) {
  if (-not (Test-Path $path)) { return $null }
  $lines = Get-Content $path | Select-Object -Skip 1 | Where-Object { $_.Trim().Length -gt 0 }
  if ($lines.Count -eq 0) { return $null }
  $totalA = 0; $totalB = 0; $n = 0
  foreach ($line in $lines) {
    $parts = $line.Split(',')
    if ($parts.Length -ge 7) {
      $totalA += [int]$parts[4].Trim()
      $totalB += [int]$parts[5].Trim()
      $n++
    }
  }
  if ($n -eq 0) { return $null }
  [pscustomobject]@{
    Path = $path
    Count = $n
    AvgA = [math]::Round($totalA / $n, 3)
    AvgB = [math]::Round($totalB / $n, 3)
    AvgDelta = [math]::Round(($totalB - $totalA) / $n, 3)
  }
}

function Invoke-MatchBatch($seat, $start, $count, $outPath, $thinkArgs, $hardArgs, $telemetryOut=$null) {
  Ensure-Dir $outPath
  if ($telemetryOut) { Ensure-Dir $telemetryOut }
  $args = @('run','-q','-p','hearts-app','--','--match-batch',$seat,$start,$count,'search','hard','--out',$outPath) + $hardArgs + $thinkArgs
  if ($telemetryOut) { $args += @('--telemetry-out', $telemetryOut) }
  Write-Info ('cargo ' + ($args -join ' '))
  & cargo @args | Out-Null
  if ($LASTEXITCODE -ne 0) {
    throw "match-batch failed for seat $seat"
  }
}

$env:MDH_DEBUG_LOGS = '0'
$env:MDH_HARD_DETERMINISTIC = '1'
$env:MDH_HARD_TEST_STEPS = [string]$HardSteps

$timestamp = Get-Date -Format 'yyyy-MM-dd_HHmmss'
$root = "designs/tuning/search_vs_hard/$timestamp"
Ensure-Folder $root

$seatConfigs = @(
  @{ Name = 'west'; Seat = 'west'; Start = $SeatStartWest; Count = $CountWest },
  @{ Name = 'south'; Seat = 'south'; Start = $SeatStartSouth; Count = $CountSouth },
  @{ Name = 'east'; Seat = 'east'; Start = $SeatStartEast; Count = $CountEast },
  @{ Name = 'north'; Seat = 'north'; Start = $SeatStartNorth; Count = $CountNorth }
)

$hardArgs = @('--hard-deterministic','--hard-steps',[string]$HardSteps)
$masterSummary = @("# Search vs Hard (think limits) - $timestamp","","Root folder: $root","","| Limit | Seat | n | Avg Search | Avg Hard | Avg Delta |","|-------|------|---|-----------:|---------:|----------:|")

foreach ($limit in $ThinkLimitsMs) {
  $label = if ($limit -le 0) { 'limit_unlimited' } else { "limit_${limit}ms" }
  $limitDir = Join-Path $root $label
  Ensure-Folder $limitDir
  $thinkArgs = Get-ThinkArgs $limit
  $limitSummary = @("# Think limit: $label","","Match averages:")
  foreach ($seatCfg in $seatConfigs) {
    $matchOut = "$limitDir/match_$($seatCfg.Name)_$($seatCfg.Start)_$($seatCfg.Count).csv"
    Invoke-MatchBatch $seatCfg.Seat $seatCfg.Start $seatCfg.Count $matchOut $thinkArgs $hardArgs
    $stats = Summarize-Match $matchOut
    if ($null -ne $stats) {
      $limitSummary += "- $($stats.Path): n=$($stats.Count) avg_search=$($stats.AvgA) avg_hard=$($stats.AvgB) avg_delta=$($stats.AvgDelta)"
      $masterSummary += "| $label | $($seatCfg.Name) | $($stats.Count) | $($stats.AvgA) | $($stats.AvgB) | $($stats.AvgDelta) |"
    } else {
      $limitSummary += "- $($matchOut): no data (check run)"
      $masterSummary += "| $label | $($seatCfg.Name) | 0 | N/A | N/A | N/A |"
    }

    $compareOut = "$limitDir/compare_$($seatCfg.Name)_$($seatCfg.Start)_$($seatCfg.Count).csv"
    Invoke-CompareBatch $seatCfg.Seat $seatCfg.Start $seatCfg.Count $compareOut $thinkArgs $hardArgs
    $lineCount = [math]::Max((Count-Lines $compareOut), 0)
    $limitSummary += "  * compare disagreements (rows): $lineCount"
  }
  $limitSummary += ""
  if ($VerifyTimeoutTelemetry) {
    $smokeResult = Invoke-TelemetrySmoke 'north' $SeatStartNorth $limitDir $hardArgs
    $limitSummary += "Timeout telemetry smoke: fallback rows=$($smokeResult.TimeoutCount) ($($smokeResult.TelemetryPath))"
  }
  $limitSummaryPath = "$limitDir/summary.md"
  Set-Content -Encoding UTF8 $limitSummaryPath -Value ($limitSummary -join "`n")
}

$masterSummaryPath = "$root/summary.md"
Set-Content -Encoding UTF8 $masterSummaryPath -Value ($masterSummary -join "`n")
Write-Host "Search vs Hard evaluation artifacts written to $root"
$null = $Error
