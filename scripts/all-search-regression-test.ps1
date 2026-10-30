param([Parameter(Mandatory=$true)][string]$Exe, [string]$PreviewDir)
$ErrorActionPreference = 'Stop'
$build = (Resolve-Path -LiteralPath $Exe).Path
$tempBase = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$root = Join-Path $tempBase ('desktuk-all-search-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path (Join-Path $root 'DeskTuk')
$process = $null
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class AllSearchWin {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int left, top, right, bottom; }
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc proc, IntPtr l);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h,StringBuilder b,int n);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out Rect rect);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h,int id);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h,IntPtr dc,uint flags);
    public static IntPtr Find(uint pid,string cls) {
        IntPtr result=IntPtr.Zero;
        EnumWindows((h,l) => {
            uint p; GetWindowThreadProcessId(h,out p);
            var name=new StringBuilder(128); GetClassName(h,name,128);
            if (p==pid && name.ToString()==cls) { result=h; return false; }
            return true;
        },IntPtr.Zero);
        return result;
    }
    public static Rect Rectangle(IntPtr h) {
        Rect rect; if (!GetWindowRect(h,out rect)) throw new Exception("Missing fixture window"); return rect;
    }
    public static void Click(IntPtr h,int x,int y) {
        SendMessage(h,0x202,IntPtr.Zero,new IntPtr(unchecked((int)((uint)(ushort)x | ((uint)(ushort)y << 16)))));
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
function Scale([int]$value) { [int][Math]::Floor(($value*$dpi+48)/96) }
function Save-Preview([IntPtr]$window,[string]$name) {
    if (-not $PreviewDir) { return }
    Add-Type -AssemblyName System.Drawing
    $null=New-Item -ItemType Directory -Path $PreviewDir -Force
    [void][AllSearchWin]::SendMessage($window,0xF,[IntPtr]::Zero,[IntPtr]::Zero)
    $r=[AllSearchWin]::Rectangle($window)
    $bitmap=New-Object Drawing.Bitmap ($r.right-$r.left),($r.bottom-$r.top)
    $graphics=[Drawing.Graphics]::FromImage($bitmap)
    $dc=$graphics.GetHdc()
    try { [void][AllSearchWin]::PrintWindow($window,$dc,2) }
    finally { $graphics.ReleaseHdc($dc); $graphics.Dispose() }
    try { $bitmap.Save((Join-Path $PreviewDir $name),[Drawing.Imaging.ImageFormat]::Png) }
    finally { $bitmap.Dispose() }
}
function Open-Search([int]$row) {
    [AllSearchWin]::Click($dock,(Scale 80),(Scale (155+28*$row)))
    Wait-Check { [AllSearchWin]::Find([uint32]$process.Id,'DeskTuk.Search') -ne [IntPtr]::Zero } "Search row $row opens"
    $window=[AllSearchWin]::Find([uint32]$process.Id,'DeskTuk.Search')
    @{Window=$window; Edit=[AllSearchWin]::GetDlgItem($window,1); List=[AllSearchWin]::GetDlgItem($window,2)}
}
function Check-Query($search,[string]$query,[int]$expected) {
    [void][AllSearchWin]::SendMessage($search.Edit,0xb1,[IntPtr]::Zero,[IntPtr](-1))
    [void][AllSearchWin]::SendMessage($search.Edit,0x102,[IntPtr]8,[IntPtr]1)
    foreach ($character in $query.ToCharArray()) { [void][AllSearchWin]::SendMessage($search.Edit,0x102,[IntPtr][int]$character,[IntPtr]1) }
    Wait-Check { [AllSearchWin]::SendMessage($search.List,0x18b,[IntPtr]::Zero,[IntPtr]::Zero).ToInt64() -eq $expected } "Query '$query' returns $expected"
}
function Close-Search($search) { [void][AllSearchWin]::SendMessage($search.Window,0x10,[IntPtr]::Zero,[IntPtr]::Zero) }
try {
    $token='all-'+[guid]::NewGuid().ToString('N')+'-'
    $files=@('music','music-group','apps','apps-group') | ForEach-Object {
        $path=Join-Path $root ($token+$_+'.txt')
        Set-Content -LiteralPath $path -Value "fixture $_" -Encoding utf8
        $path
    }
    $hashes=@($files | ForEach-Object { (Get-FileHash -LiteralPath $_).Hash })
    $cfg=Join-Path $root 'DeskTuk/desktuk.cfg'
    $lines=@('host=window','ui=circles','grid=0','language=en','recent=300|200',
        'circle=Music|500|200','group=Music|Live',"citem=Music||$($files[0])","citem=Music|Live|$($files[1])",
        'circle=Apps|700|200','group=Apps|Tools',"citem=Apps||$($files[2])","citem=Apps|Tools|$($files[3])")
    Set-Content -LiteralPath $cfg -Value ($lines -join "`n") -Encoding utf8
    $previousAppData=$env:APPDATA
    try {
        $env:APPDATA=$root
        $process=Start-Process -FilePath $build -WindowStyle Hidden -PassThru -ArgumentList @('--allow-multiple','--exit-after','45000')
    } finally { $env:APPDATA=$previousAppData }
    Wait-Check { [AllSearchWin]::Find([uint32]$process.Id,'DeskTuk.Dock') -ne [IntPtr]::Zero } 'Fixture dock opens'
    $dock=[AllSearchWin]::Find([uint32]$process.Id,'DeskTuk.Dock')
    $dpi=[AllSearchWin]::GetDpiForWindow($dock)
    Save-Preview $dock 'expanded.png'
    $all=Open-Search 0
    $allCount=[AllSearchWin]::SendMessage($all.List,0x18b,[IntPtr]::Zero,[IntPtr]::Zero).ToInt64()
    if ($allCount -lt 4) { throw 'All omitted fixture references' }
    Check-Query $all $token 4
    Save-Preview $all.Window 'all-results.png'
    foreach ($name in @('music.txt','music-group.txt','apps.txt','apps-group.txt')) { Check-Query $all ($token+$name) 1 }
    Check-Query $all ($token+'missing') 0
    Check-Query $all '' $allCount
    Close-Search $all
    $recent=Open-Search 1
    Check-Query $recent $token 0
    Check-Query $recent '' ($allCount-4)
    Close-Search $recent
    $circle=Open-Search 2
    Check-Query $circle $token 2
    Check-Query $circle '' 2
    [void][AllSearchWin]::SendMessage($circle.List,0x186,[IntPtr]::Zero,[IntPtr]::Zero)
    [void][AllSearchWin]::SendMessage($circle.Window,0x804c,[IntPtr]::Zero,[IntPtr]::Zero)
    Check-Query $circle ($token+'music.txt') 0
    Check-Query $circle ($token+'music-group.txt') 1
    Check-Query $circle '' 2
    [void][AllSearchWin]::SendMessage($circle.List,0x186,[IntPtr]::Zero,[IntPtr]::Zero)
    [void][AllSearchWin]::SendMessage($circle.Window,0x804c,[IntPtr]::Zero,[IntPtr]::Zero)
    Check-Query $circle $token 2
    Close-Search $circle
    [AllSearchWin]::Click($dock,(Scale 18),(Scale 20))
    if ((Get-Content -LiteralPath $cfg -Raw) -notmatch '(?m)^dock_locked=1') { throw 'Fixture lock did not engage' }
    $locked=Open-Search 0
    Check-Query $locked $token 4
    Close-Search $locked
    [AllSearchWin]::Click($dock,(Scale 18),(Scale 20))
    $expanded=[AllSearchWin]::Rectangle($dock)
    [AllSearchWin]::Click($dock,($expanded.right-$expanded.left-(Scale 18)),(Scale 20))
    $compact=[AllSearchWin]::Rectangle($dock)
    if ($compact.right-$compact.left -ne (Scale 64) -or $compact.bottom-$compact.top -ne (Scale 132)) { throw 'Compact dimensions changed' }
    Save-Preview $dock 'collapsed.png'
    Write-Host "Compact width=$($compact.right-$compact.left) height=$($compact.bottom-$compact.top) DPI=$dpi : PASS"
    $after=@($files | ForEach-Object { (Get-FileHash -LiteralPath $_).Hash })
    if (@(Compare-Object $hashes $after).Count -ne 0) { throw 'Referenced fixture files changed' }
    Write-Host 'All/Recent/circle/group search, empty reset, locked access and reference preservation : PASS'
    Write-Host 'Only routed messages were used; no physical mouse or keyboard input.'
} finally {
    if ($process -and -not $process.HasExited) {
        $controller=[AllSearchWin]::Find([uint32]$process.Id,'DeskTuk.Controller')
        [void][AllSearchWin]::SendMessage($controller,0x10,[IntPtr]::Zero,[IntPtr]::Zero)
        if (-not $process.WaitForExit(3000)) { Stop-Process -Id $process.Id -Force }
    }
    $resolved=(Resolve-Path -LiteralPath $root).Path
    if (-not $resolved.StartsWith($tempBase,[StringComparison]::OrdinalIgnoreCase) -or (Split-Path $resolved -Leaf) -notlike 'desktuk-all-search-*') { throw 'Unsafe fixture cleanup path' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
