<# Compares circle/grid input processing with isolated TEMP state. Each native move
   is verified. No physical input, Explorer restart or Desktop writes. Submission
   timing is not a displayed-frame-rate measurement. Keep the physical pointer still
   during measurement; mixed input or any incorrect move aborts the result. #>
param([string]$Exe = 'target/release/DeskTuk.exe',[ValidateRange(1,500)][int]$Count=120)
$ErrorActionPreference='Stop'
$build=(Resolve-Path -LiteralPath $Exe).Path
$tempBase=[IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$root=Join-Path $tempBase ('desktuk-drag-bench-'+[guid]::NewGuid().ToString('N'))
$null=New-Item -ItemType Directory -Path (Join-Path $root 'DeskTuk')
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class DragBenchWin {
    public static double Burst(IntPtr hwnd, Rect work, int diameter, int count) {
        var timer=System.Diagnostics.Stopwatch.StartNew();
        for(int i=0;i<count;i++) {
            int col=i%3, row=(i/3)%3;
            int x=work.left+(int)((col+0.5)*(work.right-work.left)/3);
            int y=work.top+(int)((row+0.5)*(work.bottom-work.top)/3);
            Rect r=Rectangle(hwnd);
            SendMessage(hwnd,0x200,new IntPtr(0x10),Point(x-r.left,y-r.top));
            Rect actual=Rectangle(hwnd);
            if (actual.left!=x-diameter/2 || actual.top!=y-diameter/2) {
                throw new Exception("Mixed input or incorrect movement at iteration "+i+": "+actual.left+","+actual.top+" expected "+(x-diameter/2)+","+(y-diameter/2));
            }
        }
        timer.Stop(); return timer.Elapsed.TotalMilliseconds;
    }
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int left, top, right, bottom; }
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder b, int n);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out Rect r);
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

try {
    $w=[DragBenchWin]::WorkArea()
    $x=$w.left+100; $y=$w.top+100
    $cfg=Join-Path $root 'DeskTuk/desktuk.cfg'
    $log=Join-Path $root 'bench.log'
    Set-Content -LiteralPath $cfg -Value "host=window`nui=circles`ngrid=3`nrecent=$x|$y`n" -Encoding UTF8
    $previous=$env:APPDATA
    try {
        $env:APPDATA=$root
        $p=Start-Process -FilePath $build -WindowStyle Hidden -PassThru -ArgumentList @('--allow-multiple','--exit-after','60000','--log',('"'+$log+'"'))
    } finally { $env:APPDATA=$previous }
    $until=[DateTime]::UtcNow.AddSeconds(8)
    do {
        $anchor=[DragBenchWin]::Find([uint32]$p.Id,'DeskTuk.Circle',$x,$y)
        if ($anchor -ne [IntPtr]::Zero -and [DragBenchWin]::Find([uint32]$p.Id,'DeskTuk.Dock',[int]::MinValue,0) -ne [IntPtr]::Zero) { break }
        Start-Sleep -Milliseconds 60
    } while([DateTime]::UtcNow -lt $until)
    if ($anchor -eq [IntPtr]::Zero) { throw 'Missing benchmark circle' }
    $d=[int][Math]::Ceiling(104*[DragBenchWin]::GetDpiForSystem()/96)
    [DragBenchWin]::MiddleMouse($anchor,0x207,$d/2,$d/2)
    $p.Refresh(); $cpu=$p.TotalProcessorTime.TotalMilliseconds
    $ms=[DragBenchWin]::Burst($anchor,$w,$d,$Count)
    $p.Refresh(); $cpu=$p.TotalProcessorTime.TotalMilliseconds-$cpu
    [DragBenchWin]::MiddleMouse($anchor,0x208,$d/2,$d/2)
    [pscustomobject]@{ Exe=$build; Moves=$Count; ElapsedMs=[Math]::Round($ms,2); AppCpuMs=[Math]::Round($cpu,2) }
    $ctl=[DragBenchWin]::Find([uint32]$p.Id,'DeskTuk.Controller',[int]::MinValue,0)
    [void][DragBenchWin]::PostMessage($ctl,0x10,[IntPtr]::Zero,[IntPtr]::Zero)
    if (-not $p.WaitForExit(3000)) { throw 'Benchmark shutdown failed' }
    Get-Content -LiteralPath $log | Select-String 'METRIC grid_drag' | ForEach-Object { Write-Host $_.Line } 
} finally {
    if ($p -and -not $p.HasExited) {
        $ctl=[DragBenchWin]::Find([uint32]$p.Id,'DeskTuk.Controller',[int]::MinValue,0)
        [void][DragBenchWin]::PostMessage($ctl,0x10,[IntPtr]::Zero,[IntPtr]::Zero)
        if (-not $p.WaitForExit(3000)) { Stop-Process -Id $p.Id -Force }
    }
    $resolved=(Resolve-Path -LiteralPath $root).Path
    if (-not $resolved.StartsWith($tempBase,[StringComparison]::OrdinalIgnoreCase) -or (Split-Path $resolved -Leaf) -notlike 'desktuk-drag-bench-*') { throw 'Unsafe benchmark cleanup' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}

