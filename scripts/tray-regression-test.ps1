param([string]$Exe = 'target/release/DeskTuk.exe', [switch]$DesktopOnly, [switch]$Topmost)
$ErrorActionPreference = 'Stop'
if ($DesktopOnly -and $Topmost) { throw 'Choose either -DesktopOnly or -Topmost.' }
$build = (Resolve-Path -LiteralPath $Exe).Path
$tempBase = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$root = Join-Path $tempBase ('desktuk-tray-regression-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path (Join-Path $root 'DeskTuk')
$processes = @()

Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class TrayRegressionWin {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int left, top, right, bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct MonitorInfo { public uint size; public Rect monitor, work; public uint flags; }
    [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] public struct NotifyData {
        public uint size; public IntPtr hwnd; public uint id, flags, message; public IntPtr icon;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst=128)] public string tip;
        public uint state, mask;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst=256)] public string info;
        public uint version;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst=64)] public string title;
        public uint infoFlags; public Guid guid; public IntPtr balloonIcon;
    }
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder b, int n);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out Rect r);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr h, int index);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern uint RegisterWindowMessage(string name);
    [DllImport("user32.dll")] public static extern IntPtr MonitorFromWindow(IntPtr h, uint flags);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern bool GetMonitorInfo(IntPtr h, ref MonitorInfo info);
    [DllImport("shell32.dll", CharSet=CharSet.Unicode)] public static extern bool Shell_NotifyIcon(uint command, ref NotifyData data);
    public static IntPtr Find(uint pid, string cls) {
        IntPtr result = IntPtr.Zero;
        EnumWindows((h,l) => {
            uint p; GetWindowThreadProcessId(h, out p);
            var name = new StringBuilder(64); GetClassName(h, name, 64);
            if (p == pid && name.ToString() == cls) { result=h; return false; }
            return true;
        }, IntPtr.Zero);
        return result;
    }
    public static bool InsideWorkArea(IntPtr h) {
        Rect r; if (!GetWindowRect(h, out r)) return false;
        var info = new MonitorInfo(); info.size=(uint)Marshal.SizeOf(info);
        if (!GetMonitorInfo(MonitorFromWindow(h, 2), ref info)) return false;
        return r.left >= info.work.left && r.top >= info.work.top && r.right <= info.work.right && r.bottom <= info.work.bottom;
    }
    public static bool DeleteIcon(IntPtr controller) {
        var data = new NotifyData(); data.size=(uint)Marshal.SizeOf(data); data.hwnd=controller; data.id=1;
        return Shell_NotifyIcon(2, ref data);
    }
}
'@

function Start-Fixture([string]$ui, [string]$log) {
    $previousAppData = $env:APPDATA
    try {
        $env:APPDATA = $root
        Start-Process -FilePath $build -WindowStyle Hidden -PassThru -ArgumentList @('--ui', $ui, '--no-save', '--exit-after', '18000', '--log', ('"' + $log + '"'))
    } finally { $env:APPDATA = $previousAppData }
}

function Wait-Check([scriptblock]$check, [string]$message) {
    $until = [DateTime]::UtcNow.AddSeconds(8)
    do {
        if (& $check) { Write-Host "$message : PASS"; return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $until)
    throw "$message : FAIL"
}

try {
    $file = Join-Path $root 'reference.txt'
    Set-Content -LiteralPath $file -Value 'fixture' -Encoding utf8
    $cfg = Join-Path $root 'DeskTuk/desktuk.cfg'
    Set-Content -LiteralPath $cfg -Encoding utf8 -Value "host=window`nui=circles`nx=300`ny=300`nitem=$file`nrecent=99999|99999`n"
    if ($DesktopOnly) { Add-Content -LiteralPath $cfg -Encoding utf8 -Value 'always_on_top=0' }
    if ($Topmost) { Add-Content -LiteralPath $cfg -Encoding utf8 -Value 'always_on_top=1' }
    $fileHash = (Get-FileHash -LiteralPath $file).Hash
    $configHash = (Get-FileHash -LiteralPath $cfg).Hash
    $hideBefore = (Get-ItemProperty -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced' -ErrorAction SilentlyContinue).HideIcons
    $runBefore = (Get-ItemProperty -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -ErrorAction SilentlyContinue).DeskTuk
    foreach ($ui in @('circles', 'panel')) {
        $log = Join-Path $root "$ui.log"
        $p = Start-Fixture $ui $log
        $processes += $p
        Wait-Check { (Test-Path -LiteralPath $log) -and (Get-Content -LiteralPath $log -Raw) -match 'tray icon ready' } "$ui tray registration"
        $controller = [TrayRegressionWin]::Find([uint32]$p.Id, 'DeskTuk.Controller')
        $cls = if ($ui -eq 'circles') { 'DeskTuk.Circle' } else { 'DeskTuk.Node' }
        $window = [TrayRegressionWin]::Find([uint32]$p.Id, $cls)
        if ($controller -eq [IntPtr]::Zero -or $window -eq [IntPtr]::Zero) { throw 'fixture windows missing' }
        [void][TrayRegressionWin]::ShowWindow($window, 0)
        [void][TrayRegressionWin]::PostMessage($controller, 0x8005, [IntPtr]0, [IntPtr]0x10400)
        Wait-Check { [TrayRegressionWin]::IsWindowVisible($window) -and [TrayRegressionWin]::InsideWorkArea($window) } "$ui mouse tray recovery"
        [void][TrayRegressionWin]::ShowWindow($window, 0)
        [void][TrayRegressionWin]::PostMessage($controller, 0x8005, [IntPtr]0, [IntPtr]0x10401)
        Wait-Check { [TrayRegressionWin]::IsWindowVisible($window) } "$ui keyboard tray recovery"
        $expectTopmost=$ui -eq 'circles' -and $Topmost
        if ((([TrayRegressionWin]::GetWindowLong($window, -20) -band 8) -ne 0) -ne $expectTopmost) { throw 'Tray reveal changed the configured window band' }
        Write-Host "$ui tray reveal respects the configured window band : PASS"
        [void][TrayRegressionWin]::ShowWindow($window, 0)
        $second = Start-Fixture $ui (Join-Path $root "$ui-second.log")
        $processes += $second
        if (-not $second.WaitForExit(8000) -or $second.ExitCode -ne 0) { throw 'second launch did not exit successfully' }
        Wait-Check { [TrayRegressionWin]::IsWindowVisible($window) } "$ui second launch recovery"
        if (-not [TrayRegressionWin]::DeleteIcon($controller)) { throw 'fixture tray removal failed' }
        $count = ([regex]::Matches((Get-Content -LiteralPath $log -Raw), 'tray icon ready')).Count
        $taskbarCreated = [TrayRegressionWin]::RegisterWindowMessage('TaskbarCreated')
        [void][TrayRegressionWin]::PostMessage($controller, $taskbarCreated, [IntPtr]0, [IntPtr]0)
        Wait-Check { ([regex]::Matches((Get-Content -LiteralPath $log -Raw), 'tray icon ready')).Count -gt $count } "$ui TaskbarCreated tray re-registration"
        [void][TrayRegressionWin]::PostMessage($controller, 0x10, [IntPtr]0, [IntPtr]0)
        if (-not $p.WaitForExit(8000) -or $p.ExitCode -ne 0) { throw 'graceful exit failed' }
        if ([TrayRegressionWin]::DeleteIcon($controller)) { throw 'tray icon remained after exit' }
        Write-Host "$ui graceful shutdown and tray cleanup : PASS"
    }
    $hideAfter = (Get-ItemProperty -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced' -ErrorAction SilentlyContinue).HideIcons
    $runAfter = (Get-ItemProperty -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -ErrorAction SilentlyContinue).DeskTuk
    if ($fileHash -ne (Get-FileHash -LiteralPath $file).Hash -or $configHash -ne (Get-FileHash -LiteralPath $cfg).Hash -or $hideBefore -ne $hideAfter -or $runBefore -ne $runAfter) { throw 'fixture files, config or desktop/startup preference changed' }
    Write-Host 'References, config, desktop visibility and real startup value preserved : PASS'
} finally {
    foreach ($p in $processes) { if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force } }
    $resolved = (Resolve-Path -LiteralPath $root).Path
    if (-not $resolved.StartsWith($tempBase, [StringComparison]::OrdinalIgnoreCase) -or (Split-Path $resolved -Leaf) -notlike 'desktuk-tray-regression-*') { throw 'unsafe fixture cleanup path' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
