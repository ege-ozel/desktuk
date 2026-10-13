<#
.SYNOPSIS
  DeskTuk Windows integration test: runs the in-app self-test for each desktop host,
  measures RAM/startup, and verifies that the referenced files were not modified.

.DESCRIPTION
  WARNING: this drives the real desktop. It moves the mouse, sends Win+D, opens and
  closes Notepad and, with -ExplorerRestart, kills and restarts explorer.exe.
  Run it on a test machine/VM or CI runner, not while you are working.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\windows-integration-test.ps1 -Exe target\release\DeskTuk.exe
#>
param(
    [string]$Exe = "$PSScriptRoot\..\target\release\DeskTuk.exe",
    [string]$Out = "$PSScriptRoot\..\test-results",
    [string[]]$Hosts = @('window', 'owned', 'child'),
    [switch]$ExplorerRestart,
    [int]$PerfRuns = 5
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$Exe = (Resolve-Path $Exe).Path
$problems = @()

Write-Host "== DeskTuk integration test"
Write-Host "exe: $Exe"
Write-Host ("METRIC exe_size_bytes={0}" -f (Get-Item $Exe).Length)
$os = Get-CimInstance Win32_OperatingSystem
Write-Host ("os: {0} {1} build {2}" -f $os.Caption, $os.Version, $os.BuildNumber)
Write-Host ("session: user={0} sessionId={1} interactive={2}" -f $env:USERNAME, (Get-Process -Id $PID).SessionId, [Environment]::UserInteractive)
Write-Host ("explorer running: {0}" -f [bool](Get-Process explorer -ErrorAction SilentlyContinue))

# --- Test items: one text file and one shortcut, on the user's real desktop -------------
$desk = [Environment]::GetFolderPath('Desktop')
$txt = Join-Path $desk 'DeskTuk-test.txt'
$lnk = Join-Path $desk 'DeskTuk test shortcut.lnk'
Set-Content -Path $txt -Value 'DeskTuk integration test file' -Encoding ascii
$ws = New-Object -ComObject WScript.Shell
$sc = $ws.CreateShortcut($lnk)
$sc.TargetPath = "$env:WINDIR\System32\notepad.exe"
$sc.Save()

function Get-Snapshot {
    Get-Item -LiteralPath $txt, $lnk -Force | ForEach-Object {
        '{0}|{1}|{2}|{3}' -f $_.FullName, (Get-FileHash -LiteralPath $_.FullName).Hash, $_.Attributes, $_.LastWriteTimeUtc.Ticks
    }
}
$before = Get-Snapshot
$deskBefore = (Get-ChildItem -LiteralPath $desk -Force | Sort-Object Name | ForEach-Object Name) -join '|'

function Ensure-Explorer {
    if (-not (Get-Process explorer -ErrorAction SilentlyContinue)) {
        Write-Host "explorer not running - starting it"
        Start-Process explorer.exe
        Start-Sleep -Seconds 6
    }
}

function Stop-Notepad { Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue }

function Invoke-DeskTuk([string[]]$ArgList, [int]$TimeoutMs) {
    $quoted = $ArgList | ForEach-Object { if ($_ -match '\s') { '"' + $_ + '"' } else { $_ } }
    $p = Start-Process -FilePath $Exe -ArgumentList $quoted -PassThru
    if (-not $p.WaitForExit($TimeoutMs)) {
        $p.Kill()
        return 'timeout'
    }
    return $p.ExitCode
}

# --- Shell diagnostics -------------------------------------------------------------------
$null = Invoke-DeskTuk @('--diag', '--log', "$Out\diag.log") 30000
Get-Content "$Out\diag.log" | Write-Host

# --- Self-test per host -----------------------------------------------------------------
$shots = Join-Path $Out 'shots'
foreach ($h in $Hosts) {
    Ensure-Explorer
    Stop-Notepad
    $mode = if ($ExplorerRestart) { '--selftest-explorer-restart' } else { '--selftest' }
    $log = Join-Path $Out "selftest-$h.log"
    Write-Host "`n== self-test host=$h"
    $code = Invoke-DeskTuk @($mode, '--host', $h, '--log', $log, '--shots', $shots, $lnk, $txt) 240000
    Write-Host "exit: $code (number of failed checks)"
    if ($code -eq 'timeout') { $problems += "self-test host=$h timed out" }
    if (Test-Path $log) { Get-Content $log | Write-Host } else { $problems += "self-test host=$h wrote no log" }
    Stop-Notepad
    Start-Sleep -Seconds 2
}
Ensure-Explorer

# --- RAM and startup ---------------------------------------------------------------------
Write-Host "`n== performance (host=window, $PerfRuns runs per UI)"
foreach ($ui in @('panel', 'circles')) {
for ($i = 1; $i -le $PerfRuns; $i++) {
    $log = Join-Path $Out "perf-$ui-$i.log"
    # circles: the runner has no config, so this measures the Recent circle only.
    $quoted = @('--ui', $ui, '--no-save', '--allow-multiple', '--exit-after', '6000', '--log', $log, $lnk, $txt) |
        ForEach-Object { if ($_ -match '\s') { '"' + $_ + '"' } else { $_ } }
    $p = Start-Process -FilePath $Exe -ArgumentList $quoted -PassThru
    Start-Sleep -Seconds 4
    $p.Refresh()
    Write-Host ("METRIC perf ui=$ui run={0} working_set_kb={1} private_kb={2} peak_working_set_kb={3} handles={4}" -f `
        $i, [int]($p.WorkingSet64 / 1KB), [int]($p.PrivateMemorySize64 / 1KB), [int]($p.PeakWorkingSet64 / 1KB), $p.HandleCount)
    $null = $p.WaitForExit(15000)
    Select-String -Path $log -Pattern 'METRIC' | ForEach-Object { Write-Host $_.Line }
}
}

# --- Files must be untouched ---------------------------------------------------------------
$after = Get-Snapshot
$deskAfter = (Get-ChildItem -LiteralPath $desk -Force | Sort-Object Name | ForEach-Object Name) -join '|'
$filesSame = ($before -join "`n") -eq ($after -join "`n")
$deskSame = $deskBefore -eq $deskAfter
if (-not $filesSame) { $problems += "test files changed:`n$($before -join "`n")`n->`n$($after -join "`n")" }
if (-not $deskSame) { $problems += "desktop listing changed: $deskBefore -> $deskAfter" }
Write-Host ("`nCHECK files_untouched {0}" -f $(if ($filesSame -and $deskSame) { 'PASS' } else { 'FAIL' }))
$before | ForEach-Object { Write-Host "  before: $_" }
$after | ForEach-Object { Write-Host "  after:  $_" }

# --- Screenshots to PNG ------------------------------------------------------------------
if (Test-Path $shots) {
    Add-Type -AssemblyName System.Drawing
    Get-ChildItem $shots -Filter *.bmp | ForEach-Object {
        $img = [System.Drawing.Image]::FromFile($_.FullName)
        $img.Save(($_.FullName -replace '\.bmp$', '.png'), [System.Drawing.Imaging.ImageFormat]::Png)
        $img.Dispose()
        Remove-Item $_.FullName
    }
}

# --- Summary ------------------------------------------------------------------------------
Write-Host "`n== summary"
Get-ChildItem $Out -Filter 'selftest-*.log' | ForEach-Object {
    Select-String -Path $_.FullName -Pattern ' (PASS|FAIL) ' | ForEach-Object { Write-Host $_.Line.Substring($_.Line.IndexOf('TEST')) }
}

Remove-Item -LiteralPath $txt, $lnk -Force -ErrorAction SilentlyContinue
if ($problems.Count -gt 0) {
    Write-Host "`nPROBLEMS:"
    $problems | ForEach-Object { Write-Host " - $_" }
    exit 1
}
exit 0
