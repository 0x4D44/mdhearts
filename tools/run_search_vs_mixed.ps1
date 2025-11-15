param(
  [string[]]$Mixes = @('shsh'),
  [string]$SeatStartWest = '1000',
  [string]$SeatStartSouth = '1080',
  [string]$SeatStartEast = '2000',
  [string]$SeatStartNorth = '1100',
  [int]$CountWest = 200,
  [int]$CountSouth = 200,
  [int]$CountEast = 200,
  [int]$CountNorth = 200,
  [int]$HardSteps = 160,
  [int[]]$ThinkLimitsMs = @(5000, 0),
  [int]$SmokeCount = 5,
  [string]$SeedsFile = $null,
  [switch]$IncludeStats,
  [switch]$TelemetrySmoke,
  [switch]$TelemetryOut,
  [switch]$Verbose,
  [switch]$MixHintTrace
)

. "$PSScriptRoot/eval_shared.ps1"

$ErrorActionPreference = 'Stop'

function Invoke-MixedMatch {
  param(
    [string]$Seat,
    [string]$SeedStart,
    [int]$Count,
    [string]$Mix,
    [string]$OutPath,
    [array]$ThinkArgs,
    [array]$HardArgs,
    [string]$SeedsFile,
    [string]$TelemetryPath,
    [switch]$IncludeStats,
    [switch]$Verbose,
    [switch]$MixHintTrace
  )

  Ensure-EvalParentDir $OutPath
  if ($TelemetryPath) {
    Ensure-EvalParentDir $TelemetryPath
  }
  if ($SeedsFile) {
    $args = @(
      'run','-q','-p','hearts-app','--','--match-mixed-file',
      $Seat,$Mix,'--seeds-file',$SeedsFile,'--out',$OutPath
    )
  } else {
    $args = @(
      'run','-q','-p','hearts-app','--','--match-mixed',
      $Seat,$SeedStart,$Count,$Mix,'--out',$OutPath
    )
  }
  if ($IncludeStats) {
    $args += '--stats'
  }
  if ($TelemetryPath) {
    $args += @('--telemetry-out', $TelemetryPath)
  }
  $args += $HardArgs + $ThinkArgs
  Write-EvalInfo -Verbose:$Verbose -Message ('cargo ' + ($args -join ' '))
  $prevMixHint = $env:MDH_SEARCH_MIX_HINT
  try {
    $mixLabel = if ($Mix) { $Mix.ToLowerInvariant() } else { '' }
    $seatLabel = if ([string]::IsNullOrWhiteSpace($Seat)) { '' } else { $Seat.ToLowerInvariant() }
    if ([string]::IsNullOrWhiteSpace($mixLabel)) {
      Remove-Item Env:MDH_SEARCH_MIX_HINT -ErrorAction SilentlyContinue
    } else {
      $env:MDH_SEARCH_MIX_HINT = if ([string]::IsNullOrWhiteSpace($seatLabel)) {
        $mixLabel
      } else {
        "${mixLabel}:${seatLabel}"
      }
    }
    if ($MixHintTrace -and $Verbose) {
      $hintValue = if ([string]::IsNullOrWhiteSpace($env:MDH_SEARCH_MIX_HINT)) {
        '<unset>'
      } else {
        $env:MDH_SEARCH_MIX_HINT
      }
      Write-EvalInfo -Verbose:$Verbose -Message (
        "MixHintTrace mix={0} seat={1} hint={2}" -f $Mix, $Seat, $hintValue
      )
    }
    & cargo @args | Out-Null
    if ($LASTEXITCODE -ne 0) {
      throw "match-mixed failed for seat $Seat (mix=$Mix)"
    }
  } finally {
    if ($null -ne $prevMixHint) {
      $env:MDH_SEARCH_MIX_HINT = $prevMixHint
    } else {
      Remove-Item Env:MDH_SEARCH_MIX_HINT -ErrorAction SilentlyContinue
    }
  }
}

function Get-MixedPenaltySummary {
  param([string]$Path)
  if (-not (Test-Path $Path)) {
    return $null
  }
  $lines = Get-Content $Path
  if ($lines.Count -le 1) {
    return $null
  }
  $rawHeader = $lines[0].Split(',')
  $header = @()
  foreach ($col in $rawHeader) {
    $header += $col.Trim().ToLowerInvariant()
  }
  $penIndex = [array]::IndexOf($header, 'pen')
  if ($penIndex -lt 0) {
    return $null
  }
  $total = 0.0
  $n = 0
  foreach ($line in $lines | Select-Object -Skip 1) {
    $parts = $line.Split(',')
    if ($parts.Length -le $penIndex) {
      continue
    }
    $value = $parts[$penIndex].Trim()
    if ([string]::IsNullOrWhiteSpace($value)) {
      continue
    }
    $total += [double]$value
    $n++
  }
  if ($n -eq 0) {
    return $null
  }
  [pscustomobject]@{
    Path = $Path
    Count = $n
    AvgPen = [math]::Round($total / $n, 3)
  }
}

function Should-BoostDepth2 {
  param(
    [string]$Mix,
    [string]$Seat
  )
  $mixLower = $Mix.ToLowerInvariant()
  $seatLower = $Seat.ToLowerInvariant()
  switch ($mixLower) {
    'snnh' {
      return $seatLower -in @('north','east')
    }
    'shsh' {
      return $seatLower -in @('south','east','west')
    }
    default { return $false }
  }
}

function Get-Depth2StepTarget {
  param(
    [string]$Mix,
    [string]$Seat,
    [int]$BaseSteps
  )
  $mixLower = $Mix.ToLowerInvariant()
  $seatLower = $Seat.ToLowerInvariant()
  switch ($mixLower) {
    'snnh' {
      if ($seatLower -eq 'north') { return 320 }
      if ($seatLower -eq 'east') { return 260 }
    }
    'shsh' {
      if ($seatLower -eq 'south') { return 360 }
      if ($seatLower -eq 'east') { return 240 }
      if ($seatLower -eq 'west') { return 240 }
    }
  }
  return $BaseSteps
}

function Get-HardArgsForSeat {
  param(
    [string]$Mix,
    [string]$Seat,
    [int]$BaseSteps
  )
  $steps = $BaseSteps
  if ($env:MDH_CONT_SCHEDULE_PATH -and (Should-BoostDepth2 $Mix $Seat)) {
    $target = Get-Depth2StepTarget $Mix $Seat $BaseSteps
    $steps = [math]::Max($steps, $target)
  }
  return @('--hard-deterministic','--hard-steps',[string]$steps)
}

$env:MDH_DEBUG_LOGS = '0'
$env:MDH_HARD_DETERMINISTIC = '1'
$env:MDH_HARD_TEST_STEPS = [string]$HardSteps

$timestamp = Get-Date -Format 'yyyy-MM-dd_HHmmss'
$root = "designs/tuning/search_vs_mixed/$timestamp"
Ensure-EvalFolder $root

$seatConfigs = @(
  @{ Name = 'west'; Seat = 'west'; Start = $SeatStartWest; Count = $CountWest },
  @{ Name = 'south'; Seat = 'south'; Start = $SeatStartSouth; Count = $CountSouth },
  @{ Name = 'east'; Seat = 'east'; Start = $SeatStartEast; Count = $CountEast },
  @{ Name = 'north'; Seat = 'north'; Start = $SeatStartNorth; Count = $CountNorth }
)

$masterSummary = @("# Search vs Mixed (think limits) - $timestamp","","Root folder: $root","","| Mix | Limit | Seat | n | Avg Pen |","|-----|-------|------|---|--------:|")

foreach ($mix in $Mixes) {
  $mixDir = Join-Path $root $mix
  Ensure-EvalFolder $mixDir

  if ($SmokeCount -gt 0 -and -not $SeedsFile) {
    $smokeLimit = $ThinkLimitsMs[0]
    $smokeLabel = if ($smokeLimit -le 0) { 'limit_unlimited' } else { "limit_${smokeLimit}ms" }
    $smokeDir = Join-Path $mixDir "smoke_$smokeLabel"
    Ensure-EvalFolder $smokeDir
    $smokeArgs = Get-ThinkLimitArgs $smokeLimit
    foreach ($seatCfg in $seatConfigs) {
      $smokeOut = "$smokeDir/smoke_${mix}_$($seatCfg.Name).csv"
      $smokeCount = [math]::Min($SmokeCount, [int]$seatCfg.Count)
      $hardArgs = Get-HardArgsForSeat $mix $seatCfg.Name $HardSteps
      Invoke-MixedMatch $seatCfg.Seat $seatCfg.Start $smokeCount $mix $smokeOut $smokeArgs $hardArgs $null $null -IncludeStats:$IncludeStats -Verbose:$Verbose -MixHintTrace:$MixHintTrace
    }
  }

  foreach ($limit in $ThinkLimitsMs) {
    $label = if ($limit -le 0) { 'limit_unlimited' } else { "limit_${limit}ms" }
    $limitDir = Join-Path $mixDir $label
    Ensure-EvalFolder $limitDir
    $thinkArgs = Get-ThinkLimitArgs $limit
    $limitSummary = @("# Mix: $mix / Think limit: $label","","Seat penalties:")

    foreach ($seatCfg in $seatConfigs) {
      $matchOut = "$limitDir/match_${mix}_$($seatCfg.Name)_$($seatCfg.Start)_$($seatCfg.Count).csv"
      $telemetryPath = $null
      if ($TelemetryOut) {
        $telemetryPath = "$limitDir/telemetry_${mix}_$($seatCfg.Name).jsonl"
      }
      $hardArgs = Get-HardArgsForSeat $mix $seatCfg.Name $HardSteps
      Invoke-MixedMatch $seatCfg.Seat $seatCfg.Start $seatCfg.Count $mix $matchOut $thinkArgs $hardArgs $SeedsFile $telemetryPath -IncludeStats:$IncludeStats -Verbose:$Verbose -MixHintTrace:$MixHintTrace
      $stats = Get-MixedPenaltySummary $matchOut
      if ($null -ne $stats) {
        $limitSummary += "- $($stats.Path): n=$($stats.Count) avg_pen=$($stats.AvgPen)"
        $masterSummary += "| $mix | $label | $($seatCfg.Name) | $($stats.Count) | $($stats.AvgPen) |"
      } else {
        $limitSummary += "- $($matchOut): no data (check run)"
        $masterSummary += "| $mix | $label | $($seatCfg.Name) | 0 | N/A |"
      }
    }

    if ($TelemetrySmoke) {
      $smokeHardArgs = Get-HardArgsForSeat $mix 'north' $HardSteps
      $smokeResult = Invoke-EvalTelemetrySmoke 'north' $SeatStartNorth $limitDir $smokeHardArgs -Verbose:$Verbose
      $limitSummary += ""
      $limitSummary += "Telemetry smoke fallback rows: $($smokeResult.TimeoutCount) ($($smokeResult.TelemetryPath))"
    }

    $summaryPath = "$limitDir/summary.md"
    Set-Content -Encoding UTF8 $summaryPath -Value ($limitSummary -join "`n")
  }
}

$masterSummaryPath = "$root/summary.md"
Set-Content -Encoding UTF8 $masterSummaryPath -Value ($masterSummary -join "`n")
Write-Host "Search vs mixed evaluation artifacts written to $root"
$null = $Error
