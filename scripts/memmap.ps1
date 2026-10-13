param([string]$Exe, [string]$Host_ = 'window', [int]$WaitMs = 3500)
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class VM {
  [StructLayout(LayoutKind.Sequential)] public struct MBI { public IntPtr Base, AllocBase; public uint AllocProt; public UIntPtr Size; public uint State, Protect, Type; }
  [DllImport("kernel32.dll")] static extern UIntPtr VirtualQueryEx(IntPtr h, IntPtr a, out MBI m, UIntPtr len);
  [DllImport("kernel32.dll")] static extern IntPtr OpenProcess(uint acc, bool inh, int pid);
  [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
  public class Reg { public long Base, Size; public uint Prot, Type, AllocProt; }
  public static List<Reg> Walk(int pid) {
    var l = new List<Reg>();
    IntPtr h = OpenProcess(0x0410, false, pid);
    long a = 0; MBI m;
    while (VirtualQueryEx(h, (IntPtr)a, out m, (UIntPtr)Marshal.SizeOf(typeof(MBI))) != UIntPtr.Zero) {
      long sz = (long)m.Size;
      if (m.State == 0x1000) l.Add(new Reg { Base = (long)m.Base, Size = sz, Prot = m.Protect, Type = m.Type, AllocProt = m.AllocProt });
      a = (long)m.Base + sz;
      if (a >= 0x7FFFFFFF0000L) break;
    }
    CloseHandle(h);
    return l;
  }
}
'@
$args2 = @('--host', $Host_, '--no-save', '--allow-multiple')
$p = Start-Process -FilePath $Exe -ArgumentList $args2 -PassThru
Start-Sleep -Milliseconds $WaitMs
$p.Refresh()
$regs = [VM]::Walk($p.Id)
$mods = @(); foreach ($m in $p.Modules) { $mods += [pscustomobject]@{ Name=$m.ModuleName; B=[int64]$m.BaseAddress; E=[int64]$m.BaseAddress + $m.ModuleMemorySize } }
function Writable($prot) { ($prot -band 0x04) -or ($prot -band 0x08) -or ($prot -band 0x40) -or ($prot -band 0x80) }
$kb = { param($b) [math]::Round($b/1024) }
"PID $($p.Id)  PrivateMemorySize64(commit)=$(& $kb $p.PrivateMemorySize64) KB  WorkingSet=$(& $kb $p.WorkingSet64) KB"
"--- committed by type"
$regs | Group-Object Type | ForEach-Object { $t = switch ($_.Name) { 16777216 {'IMAGE'} 262144 {'MAPPED'} 131072 {'PRIVATE'} default {$_.Name} }; "{0,-8} {1,8} KB  regions={2}" -f $t, (& $kb ($_.Group | Measure-Object Size -Sum).Sum), $_.Count }
"--- IMAGE committed, writable (private copy-on-write) vs read-only/exec, per module"
$img = $regs | Where-Object { $_.Type -eq 16777216 }
$rows = foreach ($r in $img) {
  $mod = $mods | Where-Object { $r.Base -ge $_.B -and $r.Base -lt $_.E } | Select-Object -First 1
  [pscustomobject]@{ Mod = if ($mod) { $mod.Name } else { '?' }; W = (Writable $r.Prot); Size = $r.Size }
}
$rows | Group-Object Mod | ForEach-Object {
  $w = ($_.Group | Where-Object W | Measure-Object Size -Sum).Sum
  $ro = ($_.Group | Where-Object { -not $_.W } | Measure-Object Size -Sum).Sum
  [pscustomobject]@{ Module = $_.Name; WritableKB = (& $kb $w); ReadOnlyKB = (& $kb $ro) }
} | Sort-Object WritableKB -Descending | Select-Object -First 12 | Format-Table -AutoSize | Out-String
"--- PRIVATE regions (largest 15)"
$regs | Where-Object { $_.Type -eq 131072 } | Sort-Object Size -Descending | Select-Object -First 15 | ForEach-Object { "{0:X} {1,8} KB prot=0x{2:X}" -f $_.Base, (& $kb $_.Size), $_.Prot }
$priv = $regs | Where-Object { $_.Type -eq 131072 }
"PRIVATE total regions=$($priv.Count) sum=$(& $kb ($priv | Measure-Object Size -Sum).Sum) KB; small (<=64KB) regions sum=$(& $kb ($priv | Where-Object Size -le 65536 | Measure-Object Size -Sum).Sum) KB"
"--- loaded modules ($($mods.Count))"
($mods | ForEach-Object Name) -join ', '
Stop-Process -Id $p.Id -Force
