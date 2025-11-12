param()

function Write-EvalInfo {
  param(
    [switch]$Verbose,
    [string]$Message
  )
  if ($Verbose) {
    Write-Host $Message
  }
}

function Ensure-EvalParentDir {
  param([string]$Path)
  $dir = Split-Path -Parent $Path
  if ([string]::IsNullOrWhiteSpace($dir)) {
    return
  }
  if (-not (Test-Path $dir)) {
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
  }
}

function Ensure-EvalFolder {
  param([string]$Path)
  if (-not (Test-Path $Path)) {
    New-Item -ItemType Directory -Force -Path $Path | Out-Null
  }
}

function Get-ThinkLimitArgs {
  param([int]$LimitMs)
  if ($LimitMs -le 0) {
    return @('--think-limit-unlimited')
  }
  return @('--think-limit-ms', [string]$LimitMs)
}

function Invoke-EvalMatchBatch {
  param(
    [string]$Seat,
    [string]$SeedStart,
    [int]$Count,
    [string]$OutPath,
    [array]$ThinkArgs,
    [array]$HardArgs,
    [string]$TelemetryOut = $null,
    [switch]$Verbose
  )
  Ensure-EvalParentDir $OutPath
  if ($TelemetryOut) {
    Ensure-EvalParentDir $TelemetryOut
  }
  $args = @(
    'run','-q','-p','hearts-app','--','--match-batch',
    $Seat,$SeedStart,$Count,'search','hard','--out',$OutPath
  ) + $HardArgs + $ThinkArgs
  if ($TelemetryOut) {
    $args += @('--telemetry-out', $TelemetryOut)
  }
  Write-EvalInfo -Verbose:$Verbose -Message ('cargo ' + ($args -join ' '))
  & cargo @args | Out-Null
  if ($LASTEXITCODE -ne 0) {
    throw "match-batch failed for seat $Seat"
  }
}

function Invoke-EvalCompareBatch {
  param(
    [string]$Seat,
    [string]$SeedStart,
    [int]$Count,
    [string]$OutPath,
    [array]$ThinkArgs,
    [array]$HardArgs,
    [switch]$Verbose
  )
  Ensure-EvalParentDir $OutPath
  $args = @(
    'run','-q','-p','hearts-app','--','--compare-batch',
    $Seat,$SeedStart,$Count,'search','hard','--only-disagree','--out',$OutPath
  ) + $HardArgs + $ThinkArgs
  Write-EvalInfo -Verbose:$Verbose -Message ('cargo ' + ($args -join ' '))
  & cargo @args | Out-Null
  if ($LASTEXITCODE -ne 0) {
    throw "compare-batch failed for seat $Seat"
  }
}

function Get-EvalDataLineCount {
  param([string]$Path)
  if (-not (Test-Path $Path)) {
    return 0
  }
  (Get-Content $Path | Measure-Object -Line).Lines - 1
}

function Summarize-EvalMatch {
  param([string]$Path)
  if (-not (Test-Path $Path)) {
    return $null
  }
  $lines = Get-Content $Path | Select-Object -Skip 1 | Where-Object { $_.Trim().Length -gt 0 }
  if ($lines.Count -eq 0) {
    return $null
  }
  $totalA = 0
  $totalB = 0
  $n = 0
  foreach ($line in $lines) {
    $parts = $line.Split(',')
    if ($parts.Length -ge 7) {
      $totalA += [int]$parts[4].Trim()
      $totalB += [int]$parts[5].Trim()
      $n++
    }
  }
  if ($n -eq 0) {
    return $null
  }
  [pscustomobject]@{
    Path = $Path
    Count = $n
    AvgA = [math]::Round($totalA / $n, 3)
    AvgB = [math]::Round($totalB / $n, 3)
    AvgDelta = [math]::Round(($totalB - $totalA) / $n, 3)
  }
}

function Invoke-EvalTelemetrySmoke {
  param(
    [string]$Seat,
    [string]$SeedStart,
    [string]$OutDir,
    [array]$HardArgs,
    [switch]$Verbose
  )
  $thinkArgs = @('--think-limit-ms','1')
  $matchCsv = Join-Path $OutDir "telemetry_smoke_$Seat.csv"
  $telemetryOut = Join-Path $OutDir 'telemetry_smoke.jsonl'
  $prev = $env:MDH_TEST_FORCE_AUTOP_TIMEOUT
  $env:MDH_TEST_FORCE_AUTOP_TIMEOUT = '1'
  try {
    Invoke-EvalMatchBatch $Seat $SeedStart 1 $matchCsv $thinkArgs $HardArgs $telemetryOut -Verbose:$Verbose
  } finally {
    if ($null -ne $prev) {
      $env:MDH_TEST_FORCE_AUTOP_TIMEOUT = $prev
    } else {
      Remove-Item Env:MDH_TEST_FORCE_AUTOP_TIMEOUT -ErrorAction SilentlyContinue
    }
  }
  if (-not (Test-Path $telemetryOut)) {
    throw "telemetry export missing ($telemetryOut)"
  }
  $records = Get-Content $telemetryOut | Where-Object { $_.Trim().Length -gt 0 } | ForEach-Object { $_ | ConvertFrom-Json }
  if ($records.Count -eq 0) {
    throw 'no telemetry records captured for timeout smoke'
  }
  $timeout = $records | Where-Object { $_.timed_out -eq $true -and $_.fallback }
  if ($timeout.Count -eq 0) {
    throw 'timeout smoke did not record fallback telemetry'
  }
  [pscustomobject]@{
    TelemetryPath = $telemetryOut
    TimeoutCount = $timeout.Count
  }
}
