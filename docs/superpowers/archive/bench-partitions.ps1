<#
.SYNOPSIS
  Native Windows comparison of 300 identical fixture paths in six partitions of 50.
  v1 only supports one panel per process; v2 only one open ring per process. Measure
  both six-vs-six processes and the shipped six-panels-vs-one-board arrangement.
  Only owned fixture windows receive messages/resizes. No mouse/keyboard, Win+D,
  Explorer restart, desktop preference changes, or user file changes.
  Results/configs/logs/fixture hashes are retained in a unique TEMP directory.
#>
param(
    [Parameter(Mandatory = $true)][string]$V1Exe,
    [Parameter(Mandatory = $true)][string]$V2Exe,
    [ValidateRange(1, 10)][int]$Runs = 3,
    [ValidateRange(1, 60)][int]$IdleSeconds = 15
)
$ErrorActionPreference = 'Stop'
$v1 = (Resolve-Path -LiteralPath $V1Exe).Path
$v2 = (Resolve-Path -LiteralPath $V2Exe).Path
$root = Join-Path $env:TEMP ('desktuk-partitions-' + [guid]::NewGuid())
$files = Join-Path $root 'files'
New-Item -ItemType Directory -Path $files | Out-Null
Copy-Item -LiteralPath $v1 -Destination (Join-Path $root 'DeskTuk-v1.exe')
Copy-Item -LiteralPath $v2 -Destination (Join-Path $root 'DeskTuk-v2.exe')
$v1 = Join-Path $root 'DeskTuk-v1.exe'
$v2 = Join-Path $root 'DeskTuk-v2.exe'
$children = [Collections.Generic.List[object]]::new()
$samples = [Collections.Generic.List[object]]::new()

Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class PartitionBenchWin {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int left, top, right, bottom; }
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out Rect r);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out Rect r);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll")] public static extern bool EnableWindow(IntPtr h, bool enabled);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int w, int height, uint flags);
    [DllImport("user32.dll")] public static extern bool InvalidateRect(IntPtr h, IntPtr rect, bool erase);
    [DllImport("user32.dll")] public static extern bool UpdateWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr process, uint kind);
    [DllImport("user32.dll")] static extern bool SystemParametersInfo(uint action, uint param, ref Rect rect, uint flags);
    public static Rect Work() {
        var rect = new Rect();
        if (!SystemParametersInfo(0x30, 0, ref rect, 0)) throw new InvalidOperationException("Work area unavailable");
        return rect;
    }
    public static IntPtr Find(uint pid, string cls, int x = int.MinValue) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h,l) => {
            uint p; GetWindowThreadProcessId(h, out p);
            if (p != pid) return true;
            var name = new StringBuilder(64); GetClassName(h, name, 64);
            Rect r; GetWindowRect(h, out r);
            if (name.ToString() == cls && (x == int.MinValue || r.left == x)) { found = h; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
    public static Rect Bounds(IntPtr h) { Rect r; if (!GetWindowRect(h,out r)) throw new InvalidOperationException("Window gone"); return r; }
    public static Rect Client(IntPtr h) { Rect r; if (!GetClientRect(h,out r)) throw new InvalidOperationException("Window gone"); return r; }
    public static void AssertOwner(IntPtr h, uint pid) {
        uint p; GetWindowThreadProcessId(h, out p);
        if (h == IntPtr.Zero || p != pid) throw new InvalidOperationException("Not an owned fixture window");
    }
    public static void DisableAll(uint pid) {
        EnumWindows((h,l) => { uint p; GetWindowThreadProcessId(h,out p); if (p == pid) EnableWindow(h,false); return true; },IntPtr.Zero);
    }
}
'@
$previousDpiContext = [PartitionBenchWin]::SetThreadDpiAwarenessContext([IntPtr](-4))
$work = [PartitionBenchWin]::Work()
$workW = $work.right - $work.left
$workH = $work.bottom - $work.top
# The measured desktop has 96 DPI and 48-pixel icons. The native ring is 722 px.
# Stagger rows enough to fit the available work area, with >=4 px between discs.
$diameter = 722
$spacing = $diameter + 4
$rowOffset = [int][math]::Min($spacing / 2, $workW - ($diameter + 2 * $spacing) - 16)
if ($rowOffset -lt 0) { throw 'Three 48-pixel-icon rings do not fit this work area' }
$rowStep = [int][math]::Ceiling([math]::Sqrt($spacing * $spacing - $rowOffset * $rowOffset))
$hexH = $diameter + $rowStep
$hexW = $diameter + 2 * $spacing + $rowOffset
if ($workW -lt $hexW -or $workH -lt $hexH) { throw "Six equal 48-pixel-icon rings require ${hexW}x${hexH}; work area is ${workW}x${workH}. No sample is valid." }
$hexLeft = $work.left + [int][math]::Floor(($workW - $hexW) / 2)
$hexTop = $work.top + [int][math]::Floor(($workH - $hexH) / 2)
$positions = @(for ($i = 0; $i -lt 6; $i++) {
    $row = [int][math]::Floor($i / 3)
    $column = $i % 3
    [pscustomobject]@{
        X = $hexLeft + $column * $spacing + $row * $rowOffset + $diameter / 2
        Y = $hexTop + $row * $rowStep + $diameter / 2
    }
})

# Equal, valid small files: 50 per type, distributed across every 50-file partition.
$extensions = @('txt', 'pdf', 'png', 'wav', 'zip', 'lnk')
$png = [Convert]::FromBase64String('iVBORw0KGgoAAAANSUhEUgAAADAAAAAwCAYAAABXAvmHAAAAZElEQVR4nO3PMQ0AIADAMAwhEZMYAhEcDcmO/d2Ya5+fGxrQgAY0oAENaEADGtCABjSgAQ1oQAMa0IAGNKABDWhAAxrQgAY0oAENaEADGtCABjSgAQ1oQAMa0IAGNKABDWjAaxcGR1RLxuBOIQAAAABJRU5ErkJggg==')
$pdfBody = "%PDF-1.4`n"
$pdfOffsets = [Collections.Generic.List[int]]::new()
$pdfObjects = @('<< /Type /Catalog /Pages 2 0 R >>', '<< /Type /Pages /Kids [3 0 R] /Count 1 >>', '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>')
for ($i = 0; $i -lt $pdfObjects.Count; $i++) {
    $pdfOffsets.Add([Text.Encoding]::ASCII.GetByteCount($pdfBody))
    $pdfBody += "$($i + 1) 0 obj`n$($pdfObjects[$i])`nendobj`n"
}
$xref = [Text.Encoding]::ASCII.GetByteCount($pdfBody)
$pdfBody += "xref`n0 4`n0000000000 65535 f `n"
foreach ($offset in $pdfOffsets) { $pdfBody += ('{0:D10} 00000 n ' -f $offset) + "`n" }
$pdfBody += "trailer`n<< /Size 4 /Root 1 0 R >>`nstartxref`n$xref`n%%EOF`n"
$wavStream = [IO.MemoryStream]::new()
$wavWriter = [IO.BinaryWriter]::new($wavStream)
$wavWriter.Write([Text.Encoding]::ASCII.GetBytes('RIFF')); $wavWriter.Write([int]38)
$wavWriter.Write([Text.Encoding]::ASCII.GetBytes('WAVEfmt ')); $wavWriter.Write([int]16)
$wavWriter.Write([int16]1); $wavWriter.Write([int16]1); $wavWriter.Write([int]8000)
$wavWriter.Write([int]16000); $wavWriter.Write([int16]2); $wavWriter.Write([int16]16)
$wavWriter.Write([Text.Encoding]::ASCII.GetBytes('data')); $wavWriter.Write([int]2); $wavWriter.Write([int16]0)
$wavBytes = $wavStream.ToArray(); $wavWriter.Dispose(); $wavStream.Dispose()
$zip = [byte[]](0x50, 0x4b, 0x05, 0x06) + [byte[]]::new(18)
$shortcutTargets = @('notepad.exe','cmd.exe','control.exe','taskmgr.exe','WindowsPowerShell/v1.0/powershell.exe') |
    ForEach-Object { Join-Path (Join-Path $env:WINDIR 'System32') $_ } | Where-Object { Test-Path -LiteralPath $_ }
if ($shortcutTargets.Count -eq 0) { throw 'No local shortcut fixture targets exist' }
$wsh = New-Object -ComObject WScript.Shell
$paths = @(for ($i = 0; $i -lt 300; $i++) {
    $ext = $extensions[$i % 6]
    $path = Join-Path $files ('file-{0:D3}.{1}' -f ($i + 1), $ext)
    switch ($ext) {
        txt { [IO.File]::WriteAllText($path, 'DeskTuk benchmark fixture', [Text.Encoding]::UTF8) }
        pdf { [IO.File]::WriteAllText($path, $pdfBody, [Text.Encoding]::ASCII) }
        png { [IO.File]::WriteAllBytes($path, $png) }
        wav { [IO.File]::WriteAllBytes($path, $wavBytes) }
        zip { [IO.File]::WriteAllBytes($path, $zip) }
        lnk {
            $shortcut = $wsh.CreateShortcut($path)
            $shortcut.TargetPath = $shortcutTargets[[int][math]::Floor($i / 6) % $shortcutTargets.Count]
            $shortcut.Save()
            [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($shortcut)
        }
    }
    $path
})
[void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($wsh)
$manifest = @($paths | ForEach-Object { Get-FileHash -LiteralPath $_ -Algorithm SHA256 | Select-Object Path,Hash })
$manifest | Export-Csv -LiteralPath (Join-Path $root 'fixture-manifest.csv') -NoTypeInformation
$metadata = [ordered]@{
    FixtureRoot = $root; Runs = $Runs; IdleSeconds = $IdleSeconds; StoredItems = 300
    Partitions = 6; ItemsPerPartition = 50; MatchedVisibleItems = 120; IconPixels = 48
    WorkArea = $work; V1SHA256 = (Get-FileHash -LiteralPath $v1).Hash; V2SHA256 = (Get-FileHash -LiteralPath $v2).Hash
    OS = (Get-CimInstance Win32_OperatingSystem | Select-Object Caption,Version,BuildNumber)
    CPU = (Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,NumberOfLogicalProcessors)
    Types = $extensions; SourceV1 = '39ef8a9'; SourceV2 = 'f301326'
    MutationPolicy = 'Only unique TEMP fixtures/configs and owned benchmark windows. User app is left running.'
}
$metadata | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $root 'metadata.json') -Encoding utf8

function Wait-Ready([scriptblock]$condition, [string]$reason) {
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    while (-not (& $condition)) {
        if ([DateTime]::UtcNow -gt $deadline) { throw $reason }
        Start-Sleep -Milliseconds 100
    }
}
function Read-Log($fixture) {
    if (Test-Path -LiteralPath $fixture.Log) { Get-Content -LiteralPath $fixture.Log -Raw -Encoding utf8 } else { '' }
}
function Loaded-Count($fixture) { ([regex]::Matches((Read-Log $fixture), 'item loaded path=')).Count }
function Get-CPU($fixtures) {
    $sum = 0.0
    foreach ($f in $fixtures) { $f.Process.Refresh(); if ($f.Process.HasExited) { throw 'Fixture process exited' }; $sum += $f.Process.TotalProcessorTime.TotalMilliseconds }
    $sum
}
function Start-Fixture([int]$run, [string]$kind, [int]$group, [bool]$singleBoard = $false) {
    $profile = Join-Path $root "run-$run/$kind-$group"
    New-Item -ItemType Directory -Path (Join-Path $profile 'DeskTuk') -Force | Out-Null
    $isPanel = $kind -in @('v1', 'v2-panel')
    $cfg = "host=window`nui=$(if ($isPanel) { 'panel' } else { 'circles' })`n"
    if ($isPanel) {
        $panelX = $work.left + [int][math]::Floor(($workW - (6 * 368 + 5 * 16)) / 2) + $group * 384
        $panelY = $work.top + [int][math]::Floor(($workH - 1241) / 2)
        $cfg += "x=$panelX`ny=$panelY`n"
        foreach ($path in $paths[($group * 50)..($group * 50 + 49)]) { $cfg += "item=$path`n" }
    } else {
        $cfg += "recent=$($work.right - 88)|$($work.bottom - 88)`n"
        $groups = if ($singleBoard) { 0..5 } else { @($group) }
        foreach ($g in $groups) {
            $cfg += "circle=Bench $g|$($positions[$g].X - 44)|$($positions[$g].Y - 44)`n"
            foreach ($path in $paths[($g * 50)..($g * 50 + 49)]) { $cfg += "citem=Bench $g||$path`n" }
        }
    }
    [IO.File]::WriteAllText((Join-Path $profile 'DeskTuk/desktuk.cfg'), $cfg, [Text.UTF8Encoding]::new($false))
    $log = Join-Path $profile 'app.log'
    $oldAppData = $env:APPDATA
    try {
        $env:APPDATA = $profile
        $binary = if ($kind -eq 'v1') { $v1 } else { $v2 }
        $process = Start-Process -FilePath $binary -WindowStyle Hidden -PassThru -ArgumentList @('--no-save','--allow-multiple','--log',('"' + $log + '"'))
    } finally { $env:APPDATA = $oldAppData }
    $children.Add($process)
    $fixture = [pscustomobject]@{ Process = $process; Log = $log; Kind = $kind; Panel = $isPanel; Group = $group; SingleBoard = $singleBoard }
    $cls = if ($isPanel) { 'DeskTuk.Node' } else { 'DeskTuk.Circle' }
    Wait-Ready { [PartitionBenchWin]::Find([uint32]$process.Id, $cls) -ne [IntPtr]::Zero } 'Fixture did not create a window'
    $handle = [PartitionBenchWin]::Find([uint32]$process.Id, $cls)
    if ([PartitionBenchWin]::GetDpiForWindow($handle) -ne 96) { throw 'This matched layout requires 96 DPI; no OS settings were changed' }
    [PartitionBenchWin]::DisableAll([uint32]$process.Id)
    if ($isPanel) {
        Wait-Ready { (Loaded-Count $fixture) -eq 50 -and (Read-Log $fixture).Contains('METRIC startup first_paint') } 'v1 did not load and paint all 50 items'
    } else {
        Wait-Ready { (Read-Log $fixture).Contains('METRIC circles_started') } 'v2 board not ready'
    }
    $fixture
}
function Ring-Handle($fixture) { [PartitionBenchWin]::Find([uint32]$fixture.Process.Id, 'DeskTuk.Ring') }
function Open-Ring($fixture, [int]$group) {
    $anchor = [PartitionBenchWin]::Find([uint32]$fixture.Process.Id, 'DeskTuk.Circle', [int]($positions[$group].X - 44))
    [PartitionBenchWin]::AssertOwner($anchor, [uint32]$fixture.Process.Id)
    $before = Loaded-Count $fixture
    [void][PartitionBenchWin]::PostMessage($anchor, 0x02A1, [IntPtr]::Zero, [IntPtr]::Zero)
    Wait-Ready { (Ring-Handle $fixture) -ne [IntPtr]::Zero -and (Loaded-Count $fixture) -ge ($before + 20) } 'v2 first page did not load'
    $entries = [regex]::Matches((Read-Log $fixture), 'ring opened target=Circle\((\d+)\) slots=50 icon_px=(\d+) per_page=(\d+) diameter=(\d+)')
    if ($entries.Count -eq 0) { throw 'Wrong ring or item count' }
    $entry = $entries[$entries.Count - 1]
    $expectedTarget = if ($fixture.SingleBoard) { $group } else { 0 }
    if ([int]$entry.Groups[1].Value -ne $expectedTarget) { throw 'A different partition opened' }
    if ([int]$entry.Groups[2].Value -ne 48 -or [int]$entry.Groups[3].Value -ne 20 -or [int]$entry.Groups[4].Value -ne $diameter) { throw 'Icon size/page geometry does not match the controlled comparison' }
    [PartitionBenchWin]::DisableAll([uint32]$fixture.Process.Id)
    Start-Sleep -Milliseconds 400
}
function Page-Ring($fixture, [int]$expectedNew) {
    $ring = Ring-Handle $fixture
    [PartitionBenchWin]::AssertOwner($ring, [uint32]$fixture.Process.Id)
    $before = Loaded-Count $fixture
    $r = [PartitionBenchWin]::Client($ring)
    $x = [int](($r.right - $r.left) / 2); $y = [int](($r.bottom - $r.top) / 2)
    [void][PartitionBenchWin]::PostMessage($ring, 0x0202, [IntPtr]::Zero, [IntPtr](($y -shl 16) -bor $x))
    Wait-Ready { (Loaded-Count $fixture) -ge ($before + $expectedNew) } 'Page did not finish its metadata requests'
    Start-Sleep -Milliseconds 400
}
function Browse-Partition($fixture) { Page-Ring $fixture 20; Page-Ring $fixture 10; Page-Ring $fixture 20 }
function Close-Ring($fixture) {
    $ring = Ring-Handle $fixture
    if ($ring -eq [IntPtr]::Zero) { return }
    [PartitionBenchWin]::AssertOwner($ring, [uint32]$fixture.Process.Id)
    [void][PartitionBenchWin]::PostMessage($ring, 0x02A3, [IntPtr]::Zero, [IntPtr]::Zero)
    Wait-Ready { (Ring-Handle $fixture) -eq [IntPtr]::Zero } 'Normal ring-close path did not complete'
}
function Assert-Views($fixtures, [int]$visible) {
    $count = 0
    $rings = [Collections.Generic.List[object]]::new()
    foreach ($f in $fixtures) {
        if ($f.Panel) {
            $h = [PartitionBenchWin]::Find([uint32]$f.Process.Id, 'DeskTuk.Node')
            $r = [PartitionBenchWin]::Client($h)
            if ($r.right - $r.left -ne 368) { throw 'Unexpected panel width' }
            if ($r.bottom - $r.top -eq 1241) { $count += 50 } elseif ($r.bottom - $r.top -eq 505) { $count += 20 } else { throw 'Unexpected panel viewport height' }
        } else {
            $h = Ring-Handle $f
            if ($h -ne [IntPtr]::Zero) {
                $r = [PartitionBenchWin]::Bounds($h)
                $rings.Add($r)
                if ($r.right - $r.left -ne $diameter -or $r.bottom - $r.top -ne $diameter) { throw 'Unexpected ring size' }
                $count += 20
                if ($r.left -lt $work.left -or $r.top -lt $work.top -or $r.right -gt $work.right -or $r.bottom -gt $work.bottom) { throw 'A ring extends outside the work area' }
            }
        }
    }
    for ($i = 0; $i -lt $rings.Count; $i++) {
        for ($j = $i + 1; $j -lt $rings.Count; $j++) {
            $dx = $rings[$i].left - $rings[$j].left; $dy = $rings[$i].top - $rings[$j].top
            if ($dx * $dx + $dy * $dy -lt $diameter * $diameter) { throw 'Opaque ring discs overlap' }
        }
    }
    if ($count -ne $visible) { throw "Expected $visible viewport items; found $count" }
}
function Measure-Case([int]$run, [string]$name, $fixtures, [int]$visible, [double]$beforeCPU) {
    Start-Sleep -Seconds 2
    Assert-Views $fixtures $visible
    $cpu = Get-CPU $fixtures
    $commit = 0.0; $gdi = 0; $userObjects = 0; $handles = 0
    foreach ($f in $fixtures) {
        $f.Process.Refresh(); $commit += $f.Process.PrivateMemorySize64 / 1KB
        $gdi += [PartitionBenchWin]::GetGuiResources($f.Process.Handle, 0)
        $userObjects += [PartitionBenchWin]::GetGuiResources($f.Process.Handle, 1)
        $handles += $f.Process.HandleCount
    }
    $filter = ($fixtures | ForEach-Object { "IDProcess=$($_.Process.Id)" }) -join ' OR '
    $counters = @(Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Filter $filter)
    if ($counters.Count -ne $fixtures.Count) { throw 'Missing per-process physical memory counter' }
    $privateWS = ($counters | Measure-Object WorkingSetPrivate -Sum).Sum / 1KB
    $idleStart = Get-CPU $fixtures
    Start-Sleep -Seconds $IdleSeconds
    $idle = (Get-CPU $fixtures) - $idleStart
    Assert-Views $fixtures $visible
    $row = [pscustomobject]@{
        Run = $run; Scenario = $name; Processes = $fixtures.Count; StoredItems = 300; ViewportItems = $visible; IconPx = 48
        CommitKB = [math]::Round($commit); PrivateWsKB = [math]::Round($privateWS)
        GdiObjects = $gdi; UserObjects = $userObjects; Handles = $handles
        CumulativeCpuMs = [math]::Round($cpu, 1); PhaseCpuMs = [math]::Round($cpu - $beforeCPU, 1)
        IdleCpuMs = [math]::Round($idle, 1); IdleSeconds = $IdleSeconds
    }
    $samples.Add($row)
    $samples | Export-Csv -LiteralPath (Join-Path $root 'samples.csv') -NoTypeInformation
    Write-Host ("run {0}: {1}; {2} processes, {3} viewport items, commit={4} KB, private WS={5} KB, idle={6} ms/{7}s" -f $run,$name,$fixtures.Count,$visible,$row.CommitKB,$row.PrivateWsKB,$row.IdleCpuMs,$IdleSeconds)
}
function Stop-Fixtures($fixtures) { foreach ($f in $fixtures) { if (-not $f.Process.HasExited) { Stop-Process -Id $f.Process.Id; $f.Process.WaitForExit() } } }
function Panel-Run([int]$run, [string]$kind, [string]$label) {
    $fixtures = @(0..5 | ForEach-Object { Start-Fixture $run $kind $_ })
    try {
        Measure-Case $run "$label six panels, all 300" $fixtures 300 0
        $cpu = Get-CPU $fixtures
        for ($i = 0; $i -lt 6; $i++) {
            $h = [PartitionBenchWin]::Find([uint32]$fixtures[$i].Process.Id, 'DeskTuk.Node')
            [PartitionBenchWin]::AssertOwner($h, [uint32]$fixtures[$i].Process.Id)
            if (-not [PartitionBenchWin]::SetWindowPos($h,[IntPtr]::Zero,[int]($positions[$i].X - 184),[int]($positions[$i].Y - 252),368,505,0x14)) { throw 'Panel viewport resize failed' }
            [void][PartitionBenchWin]::InvalidateRect($h,[IntPtr]::Zero,$false)
            [void][PartitionBenchWin]::UpdateWindow($h)
        }
        Measure-Case $run "$label six panels, 120 viewport" $fixtures 120 $cpu
    } finally { Stop-Fixtures $fixtures }
}
function V2-Multi-Run([int]$run) {
    $fixtures = @(0..5 | ForEach-Object { Start-Fixture $run 'v2-multi' $_ })
    try {
        Measure-Case $run 'v2 six processes, closed' $fixtures 0 0
        $cpu = Get-CPU $fixtures
        for ($i = 0; $i -lt 6; $i++) { Open-Ring $fixtures[$i] $i }
        Measure-Case $run 'v2 six rings, first 120' $fixtures 120 $cpu
        $cpu = Get-CPU $fixtures
        foreach ($f in $fixtures) { Browse-Partition $f }
        Measure-Case $run 'v2 six rings, all 300 visited' $fixtures 120 $cpu
        $cpu = Get-CPU $fixtures
        foreach ($f in $fixtures) { Close-Ring $f }
        Measure-Case $run 'v2 six processes, warm closed' $fixtures 0 $cpu
    } finally { Stop-Fixtures $fixtures }
}
function V2-Single-Run([int]$run) {
    $fixtures = @(Start-Fixture $run 'v2-single' 0 $true)
    $f = $fixtures[0]
    try {
        Measure-Case $run 'v2 one board, six circles closed' $fixtures 0 0
        $cpu = Get-CPU $fixtures
        Open-Ring $f 0
        Measure-Case $run 'v2 one board, first 20' $fixtures 20 $cpu
        $cpu = Get-CPU $fixtures
        for ($i = 0; $i -lt 6; $i++) {
            if ($i -gt 0) { Close-Ring $f; Open-Ring $f $i }
            Browse-Partition $f
        }
        Measure-Case $run 'v2 one board, all 300 visited' $fixtures 20 $cpu
        $cpu = Get-CPU $fixtures
        Close-Ring $f
        Measure-Case $run 'v2 one board, warm closed' $fixtures 0 $cpu
    } finally { Stop-Fixtures $fixtures }
}
function Median($values) { $ordered = @($values | Sort-Object); $ordered[[int][math]::Floor(($ordered.Count - 1) / 2)] }
try {
    Write-Host "Fixtures/results: $root"
    for ($run = 1; $run -le $Runs; $run++) {
        # Alternate the first version; these are fresh-process, OS-cache-warm runs.
        if ($run % 2 -eq 1) { Panel-Run $run 'v1' 'v1'; V2-Multi-Run $run } else { V2-Multi-Run $run; Panel-Run $run 'v1' 'v1' }
        Panel-Run $run 'v2-panel' 'v2 panel'
        V2-Single-Run $run
    }
    foreach ($entry in $manifest) { if ((Get-FileHash -LiteralPath $entry.Path).Hash -ne $entry.Hash) { throw "Fixture changed: $($entry.Path)" } }
    $summary = @($samples | Group-Object Scenario | ForEach-Object {
        $group = $_.Group
        [pscustomobject]@{
            Scenario = $_.Name; Processes = $group[0].Processes; StoredItems = 300; ViewportItems = $group[0].ViewportItems; IconPx = 48
            CommitKB = Median $group.CommitKB; PrivateWsKB = Median $group.PrivateWsKB
            GdiObjects = Median $group.GdiObjects; UserObjects = Median $group.UserObjects; Handles = Median $group.Handles
            CumulativeCpuMs = Median $group.CumulativeCpuMs; PhaseCpuMs = Median $group.PhaseCpuMs; IdleCpuMs = Median $group.IdleCpuMs
        }
    })
    $summary | Export-Csv -LiteralPath (Join-Path $root 'summary.csv') -NoTypeInformation
    $summary | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $root 'summary.json') -Encoding utf8
    $summary | Format-Table -AutoSize | Out-String -Width 240
    Write-Host "All 300 fixture hashes unchanged. Retained results: $root"
} finally {
    foreach ($p in $children) { if (-not $p.HasExited) { Stop-Process -Id $p.Id } }
    [void][PartitionBenchWin]::SetThreadDpiAwarenessContext($previousDpiContext)
}
