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

. "$PSScriptRoot/eval_shared.ps1"

$ErrorActionPreference = 'Stop'

$env:MDH_DEBUG_LOGS = '0'
$env:MDH_HARD_DETERMINISTIC = '1'
$env:MDH_HARD_TEST_STEPS = [string]$HardSteps

$timestamp = Get-Date -Format 'yyyy-MM-dd_HHmmss'
$root = "designs/tuning/search_vs_hard/$timestamp"
Ensure-EvalFolder $root

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
  Ensure-EvalFolder $limitDir
  $thinkArgs = Get-ThinkLimitArgs $limit
  $limitSummary = @("# Think limit: $label","","Match averages:")
  foreach ($seatCfg in $seatConfigs) {
    $matchOut = "$limitDir/match_$($seatCfg.Name)_$($seatCfg.Start)_$($seatCfg.Count).csv"
    Invoke-EvalMatchBatch $seatCfg.Seat $seatCfg.Start $seatCfg.Count $matchOut $thinkArgs $hardArgs -Verbose:$Verbose
    $stats = Summarize-EvalMatch $matchOut
    if ($null -ne $stats) {
      $limitSummary += "- $($stats.Path): n=$($stats.Count) avg_search=$($stats.AvgA) avg_hard=$($stats.AvgB) avg_delta=$($stats.AvgDelta)"
      $masterSummary += "| $label | $($seatCfg.Name) | $($stats.Count) | $($stats.AvgA) | $($stats.AvgB) | $($stats.AvgDelta) |"
    } else {
      $limitSummary += "- $($matchOut): no data (check run)"
      $masterSummary += "| $label | $($seatCfg.Name) | 0 | N/A | N/A | N/A |"
    }

    $compareOut = "$limitDir/compare_$($seatCfg.Name)_$($seatCfg.Start)_$($seatCfg.Count).csv"
    Invoke-EvalCompareBatch $seatCfg.Seat $seatCfg.Start $seatCfg.Count $compareOut $thinkArgs $hardArgs -Verbose:$Verbose
    $lineCount = [math]::Max((Get-EvalDataLineCount $compareOut), 0)
    $limitSummary += "  * compare disagreements (rows): $lineCount"
  }
  $limitSummary += ""
  if ($VerifyTimeoutTelemetry) {
    $smokeResult = Invoke-EvalTelemetrySmoke 'north' $SeatStartNorth $limitDir $hardArgs -Verbose:$Verbose
    $limitSummary += "Timeout telemetry smoke: fallback rows=$($smokeResult.TimeoutCount) ($($smokeResult.TelemetryPath))"
  }
  $limitSummaryPath = "$limitDir/summary.md"
  Set-Content -Encoding UTF8 $limitSummaryPath -Value ($limitSummary -join "`n")
}

$masterSummaryPath = "$root/summary.md"
Set-Content -Encoding UTF8 $masterSummaryPath -Value ($masterSummary -join "`n")
Write-Host "Search vs Hard evaluation artifacts written to $root"
$null = $Error
