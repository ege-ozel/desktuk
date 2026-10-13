<#
.SYNOPSIS
  DeskTuk memory/CPU benchmark: compares builds (e.g. v1 = Phase 1, v2 = Phase 2) in the
  same scenarios. Read-only for the user's files: every run uses a throw-away APPDATA
  (config) and throw-away item files under %TEMP%; nothing is written to the Desktop.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\bench-memory.ps1 -Exe C:\tmp\DeskTuk-v1.exe,target\release\DeskTuk.exe

  Give each build a distinct file name (the name is the table's Build column).
  Builds that do not know the circles UI (v1) are only run in the panel scenarios: the UI is
  selected with a `ui=` config line, which v1 ignores, never with `--ui` (v1 would show an
  "unknown option" message box).
#>
param(
    [Parameter(Mandatory = $true)][string[]]$Exe,
    [int]$Runs = 3,
    [int]$IdleSeconds = 15
)
$ErrorActionPreference = 'Stop'
# `-File` passes `a.exe,b.exe` as one string.
$Exe = @($Exe | ForEach-Object { $_ -split ',' } | Where-Object { $_ })

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class BenchWin {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, System.Text.StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int left, top, right, bottom; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out Rect r);
    public static IntPtr FindOwn(uint pid, string cls) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) => {
            uint p; GetWindowThreadProcessId(h, out p);
            if (p != pid) return true;
            var sb = new System.Text.StringBuilder(64); GetClassName(h, sb, 64);
            if (sb.ToString() == cls) {
                Rect rect; GetWindowRect(h, out rect);
                // Benchmark the known synthetic circle, not Recent with the user's files.
                if (cls == "DeskTuk.Circle" && (rect.left != 200 || rect.top != 200)) return true;
                found = h; return false;
            }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
'@

$root = Join-Path $env:TEMP ("desktuk-bench-$PID-" + [guid]::NewGuid())
$files = Join-Path $root 'items'
New-Item -ItemType Directory -Force $files | Out-Null
1..300 | ForEach-Object { Set-Content -LiteralPath (Join-Path $files ('file-{0:D3}.txt' -f $_)) -Value 'x' }
$paths = Get-ChildItem $files | Sort-Object Name | ForEach-Object FullName

function Config-Text([string]$kind) {
    switch ($kind) {
        'panel-default' { "host=window`nui=panel`n" }
        'panel-50'      { "host=window`nui=panel`n" + (($paths | Select-Object -First 50 | ForEach-Object { "item=$_" }) -join "`n") + "`n" }
        'circles-empty' { "host=window`nui=circles`n" }
        'circles-300'   {
            $t = "host=window`nui=circles`n"
            for ($c = 0; $c -lt 5; $c++) {
                $t += "circle=Bench $c|$(200 + 110 * $c)|200`n"
                $t += (($paths | Select-Object -Skip (60 * $c) -First 60 | ForEach-Object { "citem=Bench $c||$_" }) -join "`n") + "`n"
            }
            $t
        }
    }
}

$scenarios = @(
    @{ Name = 'panel, default items';      Config = 'panel-default'; Circles = $false; Ring = $false },
    @{ Name = 'panel, 50 items';           Config = 'panel-50';      Circles = $false; Ring = $false },
    @{ Name = 'circles, Recent only';      Config = 'circles-empty'; Circles = $true;  Ring = $false },
    @{ Name = 'circles, 5 x 60 items';     Config = 'circles-300';   Circles = $true;  Ring = $false },
    @{ Name = 'circles, ring open (20)';   Config = 'circles-300';   Circles = $true;  Ring = $true }
)

function Median($v) { $s = @($v | Sort-Object); if ($s.Count -eq 0) { return '' }; $s[[int][math]::Floor(($s.Count - 1) / 2)] }

$rows = foreach ($sc in $scenarios) {
    foreach ($item in $Exe) {
        $build = (Resolve-Path $item).Path
        # A build without the circles UI has no 'DeskTuk.Circle' window class string.
        $hasCircles = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($build)).Contains('DeskTuk.Circle')
        if ($sc.Circles -and -not $hasCircles) { continue }
        $m = @{ commit = @(); pws = @(); mods = @(); handles = @(); startcpu = @(); idlecpu = @(); ui = @() }
        for ($r = 0; $r -lt $Runs; $r++) {
            $appdata = Join-Path $root "appdata-$r"
            New-Item -ItemType Directory -Force (Join-Path $appdata 'DeskTuk') | Out-Null
            Set-Content -LiteralPath (Join-Path $appdata 'DeskTuk\desktuk.cfg') -Value (Config-Text $sc.Config) -Encoding utf8
            $log = Join-Path $root "run.log"
            Remove-Item $log -ErrorAction SilentlyContinue
            $old = $env:APPDATA; $env:APPDATA = $appdata
            $p = Start-Process -FilePath $build -WindowStyle Hidden -ArgumentList @('--no-save', '--allow-multiple', '--log', "`"$log`"") -PassThru
            $env:APPDATA = $old
            Start-Sleep -Seconds 4
            $ui = if (Test-Path $log) { (Select-String -Path $log -Pattern 'ui=(\w+)' | Select-Object -First 1 | ForEach-Object { $_.Matches[0].Groups[1].Value }) } else { $null }
            if (-not $ui) { $ui = 'panel' } # v1 does not log a ui line
            if ($sc.Ring -and $ui -eq 'circles') {
                $h = [BenchWin]::FindOwn([uint32]$p.Id, 'DeskTuk.Circle')
                if ($h -eq [IntPtr]::Zero) { throw 'benchmark circle was not found' }
                [void][BenchWin]::PostMessage($h, 0x02A1, [IntPtr]::Zero, [IntPtr]::Zero) # WM_MOUSEHOVER
                Start-Sleep -Milliseconds 1500
                if ([BenchWin]::FindOwn([uint32]$p.Id, 'DeskTuk.Ring') -eq [IntPtr]::Zero) { throw 'benchmark ring did not open' }
            }
            $p.Refresh()
            if ($p.HasExited) { throw "benchmark process exited before sampling: $build" }
            $t0 = $p.TotalProcessorTime.TotalMilliseconds
            $pw = (Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Filter "IDProcess=$($p.Id)").WorkingSetPrivate
            $m.commit += [math]::Round($p.PrivateMemorySize64 / 1KB); $m.pws += [math]::Round($pw / 1KB)
            $m.mods += $p.Modules.Count; $m.handles += $p.HandleCount; $m.startcpu += [math]::Round($t0)
            $m.ui += $ui
            if (-not $sc.Ring) {
                Start-Sleep -Seconds $IdleSeconds
                $p.Refresh(); $m.idlecpu += [math]::Round($p.TotalProcessorTime.TotalMilliseconds - $t0, 1)
            }
            Stop-Process -Id $p.Id -Force; Start-Sleep -Milliseconds 300
        }
        $ranUi = ($m.ui | Select-Object -Unique) -join ','
        if ($sc.Circles -and $ranUi -ne 'circles') { continue } # this build has no circles UI
        [pscustomobject]@{
            Scenario = $sc.Name; Build = Split-Path $build -Leaf; UI = $ranUi
            CommitKB = Median $m.commit; PrivateWsKB = Median $m.pws; Modules = Median $m.mods; Handles = Median $m.handles
            'StartCpuMs' = Median $m.startcpu; "IdleCpuMs/$($IdleSeconds)s" = Median $m.idlecpu
        }
    }
}
$rows | Format-Table -AutoSize | Out-String -Width 220
$resolvedRoot = (Resolve-Path -LiteralPath $root).Path
$tempPrefix = [IO.Path]::GetFullPath($env:TEMP).TrimEnd('\') + '\'
if (-not $resolvedRoot.StartsWith($tempPrefix, [StringComparison]::OrdinalIgnoreCase) -or (Split-Path $resolvedRoot -Leaf) -notlike "desktuk-bench-$PID-*") { throw 'unsafe benchmark cleanup path' }
Remove-Item -LiteralPath $resolvedRoot -Recurse -Force
