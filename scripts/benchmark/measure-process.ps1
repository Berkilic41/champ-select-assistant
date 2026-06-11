<#
.SYNOPSIS
  Read-only process footprint measurement for the competitive benchmark
  (docs/competitive-benchmark.md). Sums RAM across all processes matching a name
  (Electron apps like iTero spawn several), reports earliest start time, and
  optionally the install-dir disk footprint.

.DESCRIPTION
  Pure measurement — does not launch, modify, or attach to any process. Safe to
  run against iTero or our own app while they are open.

.PARAMETER Name
  Process name (without .exe). Examples: "The iTero Coach", "champ-select-assistant".

.PARAMETER InstallDir
  Optional install directory to measure on-disk footprint (recursive byte sum).

.PARAMETER Json
  Emit a JSON object instead of the human table.

.EXAMPLE
  pwsh scripts/benchmark/measure-process.ps1 -Name "The iTero Coach" -InstallDir "$env:LOCALAPPDATA\Programs\iTero Drafting Coach Electron"

.EXAMPLE
  pwsh scripts/benchmark/measure-process.ps1 -Name "champ-select-assistant"
#>
param(
    [Parameter(Mandatory = $true)][string]$Name,
    [string]$InstallDir,
    [switch]$Json
)

$procs = Get-Process -Name $Name -ErrorAction SilentlyContinue
if (-not $procs) {
    Write-Error "Çalışan eşleşen process yok: '$Name' (uygulamayı açıp tekrar dene)"
    exit 1
}

$ramMB = [math]::Round((($procs | Measure-Object WorkingSet64 -Sum).Sum) / 1MB, 1)
$privMB = [math]::Round((($procs | Measure-Object PrivateMemorySize64 -Sum).Sum) / 1MB, 1)
$count = @($procs).Count

# StartTime can throw for protected processes — guard it.
$startMin = $null
try { $startMin = ($procs | Where-Object { $_.StartTime } | Measure-Object StartTime -Minimum).Minimum } catch {}

$diskMB = $null
if ($InstallDir -and (Test-Path $InstallDir)) {
    $diskMB = [math]::Round(((Get-ChildItem $InstallDir -Recurse -File -ErrorAction SilentlyContinue |
                Measure-Object Length -Sum).Sum) / 1MB, 1)
}

$result = [ordered]@{
    name           = $Name
    process_count  = $count
    ram_mb         = $ramMB
    private_mb     = $privMB
    disk_mb        = $diskMB
    earliest_start = if ($startMin) { $startMin.ToString('s') } else { $null }
    measured_at    = (Get-Date).ToString('s')
}

if ($Json) {
    $result | ConvertTo-Json
    return
}

"== $Name =="
"Process sayısı : $count"
"RAM (working)  : $ramMB MB"
"RAM (private)  : $privMB MB"
if ($null -ne $diskMB) { "Disk footprint : $diskMB MB" }
if ($startMin) { "En erken start : $startMin" }
""
"Markdown satırı (doc'a yapıştır):"
$disk = if ($null -ne $diskMB) { "$diskMB" } else { "-" }
"| $Name | $count | $ramMB | $privMB | $disk |"
