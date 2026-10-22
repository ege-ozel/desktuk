param([string]$Exe, [ValidateSet(0,2,3,4)][int]$Grid = 0, [switch]$Topmost, [string]$PreviewDir)
$ErrorActionPreference = 'Stop'
function Save-DockPreview([IntPtr]$window,[string]$name) {
    if (-not $PreviewDir) { return }
    Add-Type -AssemblyName System.Drawing
    $null=New-Item -ItemType Directory -Path $PreviewDir -Force
    [void][OpenCircleDragWin]::SendMessage($window,0xF,[IntPtr]::Zero,[IntPtr]::Zero)
    $r=[OpenCircleDragWin]::Rectangle($window)
    $bitmap=New-Object Drawing.Bitmap ($r.right-$r.left),($r.bottom-$r.top)
    $graphics=[Drawing.Graphics]::FromImage($bitmap)
    $dc=$graphics.GetHdc()
    try { [void][OpenCircleDragWin]::PrintWindow($window,$dc,2) }
    finally { $graphics.ReleaseHdc($dc); $graphics.Dispose() }
    try { $bitmap.Save((Join-Path $PreviewDir $name),[Drawing.Imaging.ImageFormat]::Png) }
    finally { $bitmap.Dispose() }
}
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
    [DllImport("user32.dll")] public static extern int GetSystemMetrics(int index);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
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
    public static void MiddleMouse(IntPtr h,uint message,int x,int y) { SendMessage(h,message,new IntPtr(message==0x200 ? 0x10 : 0),Point(x,y)); }
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
    # Begin routed interaction immediately: the real pointer stays outside this
    # isolated fixture and can legitimately dismiss an idle ring during a sleep.
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
    # Route only messages to the isolated dock: collapse must leave the visibility
    # square usable, and the existing tray/show action must restore the full menu.
    $dock=[OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Dock',[int]::MinValue,0)
    Save-DockPreview $dock "expanded.png"
    $dockRect=[OpenCircleDragWin]::Rectangle($dock)
    $dpi=[OpenCircleDragWin]::GetDpiForSystem()
    $arrowX=$dockRect.right-$dockRect.left-[Math]::Round(18*$dpi/96)
    $arrowY=[Math]::Round(20*$dpi/96)
    [OpenCircleDragWin]::Mouse($dock,0x202,$arrowX,$arrowY)
    Save-DockPreview $dock "collapsed.png"
    $collapsed=[OpenCircleDragWin]::Rectangle($dock)
    $side=[Math]::Floor((64*$dpi+48)/96)
    if ($collapsed.right-$collapsed.left -ne $side -or $collapsed.bottom-$collapsed.top -ne $side) { throw 'Collapsed dock must contain only the padded square' }
    $anchor=[OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Circle',$recentX,$sourceY)
    if (-not [OpenCircleDragWin]::IsWindowVisible($anchor)) { throw 'Collapsing menu changed circle visibility' }
    [OpenCircleDragWin]::Mouse($dock,0x202,$side/2,$side/2)
    if ([OpenCircleDragWin]::IsWindowVisible($anchor)) { throw 'Collapsed visibility square did not hide circles' }
    [void][OpenCircleDragWin]::SendMessage($dock,0x2A3,[IntPtr]::Zero,[IntPtr]::Zero)
    [OpenCircleDragWin]::Mouse($dock,0x202,$side/2,$side/2)
    if (-not [OpenCircleDragWin]::IsWindowVisible($anchor)) { throw 'Collapsed visibility square did not show circles' }
    # Move the collapsed tile along its detected taskbar rail; a drag must not
    # toggle circles, take focus or run the dwell action while held.
    $beforeDock=[OpenCircleDragWin]::Rectangle($dock)
    $fg=[OpenCircleDragWin]::GetForegroundWindow()
    $vertical=[Math]::Max($work.left,[OpenCircleDragWin]::GetSystemMetrics(0)-$work.right) -gt [Math]::Max($work.top,[OpenCircleDragWin]::GetSystemMetrics(1)-$work.bottom)
    $moveX=if ($vertical) { 0 } else { -120 }
    $moveY=if ($vertical) { 80 } else { 0 }
    [OpenCircleDragWin]::Mouse($dock,0x201,$side/2,$side/2)
    [OpenCircleDragWin]::Mouse($dock,0x200,($side/2+$moveX),($side/2+$moveY))
    [void][OpenCircleDragWin]::SendMessage($dock,0x113,[IntPtr]1,[IntPtr]::Zero)
    if (-not [OpenCircleDragWin]::IsWindowVisible($anchor)) { throw 'Menu drag fired the visibility hover timer' }
    [OpenCircleDragWin]::Mouse($dock,0x202,$side/2,$side/2)
    $movedDock=[OpenCircleDragWin]::Rectangle($dock)
    if (($vertical -and ($movedDock.left -ne $beforeDock.left -or $movedDock.top -eq $beforeDock.top)) -or (-not $vertical -and ($movedDock.top -ne $beforeDock.top -or $movedDock.left -eq $beforeDock.left))) { throw 'Collapsed menu did not move along its taskbar rail' }
    if ([OpenCircleDragWin]::GetForegroundWindow() -ne $fg) { throw 'Moving menu took focus' }
    if (-not [OpenCircleDragWin]::IsWindowVisible($anchor) -or (Get-Content -LiteralPath $cfg -Raw) -notmatch '(?m)^dock_pos=') { throw ('Menu drag changed visibility or failed to persist; visible='+[OpenCircleDragWin]::IsWindowVisible($anchor)+' saved='+(Get-Content -LiteralPath $cfg -Raw)) }
    Write-Host 'Collapsed DeskTuk drag, rail constraint, hover suppression and persistence : PASS'
    $controller=[OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Controller',[int]::MinValue,0)
    [void][OpenCircleDragWin]::SendMessage($controller,0x8006,[IntPtr]::Zero,[IntPtr]::Zero)
    $restored=[OpenCircleDragWin]::Rectangle($dock)
    if ($restored.right-$restored.left -ne $dockRect.right-$dockRect.left -or $restored.bottom-$restored.top -ne $dockRect.bottom-$dockRect.top) { throw 'Show action did not restore the menu' }
    $beforeHeading=[OpenCircleDragWin]::Rectangle($dock)
    $savedDock=(Get-Content -LiteralPath $cfg | Where-Object { $_ -like 'dock_pos=*' })
    [OpenCircleDragWin]::Mouse($dock,0x201,[Math]::Round(60*$dpi/96),[Math]::Round(20*$dpi/96))
    $headerDx=if ($vertical) { 0 } else { -40 }
    $headerDy=if ($vertical) { -40 } else { 0 }
    [OpenCircleDragWin]::Mouse($dock,0x200,([Math]::Round(60*$dpi/96)+$headerDx),([Math]::Round(20*$dpi/96)+$headerDy))
    $headingMoved=[OpenCircleDragWin]::Rectangle($dock)
    if ($headingMoved.left -eq $beforeHeading.left -and $headingMoved.top -eq $beforeHeading.top) { throw 'Expanded DeskTuk heading could not be dragged' }
    [void][OpenCircleDragWin]::SendMessage($dock,0x1F,[IntPtr]::Zero,[IntPtr]::Zero)
    $headingRestored=[OpenCircleDragWin]::Rectangle($dock)
    if ($headingRestored.left -ne $beforeHeading.left -or $headingRestored.top -ne $beforeHeading.top -or $savedDock -ne (Get-Content -LiteralPath $cfg | Where-Object { $_ -like 'dock_pos=*' })) { throw 'Cancelling heading drag did not restore the saved position' }
    Write-Host 'Expanded heading drag and cancellation restore saved position : PASS'
    Write-Host 'Compact dock collapse, visibility square and menu recovery : PASS'
    # Lock both native click and hover routes without moving the physical pointer.
    $lockX=[Math]::Round(114*$dpi/96); $lockY=[Math]::Round(20*$dpi/96)
    $tileX=[Math]::Round(76*$dpi/96); $tileY=[Math]::Round(68*$dpi/96)
    $anchor=[OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Circle',$musicX,$sourceY)
    [OpenCircleDragWin]::Mouse($dock,0x202,$lockX,$lockY)
    Save-DockPreview $dock 'locked.png'
    if ((Get-Content -LiteralPath $cfg -Raw) -notmatch '(?m)^dock_locked=1') { throw 'Lock did not persist' }
    [OpenCircleDragWin]::Mouse($dock,0x202,$tileX,$tileY)
    [OpenCircleDragWin]::Mouse($dock,0x200,$tileX,$tileY)
    [void][OpenCircleDragWin]::SendMessage($dock,0x113,[IntPtr]1,[IntPtr]::Zero)
    [OpenCircleDragWin]::Mouse($dock,0x202,[Math]::Round(30*$dpi/96),[Math]::Round(108*$dpi/96))
    [OpenCircleDragWin]::Mouse($anchor,0x200,20,20)
    if (-not [OpenCircleDragWin]::IsWindowVisible($anchor) -or [OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Ring',[int]::MinValue,0) -ne [IntPtr]::Zero) { throw 'Locked click/hover changed the visible board' }
    [OpenCircleDragWin]::Mouse($dock,0x202,$lockX,$lockY)
    [OpenCircleDragWin]::Mouse($dock,0x202,$tileX,$tileY)
    if ([OpenCircleDragWin]::IsWindowVisible($anchor)) { throw 'Unlocked visibility click did not hide circles' }
    Save-DockPreview $dock 'hidden.png'
    [OpenCircleDragWin]::Mouse($dock,0x202,$lockX,$lockY)
    [OpenCircleDragWin]::Mouse($dock,0x202,$tileX,$tileY)
    if ([OpenCircleDragWin]::IsWindowVisible($anchor)) { throw 'Locked click revealed hidden circles' }
    [OpenCircleDragWin]::Mouse($dock,0x202,$arrowX,$arrowY)
    Save-DockPreview $dock 'compact-locked.png'
    [OpenCircleDragWin]::Mouse($dock,0x202,[Math]::Round(53*$dpi/96),[Math]::Round(11*$dpi/96))
    if ((Get-Content -LiteralPath $cfg -Raw) -match '(?m)^dock_locked=1') { throw 'Compact unlock did not work' }
    [OpenCircleDragWin]::Mouse($dock,0x202,$side/2,$side/2)
    if (-not [OpenCircleDragWin]::IsWindowVisible($anchor)) { throw 'Compact visibility click failed after unlocking' }
    # No physical pointer movement: an already queued, cancelled dwell must be inert.
    for ($frame=0; $frame -lt 24; $frame++) {
        [void][OpenCircleDragWin]::SendMessage($dock,0x113,[IntPtr]1,[IntPtr]::Zero)
        [void][OpenCircleDragWin]::SendMessage($dock,0xF,[IntPtr]::Zero,[IntPtr]::Zero)
    }
    if (-not [OpenCircleDragWin]::IsWindowVisible($anchor)) { throw 'Stale hover timer reversed completed visibility' }
    Save-DockPreview $dock 'hover-settled.png'
    Write-Host 'Completed hover survives queued timers and repeated native paints : PASS'

    [void][OpenCircleDragWin]::SendMessage($controller,0x8006,[IntPtr]::Zero,[IntPtr]::Zero)
    Write-Host 'Lock blocks visible/hidden actions and circle hover; compact unlock works : PASS'

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
        if ($sourceX -eq $musicX) {
            foreach ($ch in @('z','z')) { [void][OpenCircleDragWin]::SendMessage($ring,0x802c,[IntPtr][int][char]$ch,[IntPtr]::Zero) }
            if ((Get-Content -LiteralPath $log -Raw) -notmatch 'ring search chars=2 results=0') { throw 'No-match query was not applied' }
            [void][OpenCircleDragWin]::SendMessage($ring,0x802c,[IntPtr]8,[IntPtr]::Zero)
            [void][OpenCircleDragWin]::SendMessage($ring,0x802c,[IntPtr]27,[IntPtr]::Zero)
            if ((Get-Content -LiteralPath $log -Raw) -notmatch 'ring search chars=0 results=24') { throw 'Escape did not restore the original page' }
        }
        $rect=[OpenCircleDragWin]::Rectangle($ring)
        $cx=($rect.right-$rect.left)/2
        $cy=($rect.bottom-$rect.top)/2
        $grabOriginX=$rect.left+$cx-$diameter/2
        $grabOriginY=$rect.top+$cy-$diameter/2
        $freeX=[int][Math]::Min($work.right-$diameter,[Math]::Max($work.left,$grabOriginX+$dx))
        $freeY=[int][Math]::Min($work.bottom-$diameter,[Math]::Max($work.top,$grabOriginY+$dy))
        $expectedX=$freeX; $expectedY=$freeY
        if ($Grid -ne 0) {
            $expectedX=Snap-Axis $freeX $work.left $work.right
            $expectedY=Snap-Axis $freeY $work.top $work.bottom
        }
        if ($sourceX -eq $recentX) { $expectedRecentX=$expectedX } else { $expectedMusicX=$expectedX }
        [OpenCircleDragWin]::MiddleMouse($ring,0x207,$cx,$cy)
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
        [OpenCircleDragWin]::MiddleMouse($anchor,0x200,$atX,$atY)
        $during=[OpenCircleDragWin]::Rectangle($anchor)
        if ($during.left -ne $freeX -or $during.top -ne $freeY) { throw 'Circle should move freely inside the work area while held' }
        # Background Desktop refresh must not restore the old saved position during a
        # live drag. The idle cleanup timer must also skip this interaction.
        [void][OpenCircleDragWin]::SendMessage($controller,0x113,[IntPtr]8,[IntPtr]::Zero)
        [void][OpenCircleDragWin]::SendMessage($controller,0x113,[IntPtr]7,[IntPtr]::Zero)
        $afterRefresh=[OpenCircleDragWin]::Rectangle($anchor)
        if ($afterRefresh.left -ne $freeX -or $afterRefresh.top -ne $freeY) { throw 'Background refresh reset a moving circle to its saved position' }
        if ((Get-Content -LiteralPath $log -Raw) -notmatch 'idle cleanup deferred during circle interaction') { throw 'Idle cleanup did not defer during dragging' }

        [OpenCircleDragWin]::MiddleMouse($anchor,0x208,$atX-$dx,$atY-$dy)
        $afterRelease=[OpenCircleDragWin]::Rectangle($anchor)
        if ($afterRelease.left -ne $expectedX -or $afterRelease.top -ne $expectedY) { throw 'Circle did not snap to the active grid cell on release' }
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
    [OpenCircleDragWin]::Mouse($ring,0x202,$cx,$cy)
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
    [OpenCircleDragWin]::Mouse($dock,0x202,$lockX,$lockY)
    $controller=[OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Controller',[int]::MinValue,0)
    [void][OpenCircleDragWin]::PostMessage($controller,0x10,[IntPtr]::Zero,[IntPtr]::Zero)
    if (-not $p.WaitForExit(8000) -or $p.ExitCode -ne 0) { throw 'Fixture did not exit cleanly' }
    $p=Start-Fixture (Join-Path $root 'restart.log')
    $processes+=$p
    Wait-Check { [OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Circle',$expectedRecentX,$expectedY) -ne [IntPtr]::Zero -and [OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Circle',$expectedMusicX,$expectedY) -ne [IntPtr]::Zero } 'Moved positions survive restart'
    $dock=[OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Dock',[int]::MinValue,0)
    $r=[OpenCircleDragWin]::Rectangle($dock)
    [OpenCircleDragWin]::Mouse($dock,0x202,($r.right-$r.left-[Math]::Round(18*$dpi/96)),[Math]::Round(20*$dpi/96))
    $restoredDock=[OpenCircleDragWin]::Rectangle($dock)
    if ($restoredDock.left -ne $movedDock.left -or $restoredDock.top -ne $movedDock.top) { throw 'Collapsed DeskTuk position did not survive restart' }
    $anchor=[OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Circle',$expectedMusicX,$expectedY)
    [OpenCircleDragWin]::Mouse($dock,0x202,$side/2,$side/2)
    [OpenCircleDragWin]::Mouse($anchor,0x200,20,20)
    if (-not [OpenCircleDragWin]::IsWindowVisible($anchor) -or [OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Ring',[int]::MinValue,0) -ne [IntPtr]::Zero) { throw 'Restart did not preserve the lock' }
    [OpenCircleDragWin]::Mouse($dock,0x202,[Math]::Round(53*$dpi/96),[Math]::Round(11*$dpi/96))
    Write-Host 'DeskTuk rail position and lock survive restart : PASS'
    $hashesAfter=@($files | ForEach-Object { (Get-FileHash -LiteralPath $_).Hash })
    if (@(Compare-Object $hashes $hashesAfter).Count -ne 0) { throw 'Referenced fixture files changed' }
    Write-Host 'All file references preserved : PASS'
    Write-Host 'Mouse movement/clicks were routed messages; the physical cursor was not moved.'
} catch {
    if (Test-Path -LiteralPath $log) {
        Get-Content -LiteralPath $log | Select-String -Pattern 'display:|ring|circle press|drag capture|drag foreground|foreground ->|app shown|app hidden' |
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
