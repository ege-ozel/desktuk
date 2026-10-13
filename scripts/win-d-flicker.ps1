param([string]$Exe, [int]$Trials = 3)
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
public static class Fl {
  [DllImport("user32.dll")] static extern IntPtr GetDC(IntPtr h);
  [DllImport("user32.dll")] static extern int ReleaseDC(IntPtr h, IntPtr dc);
  [DllImport("gdi32.dll")] static extern uint GetPixel(IntPtr dc, int x, int y);
  [DllImport("user32.dll")] static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  static void Combo() {
    keybd_event(0x5B, 0, 0, UIntPtr.Zero); keybd_event(0x44, 0, 0, UIntPtr.Zero);
    keybd_event(0x44, 0, 2, UIntPtr.Zero); keybd_event(0x5B, 0, 2, UIntPtr.Zero);
  }
  [StructLayout(LayoutKind.Sequential)] public struct PT { public int x, y; }
  [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(PT p);
  [DllImport("user32.dll")] static extern IntPtr GetAncestor(IntPtr h, uint f);
  [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] static extern bool BringWindowToTop(IntPtr h);
  public static long AppAt(int x, int y) { return (long)GetAncestor(WindowFromPoint(new PT { x = x, y = y }), 2); }
  [DllImport("user32.dll")] static extern IntPtr FindWindow(string c, string t);
  public static void Activate(long h) {
    keybd_event(0x12, 0, 0, UIntPtr.Zero); keybd_event(0x12, 0, 2, UIntPtr.Zero);
    SetForegroundWindow(FindWindow("Shell_TrayWnd", null));   // switch away first ...
    System.Threading.Thread.Sleep(500);                       // ... then back, so the app is raised normally
    keybd_event(0x12, 0, 0, UIntPtr.Zero); keybd_event(0x12, 0, 2, UIntPtr.Zero); // Alt unlocks SetForegroundWindow
    SetForegroundWindow((IntPtr)h);
  }
  // Returns transitions "t_ms:state" (state 1 = node accent colour visible), plus key press times.
  public static List<string> Run(int x, int y, uint color, int[] pressAt, int totalMs) {
    var res = new List<string>();
    var sw = Stopwatch.StartNew();
    IntPtr dc = GetDC(IntPtr.Zero);
    int last = -1, next = 0; long samples = 0;
    while (sw.Elapsed.TotalMilliseconds < totalMs) {
      double t = sw.Elapsed.TotalMilliseconds;
      if (next < pressAt.Length && t >= pressAt[next]) { res.Add(string.Format("{0:F1}:KEY{1}", t, next + 1)); Combo(); next++; }
      int s = GetPixel(dc, x, y) == color ? 1 : 0; samples++;
      if (s != last) { res.Add(string.Format("{0:F1}:{1}", sw.Elapsed.TotalMilliseconds, s == 1 ? "VISIBLE" : "hidden")); last = s; }
    }
    ReleaseDC(IntPtr.Zero, dc);
    res.Add("samples=" + samples + " avg_us=" + (totalMs * 1000.0 / samples).ToString("F0"));
    return res;
  }
}
'@
$px = 1800; $py = 300; $color = [uint32]0x00D27D2D   # ACCENT rgb(45,125,210) as COLORREF; sample y+1
foreach ($hostName in 'window', 'owned') {
  "=== host=$hostName"
  for ($i = 1; $i -le $Trials; $i++) {
    $app = [Fl]::AppAt($px + 80, $py + 1)
    $p = Start-Process $Exe -ArgumentList '--host', $hostName, '--no-save', '--allow-multiple', '--pos', "$px,$py" -PassThru
    Start-Sleep -Milliseconds 2000
    # Activate the application under the node, so it sits above it like a normal window would.
    [Fl]::Activate($app)
    Start-Sleep -Milliseconds 800
    $r = [Fl]::Run($px + 80, $py + 1, $color, @(500, 2500), 4500)
    "trial ${i}: " + ($r -join '  ')
    Stop-Process -Id $p.Id -Force
    Start-Sleep -Milliseconds 800
  }
}
