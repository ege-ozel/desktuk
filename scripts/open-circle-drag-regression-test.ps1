param([string]$Exe, [ValidateSet(0,2,3,4)][int]$Grid = 0, [switch]$Topmost)
$ErrorActionPreference = 'Stop'
$build = (Resolve-Path -LiteralPath $Exe).Path
$tempBase = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$root = Join-Path $tempBase ('desktuk-open-circle-drag-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path (Join-Path $root 'DeskTuk')
$processes = @()

Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class OpenCircleDragWin {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int left, top, right, bottom; }
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder b, int n);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out Rect r);
    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int command);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int w, int height, uint flags);
    [DllImport("user32.dll", EntryPoint="GetWindowLongW")] public static extern int GetWindowLong(IntPtr h, int index);
    [DllImport("user32.dll")] public static extern bool SystemParametersInfo(uint action,uint param,out Rect result,uint flags);
    [DllImport("user32.dll")] public static extern uint GetDpiForSystem();
    public static Rect WorkArea() { Rect r; if (!SystemParametersInfo(0x30,0,out r,0)) throw new Exception("No work area"); return r; }
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    public static IntPtr Find(uint pid, string cls, int x, int y) {
        IntPtr result = IntPtr.Zero;
        EnumWindows((h,l) => {
            uint p; GetWindowThreadProcessId(h, out p);
            var name=new StringBuilder(64); GetClassName(h,name,64);
            Rect rect; GetWindowRect(h,out rect);
            if (p==pid && name.ToString()==cls && (x==Int32.MinValue || (rect.left==x && rect.top==y))) {
                result=h; return false;
            }
            return true;
        },IntPtr.Zero);
        return result;
    }
    public static Rect Rectangle(IntPtr h) { Rect r; if (!GetWindowRect(h,out r)) throw new Exception("Missing window"); return r; }
    public static IntPtr Point(int x,int y) { return new IntPtr(unchecked((int)((uint)(ushort)x | ((uint)(ushort)y << 16)))); }
    public static void Mouse(IntPtr h,uint message,int x,int y) { SendMessage(h,message,new IntPtr(message==0x202 ? 0 : 1),Point(x,y)); }
}
'@

function Wait-Check([scriptblock]$check,[string]$label) {
    $until=[DateTime]::UtcNow.AddSeconds(6)
    do {
        if (& $check) { Write-Host "$label : PASS"; return }
        Start-Sleep -Milliseconds 60
    } while ([DateTime]::UtcNow -lt $until)
    throw "$label : FAIL"
}

function Start-Fixture([string]$log) {
    $previousAppData=$env:APPDATA
    try {
        $env:APPDATA=$root
        Start-Process -FilePath $build -WindowStyle Hidden -PassThru -ArgumentList @('--allow-multiple','--exit-after','45000','--log',('"'+$log+'"'))
    } finally { $env:APPDATA=$previousAppData }
}

function Open-Ring($p,[IntPtr]$anchor) {
    [void][OpenCircleDragWin]::SendMessage($anchor,0x200,[IntPtr]::Zero,[OpenCircleDragWin]::Point(20,20))
    if ([OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Ring',[int]::MinValue,0) -eq [IntPtr]::Zero) {
        throw 'Pointer entry must create the ring synchronously without hover dwell'
    }
    Wait-Check { [OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Ring',[int]::MinValue,0) -ne [IntPtr]::Zero } 'Ring opens'
    Start-Sleep -Milliseconds 400
    [OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Ring',[int]::MinValue,0)
}

try {
    $cfg=Join-Path $root 'DeskTuk/desktuk.cfg'
    $work=[OpenCircleDragWin]::WorkArea()
    $diameter=[int][Math]::Ceiling(104*[OpenCircleDragWin]::GetDpiForSystem()/96)
    $recentX=$work.left+20
    $musicX=$work.left+[int][Math]::Max(40,[Math]::Floor(($work.right-$work.left-$diameter-20)/2))
    $sourceY=$work.top+20
    $dx=[int][Math]::Max(1,[Math]::Min(90,[Math]::Floor(($work.right-$work.left-$diameter-40)/4)))
    $dy=[int][Math]::Max(1,[Math]::Min(35,[Math]::Floor(($work.bottom-$work.top-$diameter-40)/4)))
    $expectedRecentX=$recentX+$dx
    $expectedMusicX=$musicX+$dx
    $expectedY=$sourceY+$dy
    if ($Grid -ne 0) {
        function Snap-Axis([int]$position,[int]$lo,[int]$hi) {
            $cellWidth=($hi-$lo)/$Grid
            $cell=[Math]::Min($Grid-1,[Math]::Max(0,[Math]::Floor(($position+$diameter/2-$lo)/$cellWidth)))
            [int][Math]::Min($hi-$diameter,[Math]::Max($lo,[Math]::Round($lo+($cell+0.5)*$cellWidth-$diameter/2,0,[MidpointRounding]::AwayFromZero)))
        }
        $expectedRecentX=Snap-Axis $expectedRecentX $work.left $work.right
        $expectedMusicX=Snap-Axis $expectedMusicX $work.left $work.right
        $expectedY=Snap-Axis $expectedY $work.top $work.bottom
    }
    $lines=@('host=window','ui=circles',"grid=$Grid","recent=$recentX|$sourceY","circle=Music|$musicX|$sourceY")
    if ($Topmost) { $lines += 'always_on_top=1' }
    $files=@()
    for ($i=0;$i -lt 24;$i++) {
        $path=Join-Path $root ("fixture-ref-{0:D2}.txt" -f $i)
        Set-Content -LiteralPath $path -Value "fixture $i" -Encoding utf8
        $files+=$path
        $lines+="citem=Music||$path"
    }
    Set-Content -LiteralPath $cfg -Value ($lines -join "`n") -Encoding utf8
    $hashes=@($files | ForEach-Object { (Get-FileHash -LiteralPath $_).Hash })
    $log=Join-Path $root 'drag.log'
    $p=Start-Fixture $log
    $processes+=$p
    Wait-Check { [OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Circle',$musicX,$sourceY) -ne [IntPtr]::Zero } 'Fixture circles created'
    foreach ($sourceX in @($recentX,$musicX)) {
        $anchor=[OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Circle',$sourceX,$sourceY)
        [void][OpenCircleDragWin]::ShowWindow($anchor,0)
        if (-not [OpenCircleDragWin]::IsWindowVisible($anchor) -or ([OpenCircleDragWin]::GetWindowLong($anchor,-20) -band 8 -eq 0)) { throw 'Show Desktop hide must not hide a topmost circle' }
        [void][OpenCircleDragWin]::SendMessage($anchor,0x112,[IntPtr]0xF020,[IntPtr]::Zero)
        if ([OpenCircleDragWin]::IsIconic($anchor)) { throw 'Topmost circle accepted shell minimization' }
        [void][OpenCircleDragWin]::SetWindowPos($anchor,[IntPtr]1,0,0,0,0,0x13)
        if (([OpenCircleDragWin]::GetWindowLong($anchor,-20) -band 8) -eq 0) { throw 'Shell ordering demoted a topmost circle' }
        $ring=Open-Ring $p $anchor | Select-Object -Last 1
        foreach ($surface in @($anchor,$ring)) {
            $surfaceStyle=[OpenCircleDragWin]::GetWindowLong($surface,-20)
            if (($surfaceStyle -band 8) -eq 0) { throw "Circle and owned ring must remain topmost: hwnd=$surface style=$surfaceStyle" }
        }
        $rect=[OpenCircleDragWin]::Rectangle($ring)
        $cx=($rect.right-$rect.left)/2
        $cy=($rect.bottom-$rect.top)/2
        [OpenCircleDragWin]::Mouse($ring,0x201,$cx,$cy)
        $guide=[OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.GridOverlay',[int]::MinValue,0)
        if (($Grid -ne 0) -ne ($guide -ne [IntPtr]::Zero)) { throw 'Grid guide visibility must match the selected grid on press' }
        if ($Grid -ne 0) {
            $guideRect=[OpenCircleDragWin]::Rectangle($guide)
            if ($guideRect.left -ne $work.left -or $guideRect.top -ne $work.top -or $guideRect.right -ne $work.right -or $guideRect.bottom -ne $work.bottom) { throw 'Grid guide must cover only the taskbar-free work area' }
        }
        # Activation/repainting can emit non-button mouse moves. They must not drag.
        [void][OpenCircleDragWin]::SendMessage($anchor,0x200,[IntPtr]::Zero,[OpenCircleDragWin]::Point(1000,-1000))
        [void][OpenCircleDragWin]::SendMessage($ring,0x200,[IntPtr]::Zero,[OpenCircleDragWin]::Point($cx+100,$cy+100))
        $unchanged=[OpenCircleDragWin]::Rectangle($anchor)
        if ($unchanged.left -ne $sourceX -or $unchanged.top -ne $sourceY) { throw 'Non-button movement advanced the centre drag' }
        [void][OpenCircleDragWin]::SendMessage($ring,0x2A3,[IntPtr]::Zero,[IntPtr]::Zero)
        Start-Sleep -Milliseconds 200
        if (-not [OpenCircleDragWin]::IsWindow($ring)) { throw 'Ring closed during a pending centre drag' }
        $atX=$rect.left+$cx+$dx-$sourceX
        $atY=$rect.top+$cy+$dy-$sourceY
        [OpenCircleDragWin]::Mouse($anchor,0x200,$atX,$atY)
        $during=[OpenCircleDragWin]::Rectangle($anchor)
        $expectedX=if ($sourceX -eq $recentX) { $expectedRecentX } else { $expectedMusicX }
        if ($during.left -ne $expectedX -or $during.top -ne $expectedY) { throw 'Circle did not snap to its entered cell during the drag' }
        [OpenCircleDragWin]::Mouse($anchor,0x202,$atX-$dx,$atY-$dy)
        if ([OpenCircleDragWin]::IsWindow($guide)) { throw 'Grid guide survived button release' }
        if (([OpenCircleDragWin]::GetWindowLong($anchor,-20) -band 8) -eq 0) { throw 'Capture cleanup demoted a topmost circle' }
        Wait-Check { [OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Circle',$expectedX,$expectedY) -ne [IntPtr]::Zero } "Open circle at $sourceX moves by event delta"
        if ([OpenCircleDragWin]::IsWindow($ring)) { throw 'Ring remained open during the circle move' }
        $saved=Get-Content -LiteralPath $cfg -Raw
        if ($Grid -ne 0 -and $saved -notmatch ('(?m)^grid=' + $Grid + '\r?$')) { throw 'Grid setting was not preserved' }
        $expectedLine=if ($sourceX -eq $recentX) { "recent=$expectedRecentX|$expectedY" } else { "circle=Music|$expectedMusicX|$expectedY" }
        if (-not $saved.Contains($expectedLine)) { throw 'Moved circle position was not saved' }
        Write-Host "$expectedLine persisted : PASS"
    }
    # A short centre click still turns the existing Music ring page, without reopening it.
    $anchor=[OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Circle',$expectedMusicX,$expectedY)
    $ring=Open-Ring $p $anchor | Select-Object -Last 1
    $rect=[OpenCircleDragWin]::Rectangle($ring)
    $cx=($rect.right-$rect.left)/2
    $cy=($rect.bottom-$rect.top)/2
    $opened=[regex]::Matches((Get-Content -LiteralPath $log -Raw),'ring opened target=Circle\(0\).*?per_page=(\d+)')
    $nextIndex=[int]$opened[$opened.Count-1].Groups[1].Value
    $nextFilePattern='item loaded path=.*fixture-ref-'+('{0:D2}' -f $nextIndex)+'\.txt'
    [OpenCircleDragWin]::Mouse($ring,0x201,$cx,$cy)
    [OpenCircleDragWin]::Mouse($anchor,0x202,44,44)
    Wait-Check { (Get-Content -LiteralPath $log -Raw) -match $nextFilePattern } 'Centre click loads the next page'
    if (-not [OpenCircleDragWin]::IsWindow($ring)) { throw 'Centre click destroyed the existing ring' }
    # No physical input: explicit cancellation/capture-loss messages must remove the guide.
    foreach ($cancel in @(0x1F,0x215,0x100)) {
        [OpenCircleDragWin]::Mouse($anchor,0x201,44,44)
        $guide=[OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.GridOverlay',[int]::MinValue,0)
        [void][OpenCircleDragWin]::SendMessage($anchor,$cancel,[IntPtr]27,[IntPtr]::Zero)
        if ([OpenCircleDragWin]::IsWindow($guide)) { throw 'Grid guide survived cancel/capture loss/Escape' }
    }
    Write-Host 'Grid guide press/release/cancel lifecycle : PASS'
    $ring=Open-Ring $p $anchor | Select-Object -Last 1
    [void][OpenCircleDragWin]::SendMessage($ring,0x200,[IntPtr]::Zero,[OpenCircleDragWin]::Point(0,0))
    Wait-Check { -not [OpenCircleDragWin]::IsWindow($ring) } 'Pointer outside the ring disc eases closed'
    $controller=[OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Controller',[int]::MinValue,0)
    [void][OpenCircleDragWin]::PostMessage($controller,0x10,[IntPtr]::Zero,[IntPtr]::Zero)
    if (-not $p.WaitForExit(8000) -or $p.ExitCode -ne 0) { throw 'Fixture did not exit cleanly' }
    $p=Start-Fixture (Join-Path $root 'restart.log')
    $processes+=$p
    Wait-Check { [OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Circle',$expectedRecentX,$expectedY) -ne [IntPtr]::Zero -and [OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Circle',$expectedMusicX,$expectedY) -ne [IntPtr]::Zero } 'Moved positions survive restart'
    $hashesAfter=@($files | ForEach-Object { (Get-FileHash -LiteralPath $_).Hash })
    if (@(Compare-Object $hashes $hashesAfter).Count -ne 0) { throw 'Referenced fixture files changed' }
    Write-Host 'All file references preserved : PASS'
    Write-Host 'Mouse movement/clicks were routed messages; the physical cursor was not moved.'
} catch {
    if (Test-Path -LiteralPath $log) {
        Get-Content -LiteralPath $log | Select-String -Pattern 'display:|ring opened|circle press|drag capture|drag foreground|foreground ->' |
            Select-Object -Last 30 | ForEach-Object { Write-Host $_.Line }
    }
    throw
} finally {
    foreach ($p in $processes) {
        if (-not $p.HasExited) {
            $controller=[OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Controller',[int]::MinValue,0)
            [void][OpenCircleDragWin]::PostMessage($controller,0x10,[IntPtr]::Zero,[IntPtr]::Zero)
            if (-not $p.WaitForExit(3000)) { Stop-Process -Id $p.Id -Force }
        }
    }
    $resolved=(Resolve-Path -LiteralPath $root).Path
    if (-not $resolved.StartsWith($tempBase,[StringComparison]::OrdinalIgnoreCase) -or (Split-Path $resolved -Leaf) -notlike 'desktuk-open-circle-drag-*') { throw 'Unsafe fixture cleanup path' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
