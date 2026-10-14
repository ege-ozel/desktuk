param([string]$Exe)
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
    [void][OpenCircleDragWin]::SendMessage($anchor,0x2A1,[IntPtr]::Zero,[IntPtr]::Zero)
    Wait-Check { [OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Ring',[int]::MinValue,0) -ne [IntPtr]::Zero } 'Ring opens'
    Start-Sleep -Milliseconds 400
    [OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Ring',[int]::MinValue,0)
}

try {
    $cfg=Join-Path $root 'DeskTuk/desktuk.cfg'
    $lines=@('host=window','ui=circles','recent=300|300','circle=Music|900|300')
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
    Wait-Check { [OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Circle',900,300) -ne [IntPtr]::Zero } 'Fixture circles created'
    foreach ($sourceX in @(300,900)) {
        $anchor=[OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Circle',$sourceX,300)
        $ring=Open-Ring $p $anchor | Select-Object -Last 1
        $rect=[OpenCircleDragWin]::Rectangle($ring)
        $cx=($rect.right-$rect.left)/2
        $cy=($rect.bottom-$rect.top)/2
        [OpenCircleDragWin]::Mouse($ring,0x201,$cx,$cy)
        [void][OpenCircleDragWin]::SendMessage($ring,0x2A3,[IntPtr]::Zero,[IntPtr]::Zero)
        Start-Sleep -Milliseconds 200
        if (-not [OpenCircleDragWin]::IsWindow($ring)) { throw 'Ring closed during a pending centre drag' }
        $atX=$rect.left+$cx+90-$sourceX
        $atY=$rect.top+$cy+35-300
        [OpenCircleDragWin]::Mouse($anchor,0x200,$atX,$atY)
        [OpenCircleDragWin]::Mouse($anchor,0x202,$atX-90,$atY-35)
        $expectedX=$sourceX+90
        Wait-Check { [OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Circle',$expectedX,335) -ne [IntPtr]::Zero } "Open circle at $sourceX moves by event delta"
        if ([OpenCircleDragWin]::IsWindow($ring)) { throw 'Ring remained open during the circle move' }
        $saved=Get-Content -LiteralPath $cfg -Raw
        $expectedLine=if ($sourceX -eq 300) { 'recent=390|335' } else { 'circle=Music|990|335' }
        if (-not $saved.Contains($expectedLine)) { throw 'Moved circle position was not saved' }
        Write-Host "$expectedLine persisted : PASS"
    }
    # A short centre click still turns the existing Music ring page, without reopening it.
    $anchor=[OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Circle',990,335)
    $ring=Open-Ring $p $anchor | Select-Object -Last 1
    $rect=[OpenCircleDragWin]::Rectangle($ring)
    $cx=($rect.right-$rect.left)/2
    $cy=($rect.bottom-$rect.top)/2
    [OpenCircleDragWin]::Mouse($ring,0x201,$cx,$cy)
    [OpenCircleDragWin]::Mouse($anchor,0x202,44,44)
    Wait-Check { (Get-Content -LiteralPath $log -Raw) -match 'item loaded path=.*fixture-ref-23.txt' } 'Centre click loads the next page'
    if (-not [OpenCircleDragWin]::IsWindow($ring)) { throw 'Centre click destroyed the existing ring' }
    $controller=[OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Controller',[int]::MinValue,0)
    [void][OpenCircleDragWin]::PostMessage($controller,0x10,[IntPtr]::Zero,[IntPtr]::Zero)
    if (-not $p.WaitForExit(8000) -or $p.ExitCode -ne 0) { throw 'Fixture did not exit cleanly' }
    $p=Start-Fixture (Join-Path $root 'restart.log')
    $processes+=$p
    Wait-Check { [OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Circle',390,335) -ne [IntPtr]::Zero -and [OpenCircleDragWin]::Find([uint32]$p.Id,'DeskTuk.Circle',990,335) -ne [IntPtr]::Zero } 'Moved positions survive restart'
    $hashesAfter=@($files | ForEach-Object { (Get-FileHash -LiteralPath $_).Hash })
    if (@(Compare-Object $hashes $hashesAfter).Count -ne 0) { throw 'Referenced fixture files changed' }
    Write-Host 'All file references preserved : PASS'
    Write-Host 'Mouse movement/clicks were routed messages; the physical cursor was not moved.'
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
