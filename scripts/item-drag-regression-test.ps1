<# Isolated TEMP config and references only; no physical input or Desktop writes. #>
param([string]$Exe = 'target/release/DeskTuk.exe')
$ErrorActionPreference='Stop'
$build=(Resolve-Path -LiteralPath $Exe).Path
$tempBase=[IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$root=Join-Path $tempBase ('desktuk-item-drag-'+[guid]::NewGuid().ToString('N'))
$null=New-Item -ItemType Directory -Path (Join-Path $root 'DeskTuk')
$processes=@()
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class ItemDragWin {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int left, top, right, bottom; }
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder b, int n);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out Rect r);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
    [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h, uint command);
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
    public static IntPtr BeginItem(uint pid, IntPtr anchor, int x, int y) {
        // File/log reads in the caller can outlast the idle dismissal animation.
        // Refresh entry and start the routed press together; never move the cursor.
        SendMessage(anchor,0x200,IntPtr.Zero,Point(20,20));
        var ring=Find(pid,"DeskTuk.Ring",Int32.MinValue,0);
        if (ring==IntPtr.Zero || GetWindow(ring,4)!=anchor) throw new Exception("Source ring not ready");
        Mouse(ring,0x201,x,y);
        Mouse(ring,0x200,x+15,y+15);
        return ring;
    }
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
        Start-Process -FilePath $build -WindowStyle Hidden -PassThru -ArgumentList @('--allow-multiple','--exit-after','60000','--log',('"'+$log+'"'))
    } finally { $env:APPDATA=$previousAppData }
}


function Slot-Point([IntPtr]$ring,[int]$index) {
    $rect=[ItemDragWin]::Rectangle($ring)
    $opened=[regex]::Matches((Get-Content -LiteralPath $log -Raw),'ring opened[^\r\n]*inner_slots=(\d+) inner_r=(\d+) outer_r=(\d+)')
    $last=$opened[$opened.Count-1]
    $inner=[int]$last.Groups[1].Value
    $radius=[int]$last.Groups[2].Value
    $capacity=[regex]::Matches($last.Value,'per_page=(\d+)')
    $total=[int]$capacity[0].Groups[1].Value
    if ($index -lt $inner) { $angle=-90+$index*360/$inner }
    else { $radius=[int]$last.Groups[3].Value; $angle=-90+($index-$inner)*360/($total-$inner) }
    @([int][Math]::Round(($rect.right-$rect.left)/2+$radius*[Math]::Cos($angle*[Math]::PI/180)),
      [int][Math]::Round(($rect.bottom-$rect.top)/2+$radius*[Math]::Sin($angle*[Math]::PI/180)))
}
function Open-At([int]$x,[int]$y) {
    # Start each logical case in its circle view. Do not depend on the real cursor
    # dismissing the previous ring after a drop (notably after a Back-slot drop).
    $previousRing=[ItemDragWin]::Find([uint32]$p.Id,'DeskTuk.Ring',[int]::MinValue,0)
    if ($previousRing -ne [IntPtr]::Zero) {
        [void][ItemDragWin]::SendMessage($previousRing,0x200,[IntPtr]::Zero,[ItemDragWin]::Point(-1000,-1000))
    }
    $anchor=[ItemDragWin]::Find([uint32]$p.Id,'DeskTuk.Circle',$x,$y)
    $until=[DateTime]::UtcNow.AddSeconds(6)
    do {
        [void][ItemDragWin]::SendMessage($anchor,0x200,[IntPtr]::Zero,[ItemDragWin]::Point(20,20))
        $candidate=[ItemDragWin]::Find([uint32]$p.Id,'DeskTuk.Ring',[int]::MinValue,0)
        if ($candidate -ne [IntPtr]::Zero -and [ItemDragWin]::GetWindow($candidate,4) -eq $anchor) {
            return $candidate
        }
        Start-Sleep -Milliseconds 20
    } while ([DateTime]::UtcNow -lt $until)
    throw 'Requested source ring did not become ready'
}
function Begin-Item([IntPtr]$ring,[int]$slot) {
    $anchor=[ItemDragWin]::GetWindow($ring,4)
    $point=Slot-Point $ring $slot
    $ring=[ItemDragWin]::BeginItem([uint32]$p.Id,$anchor,$point[0],$point[1])
    $preview=[ItemDragWin]::Find([uint32]$p.Id,'DeskTuk.DragPreview',[int]::MinValue,0)
    if ($preview -eq [IntPtr]::Zero) { throw 'Item drag failed to start when foreground capture was denied' }
    $ring
}
function Release-At([IntPtr]$ring,[int]$x,[int]$y) {
    $r=[ItemDragWin]::Rectangle($ring)
    [ItemDragWin]::Mouse($ring,0x200,($x-$r.left),($y-$r.top))
    [void][ItemDragWin]::SendMessage($ring,0x2A3,[IntPtr]::Zero,[IntPtr]::Zero)
    Start-Sleep -Milliseconds 160
    if (-not [ItemDragWin]::IsWindow($ring)) { throw 'Source ring closed while dragging outside its disc' }
    [ItemDragWin]::Mouse($ring,0x202,($x-$r.left),($y-$r.top))
    if ([ItemDragWin]::Find([uint32]$p.Id,'DeskTuk.DragPreview',[int]::MinValue,0) -ne [IntPtr]::Zero) { throw 'Preview survived release' }
}
function Assert-Place([string]$place,[string]$label) {
    $saved=Get-Content -LiteralPath $cfg -Raw
    $line=if ($place) { "citem=$place|$file" } else { $null }
    $assignments=@(($saved -split '\r?\n') | Where-Object { $_.StartsWith('citem=') -and $_.EndsWith('|'+$file) })
    if (($place -and ($assignments.Count -ne 1 -or $assignments[0] -ne $line)) -or (-not $place -and $assignments.Count -ne 0)) { throw "$label failed: $assignments" }
    Write-Host "$label : PASS"
}
try {
    $cfg=Join-Path $root 'DeskTuk/desktuk.cfg'
    $log=Join-Path $root 'drag.log'
    $file=Join-Path $root 'reference.txt'
    Set-Content -LiteralPath $file -Value 'Read-only fixture reference'
    $hash=(Get-FileHash -LiteralPath $file).Hash
    $work=[ItemDragWin]::WorkArea()
    $d=[int][Math]::Ceiling(104*[ItemDragWin]::GetDpiForSystem()/96)
    $sx=$work.left+120; $sy=$work.top+40
    $nearX=$sx+$d+30
    $farX=$work.right-$d-80
    $recentX=$work.left+30; $recentY=$work.bottom-$d-30
    $config=@('host=window','ui=circles','grid=3',"recent=$recentX|$recentY",
        "circle=Source|$sx|$sy",'group=Source|Group',"circle=Near|$nearX|$sy",
        "circle=Far|$farX|$sy","citem=Source||$file")
    Set-Content -LiteralPath $cfg -Value ($config -join "`n") -Encoding UTF8
    $p=Start-Fixture $log
    $processes+=$p
    Wait-Check { [ItemDragWin]::Find([uint32]$p.Id,'DeskTuk.Circle',$sx,$sy) -ne [IntPtr]::Zero -and [ItemDragWin]::Find([uint32]$p.Id,'DeskTuk.Dock',[int]::MinValue,0) -ne [IntPtr]::Zero } 'All fixture windows ready'
    $ring=Open-At $sx $sy
    $ring=Begin-Item $ring 1
    Release-At $ring ($nearX+$d/2) ($sy+$d/2)
    Assert-Place 'Near|' 'Drop onto a circle underneath the source ring'
    $ring=Open-At $nearX $sy
    $ring=Begin-Item $ring 0
    Release-At $ring ($farX+$d/2) ($sy+$d/2)
    Assert-Place 'Far|' 'Drop outside the source ring into a distant circle'
    foreach ($cancel in @(0x1F,0x215,0x100)) {
        $ring=Open-At $farX $sy
        $ring=Begin-Item $ring 0
        [void][ItemDragWin]::SendMessage($ring,$cancel,[IntPtr]27,[IntPtr]::Zero)
        if ([ItemDragWin]::Find([uint32]$p.Id,'DeskTuk.DragPreview',[int]::MinValue,0) -ne [IntPtr]::Zero) { throw 'Preview survived cancellation' }
        Assert-Place 'Far|' 'Cancel/capture loss/Escape keeps the assignment'
    }
    $ring=Open-At $farX $sy
    $ring=Begin-Item $ring 0
    Release-At $ring ($work.right-10) ($work.bottom-10)
    Assert-Place 'Far|' 'Invalid desktop drop keeps the assignment'
    $ring=Open-At $farX $sy
    $ring=Begin-Item $ring 0
    Release-At $ring ($sx+$d/2) ($sy+$d/2)
    Assert-Place 'Source|' 'Return to source circle'
    $ring=Open-At $sx $sy
    $ring=Begin-Item $ring 1
    $slot=Slot-Point $ring 0
    $rect=[ItemDragWin]::Rectangle($ring)
    Release-At $ring ($rect.left+$slot[0]) ($rect.top+$slot[1])
    Assert-Place 'Source|Group' 'Drop onto group slot'
    $ring=Open-At $sx $sy
    $slot=Slot-Point $ring 0
    [ItemDragWin]::Mouse($ring,0x202,$slot[0],$slot[1])
    $ring=Begin-Item $ring 1
    $slot=Slot-Point $ring 0
    $rect=[ItemDragWin]::Rectangle($ring)
    Release-At $ring ($rect.left+$slot[0]) ($rect.top+$slot[1])
    Assert-Place 'Source|' 'Drop onto Back slot'
    $ring=Open-At $sx $sy
    $ring=Begin-Item $ring 1
    Release-At $ring ($recentX+$d/2) ($recentY+$d/2)
    Assert-Place '' 'Drop onto Recent unassigns the reference'
    if ($hash -ne (Get-FileHash -LiteralPath $file).Hash) { throw 'Referenced file changed' }
    # A second isolated board exercises stable in-place ordering in circle and group views.
    $other1=Join-Path $root 'z.exe'
    $other2=Join-Path $root 'b.txt'
    $other3=Join-Path $root 'a.exe'
    foreach ($item in @($other1,$other2,$other3)) { Set-Content -LiteralPath $item -Value 'Reference only' }
    $hashes=@{}; foreach ($item in @($file,$other1,$other2,$other3)) { $hashes[$item]=(Get-FileHash -LiteralPath $item).Hash }
    $controller=[ItemDragWin]::Find([uint32]$p.Id,'DeskTuk.Controller',[int]::MinValue,0)
    [void][ItemDragWin]::PostMessage($controller,0x10,[IntPtr]::Zero,[IntPtr]::Zero)
    if (-not $p.WaitForExit(3000)) { throw 'First fixture did not exit' }
    $config+=@("citem=Source||$other1","citem=Source||$other2","citem=Source||$other3")
    Set-Content -LiteralPath $cfg -Value ($config -join "`n") -Encoding UTF8
    $p=Start-Fixture $log; $processes+=$p
    Wait-Check { [ItemDragWin]::Find([uint32]$p.Id,'DeskTuk.Dock',[int]::MinValue,0) -ne [IntPtr]::Zero } 'Ordering fixture ready'
    function Assert-Order([string]$group,[string[]]$expected,[string]$label) {
        $prefix="citem=Source|$group|"
        Wait-Check { $actual=@(Get-Content -LiteralPath $cfg | Where-Object { $_.StartsWith($prefix) } | ForEach-Object { $_.Substring($prefix.Length) }); ($actual -join '|') -eq ($expected -join '|') } $label
    }
    $ring=Open-At $sx $sy
    $ring=Begin-Item $ring 4
    $slot=Slot-Point $ring 1; $rect=[ItemDragWin]::Rectangle($ring)
    Release-At $ring ($rect.left+$slot[0]) ($rect.top+$slot[1])
    Assert-Order '' @($other3,$file,$other1,$other2) 'Circle reorder inserts before the fixed target'
    $ring=Open-At $sx $sy
    $ring=Begin-Item $ring 1
    $slot=Slot-Point $ring 4; $rect=[ItemDragWin]::Rectangle($ring)
    $dx=$slot[0]-($rect.right-$rect.left)/2; $dy=$slot[1]-($rect.bottom-$rect.top)/2
    $length=[Math]::Sqrt($dx*$dx+$dy*$dy)
    Release-At $ring ([int]($rect.left+$slot[0]-16*$dy/$length)) ([int]($rect.top+$slot[1]+16*$dx/$length))
    Assert-Order '' @($file,$other1,$other2,$other3) 'Circle reorder appends after the last fixed target'
    $ring=Open-At $sx $sy; $ring=Begin-Item $ring 3
    [void][ItemDragWin]::SendMessage($ring,0x100,[IntPtr]27,[IntPtr]::Zero)
    Assert-Order '' @($file,$other1,$other2,$other3) 'Escape preserves custom order'
    foreach ($index in @(4,3)) {
        $ring=Open-At $sx $sy; $ring=Begin-Item $ring $index
        $slot=Slot-Point $ring 0; $rect=[ItemDragWin]::Rectangle($ring)
        Release-At $ring ($rect.left+$slot[0]) ($rect.top+$slot[1])
    }
    $ring=Open-At $sx $sy
    $slot=Slot-Point $ring 0; [ItemDragWin]::Mouse($ring,0x202,$slot[0],$slot[1])
    $ring=Begin-Item $ring 2
    $slot=Slot-Point $ring 1; $rect=[ItemDragWin]::Rectangle($ring)
    Release-At $ring ($rect.left+$slot[0]) ($rect.top+$slot[1])
    Assert-Order 'Group' @($other2,$other3) 'Group reorder uses the same fixed-slot drag'
    foreach ($item in $hashes.Keys) { if ($hashes[$item] -ne (Get-FileHash -LiteralPath $item).Hash) { throw 'Reference changed during reorder' } }
    Write-Host 'Reference contents preserved during transfers and reordering; no physical input : PASS' 
} catch {
    if (Test-Path -LiteralPath $log) { Get-Content -LiteralPath $log | Select-Object -Last 30 }
    throw
} finally {
    foreach ($instance in $processes) {
        if (-not $instance.HasExited) {
            $ctl=[ItemDragWin]::Find([uint32]$instance.Id,'DeskTuk.Controller',[int]::MinValue,0)
            [void][ItemDragWin]::PostMessage($ctl,0x10,[IntPtr]::Zero,[IntPtr]::Zero)
            if (-not $instance.WaitForExit(3000)) { Stop-Process -Id $instance.Id -Force }
        }
    }
    $resolved=(Resolve-Path -LiteralPath $root).Path
    if (-not $resolved.StartsWith($tempBase,[StringComparison]::OrdinalIgnoreCase) -or (Split-Path $resolved -Leaf) -notlike 'desktuk-item-drag-*') { throw 'Unsafe fixture cleanup' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
