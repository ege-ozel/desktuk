<# Read-only for user files: all fixtures and configuration live in a unique TEMP directory.
   No mouse movement, keyboard input, Explorer restart or Desktop writes. #>
param([string]$Exe = 'target\release\DeskTuk.exe', [switch]$CheckDragCapture)
$ErrorActionPreference = 'Stop'
$build = (Resolve-Path -LiteralPath $Exe).Path
$tempBase = [IO.Path]::GetFullPath($env:TEMP).TrimEnd('\') + '\'
$root = Join-Path $tempBase ('desktuk-circles-regression-' + [guid]::NewGuid())
$cfg = Join-Path $root 'DeskTuk\desktuk.cfg'
$processes = @()
New-Item -ItemType Directory -Path (Join-Path $root 'DeskTuk') | Out-Null

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class CirclesRegressionWin {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int left, top, right, bottom; }
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out Rect r);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [StructLayout(LayoutKind.Sequential)] public struct GuiThreadInfo {
        public uint size, flags;
        public IntPtr active, focus, capture, menuOwner, moveSize, caret;
        public Rect caretRect;
    }
    [DllImport("user32.dll")] public static extern bool GetGUIThreadInfo(uint thread, ref GuiThreadInfo info);
    public static IntPtr CaptureOf(IntPtr h) {
        uint pid; uint thread = GetWindowThreadProcessId(h, out pid);
        var info = new GuiThreadInfo(); info.size = (uint)Marshal.SizeOf(info);
        return GetGUIThreadInfo(thread, ref info) ? info.capture : IntPtr.Zero;
    }
    [DllImport("shell32.dll", CharSet=CharSet.Unicode)] public static extern void SHChangeNotify(uint ev, uint flags, string path, IntPtr second);
    public static IntPtr FindCircle(uint pid, int x) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) => {
            uint p; GetWindowThreadProcessId(h, out p);
            if (p != pid) return true;
            var name = new StringBuilder(64); GetClassName(h, name, 64);
            Rect rect; GetWindowRect(h, out rect);
            if (name.ToString() == "DeskTuk.Circle" && rect.left == x) { found = h; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
'@

function Start-Fixture([string]$log, [string[]]$extra) {
    $previousAppData = $env:APPDATA
    try {
        $env:APPDATA = $root
        Start-Process -FilePath $build -WindowStyle Hidden -PassThru -ArgumentList (@('--allow-multiple', '--exit-after', '8000', '--log', ('"' + $log + '"')) + $extra)
    } finally { $env:APPDATA = $previousAppData }
}

try {
    $file = Join-Path $root 'reference.txt'
    Set-Content -LiteralPath $file -Value 'fixture'
    $initial = "host=window`nui=circles`nitem=$file`ncircle=Music|300|300`ngroup=Music|Live`ncitem=Music||$file`n"
    Set-Content -LiteralPath $cfg -Value $initial -Encoding utf8
    $p = Start-Fixture (Join-Path $root 'panel.log') @('--ui', 'panel', '--exit-after', '1200')
    $processes += $p
    if (-not $p.WaitForExit(10000) -or $p.ExitCode -ne 0) { throw 'panel process did not exit successfully' }
    $saved = Get-Content -LiteralPath $cfg -Raw
    $persisted = $saved -match '(?m)^ui=panel\r?$' -and $saved.Contains("item=$file") -and $saved.Contains('circle=Music|300|300') -and $saved.Contains('group=Music|Live')
    Write-Host "CLI UI selection persisted with both item lists: $persisted"

    Set-Content -LiteralPath $cfg -Value $initial -Encoding utf8
    $ringLog = Join-Path $root 'ring.log'
    $p = Start-Fixture $ringLog @('--no-save')
    $processes += $p
    Start-Sleep -Milliseconds 1500
    $h = [CirclesRegressionWin]::FindCircle([uint32]$p.Id, 300)
    if ($h -eq [IntPtr]::Zero) { throw 'fixture circle was not created' }
    if ($CheckDragCapture) {
        # Synthetic messages do not grant foreground rights. This opt-in probe requires
        # a desktop that permits activation; it does not replace the manual drag checks.
        $foreground = [CirclesRegressionWin]::GetForegroundWindow()
        [void][CirclesRegressionWin]::PostMessage($h, 0x0201, [IntPtr]1, [IntPtr]0x002c002c)
        Start-Sleep -Milliseconds 150
        $capturedForeground = [CirclesRegressionWin]::GetForegroundWindow()
        $capturedWindow = [CirclesRegressionWin]::CaptureOf($h)
        $captureReady = $capturedForeground -eq $h -and $capturedWindow -eq $h
        [void][CirclesRegressionWin]::SendMessage($h, 0x0202, [IntPtr]::Zero, [IntPtr]0x002c002c)
        Start-Sleep -Milliseconds 150
        $captureRestored = [CirclesRegressionWin]::GetForegroundWindow() -eq $foreground
        Write-Host "Full drag capture acquired and foreground restored: $($captureReady -and $captureRestored)"
        if (-not $captureReady -or -not $captureRestored) {
            Write-Host "capture diagnostics: previous=$foreground target=$h foreground=$capturedForeground capture=$capturedWindow restored=$captureRestored"
            Get-Content -LiteralPath $ringLog -Tail 8 | Write-Host
            throw 'foreground capture was not verified; requires a desktop permitting activation'
        }
    } else {
        Write-Host 'Actual drag capture: not verified (desktop check required)'
    }
    [void][CirclesRegressionWin]::PostMessage($h, 0x0200, [IntPtr]::Zero, [IntPtr]0x00140014)
    Start-Sleep -Milliseconds 1500
    $before = Get-Content -LiteralPath $ringLog -Raw
    if ($before -notmatch 'ring opened target=Circle\(0\)' -or $before -notmatch 'missing=false') { throw 'ring fixture did not load its reference' }
    Remove-Item -LiteralPath $file
    [CirclesRegressionWin]::SHChangeNotify(4, 5, $file, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 1500
    $updated = (Get-Content -LiteralPath $ringLog -Raw) -match 'missing=true'
    Write-Host "Deleted visible reference refreshed as missing: $updated"
    if (-not $persisted -or -not $updated) { throw 'circles runtime regressions failed' }
} finally {
    foreach ($p in $processes) { if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force } }
    $resolved = (Resolve-Path -LiteralPath $root).Path
    if (-not $resolved.StartsWith($tempBase, [StringComparison]::OrdinalIgnoreCase) -or (Split-Path $resolved -Leaf) -notlike 'desktuk-circles-regression-*') { throw 'unsafe fixture cleanup path' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
