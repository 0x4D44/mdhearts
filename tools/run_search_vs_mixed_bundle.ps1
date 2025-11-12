[CmdletBinding()]
param(
    [switch]$SkipRuns,
    [string]$ExistingRoot,
    [string[]]$Mixes = @('snnh','shsh'),
    [string]$SeatStartWest = '1000',
    [string]$SeatStartSouth = '1080',
    [string]$SeatStartEast = '2000',
    [string]$SeatStartNorth = '1100',
    [int]$CountWest = 40,
    [int]$CountSouth = 40,
    [int]$CountEast = 40,
    [int]$CountNorth = 40,
    [int]$HardSteps = 200,
    [int[]]$ThinkLimitsMs = @(5000,10000,15000,20000),
    [int]$SmokeCount = 0,
    [string]$SeedsFile = $null,
    [switch]$IncludeStats,
    [switch]$TelemetrySmoke,
    [switch]$TelemetryOut,
    [switch]$VerboseRun,
    [string]$BundleName = 'bundle',
    [string]$AnalyzerOut = $null,
    [string]$FitOut = $null,
    [switch]$SkipAnalyzer,
    [switch]$SkipFit,
    [switch]$CopyToTmp = $true
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$runScript = Join-Path $PSScriptRoot 'run_search_vs_mixed.ps1'
$analyzerScript = Join-Path $PSScriptRoot 'analyze_search_vs_mixed.py'
$fitScript = Join-Path $PSScriptRoot 'fit_continuation_schedule.py'

function Resolve-SweepRoot {
    param(
        [switch]$SkipRunsLocal,
        [string]$ExistingRootLocal
    )
    if ($ExistingRootLocal) {
        $resolved = Resolve-Path -Path $ExistingRootLocal -ErrorAction Stop
        return $resolved.Path
    }
    if ($SkipRunsLocal) {
        throw 'SkipRuns is set but no ExistingRoot was provided.'
    }
    $runArgs = @('-Mixes'); $runArgs += $Mixes
    $runArgs += '-SeatStartWest'; $runArgs += $SeatStartWest
    $runArgs += '-SeatStartSouth'; $runArgs += $SeatStartSouth
    $runArgs += '-SeatStartEast'; $runArgs += $SeatStartEast
    $runArgs += '-SeatStartNorth'; $runArgs += $SeatStartNorth
    $runArgs += '-CountWest'; $runArgs += $CountWest
    $runArgs += '-CountSouth'; $runArgs += $CountSouth
    $runArgs += '-CountEast'; $runArgs += $CountEast
    $runArgs += '-CountNorth'; $runArgs += $CountNorth
    $runArgs += '-HardSteps'; $runArgs += $HardSteps
    $runArgs += '-ThinkLimitsMs'; $runArgs += $ThinkLimitsMs
    $runArgs += '-SmokeCount'; $runArgs += $SmokeCount
    if ($SeedsFile) { $runArgs += '-SeedsFile'; $runArgs += $SeedsFile }
    if ($IncludeStats) { $runArgs += '-IncludeStats' }
    if ($TelemetrySmoke) { $runArgs += '-TelemetrySmoke' }
    if ($TelemetryOut) { $runArgs += '-TelemetryOut' }
    if ($VerboseRun) { $runArgs += '-Verbose' }

    $runLines = @()
    & $runScript @runArgs 2>&1 | Tee-Object -Variable runLines | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "run_search_vs_mixed.ps1 exited with code $LASTEXITCODE"
    }
    $match = $runLines | Select-String -Pattern 'Search vs mixed evaluation artifacts written to (.+)' | Select-Object -Last 1
    if (-not $match) {
        throw 'Unable to determine sweep root from run output.'
    }
    $rootPath = $match.Matches[0].Groups[1].Value.Trim()
    if (-not (Test-Path $rootPath)) {
        throw "Sweep root '$rootPath' not found after run."
    }
    return (Resolve-Path -Path $rootPath).Path
}

function Invoke-Analyzer {
    param(
        [string]$Root,
        [string]$OutPath
    )
    $outDir = Split-Path -Parent $OutPath
    if ($outDir) { New-Item -ItemType Directory -Path $outDir -Force | Out-Null }
    & python $analyzerScript --root $Root --out $OutPath
    if ($LASTEXITCODE -ne 0) {
        throw "Analyzer failed with exit code $LASTEXITCODE"
    }
}

function Invoke-Fitter {
    param(
        [string]$InputPath,
        [string]$OutPath
    )
    $outDir = Split-Path -Parent $OutPath
    if ($outDir) { New-Item -ItemType Directory -Path $outDir -Force | Out-Null }
    & python $fitScript --inputs $InputPath --out $OutPath
    if ($LASTEXITCODE -ne 0) {
        throw "Fitter failed with exit code $LASTEXITCODE"
    }
}

function Write-Summary {
    param(
        [string]$Root,
        [string]$AnalyzerPath,
        [string]$FitPath,
        [string]$SummaryPath
    )
    $bundleDir = Split-Path -Parent $SummaryPath
    if ($bundleDir) { New-Item -ItemType Directory -Path $bundleDir -Force | Out-Null }
    $analysis = Get-Content -Raw -Path $AnalyzerPath | ConvertFrom-Json
    $rows = @()
    foreach ($mixProp in $analysis.PSObject.Properties) {
        $mixName = $mixProp.Name
        $seatTrends = $mixProp.Value.seat_trends
        if (-not $seatTrends) { continue }
        foreach ($seatProp in $seatTrends.PSObject.Properties) {
            $trend = $seatProp.Value
            $rows += [pscustomobject]@{
                Mix = $mixName
                Seat = $seatProp.Name
                AvgDepth2 = $trend.avg_depth2_samples
                Slope = $trend.penalty_slope_per_10s
                Fails = [bool]$trend.fails_goals
            }
        }
    }
    $fails = ($rows | Where-Object { $_.Fails }).Count
    $total = $rows.Count
    $bundleLabel = Split-Path -Leaf $Root
    $lines = @()
    $lines += "# Search vs Mixed Bundle - $bundleLabel"
    $lines += ''
    $lines += "Root: $Root"
    $lines += "Analyzer JSON: $AnalyzerPath"
    $lines += "Fit JSON: $FitPath"
    $lines += ''
    $lines += "Fails: $fails / $total seats"
    $lines += ''
    $lines += '| Mix | Seat | Avg Depth2 | Slope / 10s | Fails Goals |'
    $lines += '|-----|------|------------:|------------:|-------------|'
    foreach ($row in $rows | Sort-Object Mix, Seat) {
        $avgValue = if ($null -ne $row.AvgDepth2) { [double]$row.AvgDepth2 } else { 0.0 }
        $slopeValue = if ($null -ne $row.Slope) { [double]$row.Slope } else { 0.0 }
        $avgDepth = '{0:N2}' -f $avgValue
        $slope = '{0:N3}' -f $slopeValue
        $flag = if ($row.Fails) { 'yes' } else { 'no' }
        $lines += "| $($row.Mix) | $($row.Seat) | $avgDepth | $slope | $flag |"
    }
    Set-Content -Path $SummaryPath -Encoding UTF8 -Value $lines
}

$sweepRoot = Resolve-SweepRoot -SkipRunsLocal:$SkipRuns -ExistingRootLocal:$ExistingRoot
$bundleDir = Join-Path $sweepRoot $BundleName
New-Item -ItemType Directory -Path $bundleDir -Force | Out-Null
function Resolve-TargetPath {
    param([string]$PathCandidate, [string]$DefaultPath)
    if (-not $PathCandidate) { return $DefaultPath }
    if ([System.IO.Path]::IsPathRooted($PathCandidate)) {
        return $PathCandidate
    }
    $cwd = Get-Location
    return (Join-Path $cwd.Path $PathCandidate)
}
$analysisPath = Resolve-TargetPath -PathCandidate $AnalyzerOut -DefaultPath (Join-Path $bundleDir 'analysis.json')
$fitPath = Resolve-TargetPath -PathCandidate $FitOut -DefaultPath (Join-Path $bundleDir 'continuation_fit.json')

if (-not $SkipAnalyzer) {
    Invoke-Analyzer -Root $sweepRoot -OutPath $analysisPath
}
if (-not $SkipFit) {
    Invoke-Fitter -InputPath $analysisPath -OutPath $fitPath
}
$summaryPath = Join-Path $bundleDir 'summary.md'
Write-Summary -Root $sweepRoot -AnalyzerPath $analysisPath -FitPath $fitPath -SummaryPath $summaryPath

if ($CopyToTmp) {
    $tmpDir = Join-Path $repoRoot 'tmp\search_vs_mixed'
    New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null
    $bundleLabel = Split-Path -Leaf $sweepRoot
    $analysisName = "${bundleLabel}_" + (Split-Path -Leaf $analysisPath)
    $fitName = "${bundleLabel}_" + (Split-Path -Leaf $fitPath)
    Copy-Item -Path $analysisPath -Destination (Join-Path $tmpDir $analysisName) -Force
    Copy-Item -Path $fitPath -Destination (Join-Path $tmpDir $fitName) -Force
}

Write-Host "Bundle complete. Root: $sweepRoot"
Write-Host "Summary: $summaryPath"
