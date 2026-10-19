param([Parameter(Mandatory=$true)][string]$Compiler)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
$root = Join-Path ([IO.Path]::GetTempPath()) ('desktuk-installer-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path $root
$output = Join-Path $workspace 'target\installer-test'
& (Join-Path $PSScriptRoot 'build-installer.ps1') -Compiler $Compiler -SkipBuild -Fixture -OutputDir $output
if ($LASTEXITCODE -ne 0) { throw 'Fixture build failed' }
$setup = Join-Path $output 'DeskTuk-InstallerFixture.exe'
$destination = Join-Path $root 'Programs\Müzik Araçları\DeskTuk'
$registry = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\DeskTuk.InstallerFixture_is1'
$runKey = 'HKCU:\Software\DeskTuk.InstallerFixture\Run'
if (Test-Path -LiteralPath $registry) { throw 'A previous installer fixture is registered; inspect it first' }

Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class InstallerWin {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr w, EnumProc p, IntPtr l);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr w);
  [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr w);
  [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr h,StringBuilder s,int n);
  [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern IntPtr SendMessageW(IntPtr h,uint m,IntPtr w,StringBuilder s);
  [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
  [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,string l);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h,IntPtr dc,uint flags);
  [StructLayout(LayoutKind.Sequential)] public struct Rect { public int left,top,right,bottom; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out Rect r);
  public static string Text(IntPtr h) { var b=new StringBuilder(2048); SendMessageW(h,0xD,(IntPtr)b.Capacity,b); return b.ToString(); }
  public static string Class(IntPtr h) { var b=new StringBuilder(128); GetClassName(h,b,b.Capacity); return b.ToString(); }
  public static IntPtr Wizard() {
    IntPtr found=IntPtr.Zero;
    EnumWindows((h,l)=> {if (Class(h)=="TWizardForm" && IsWindowVisible(h) && Text(h).Contains("DeskTuk")) {found=h; return false;} return true;},IntPtr.Zero);
    return found;
  }
  public static IntPtr[] Children(IntPtr h) {
    var a=new List<IntPtr>(); EnumChildWindows(h,(w,l)=>{if(IsWindowVisible(w)) a.Add(w);return true;},IntPtr.Zero); return a.ToArray();
  }
}
'@
function Wait-Until([scriptblock]$Check,[string]$Label) {
    $until=[DateTime]::UtcNow.AddSeconds(20)
    do { if (& $Check) { return }; Start-Sleep -Milliseconds 80 } while ([DateTime]::UtcNow -lt $until)
    throw "Timed out: $Label"
}
function Screenshot([IntPtr]$window,[string]$name) {
    $r=New-Object InstallerWin+Rect
    [void][InstallerWin]::GetWindowRect($window,[ref]$r)
    $bmp=New-Object Drawing.Bitmap ($r.right-$r.left),($r.bottom-$r.top)
    $g=[Drawing.Graphics]::FromImage($bmp); $dc=$g.GetHdc()
    try { [void][InstallerWin]::PrintWindow($window,$dc,2) }
    finally { $g.ReleaseHdc($dc); $g.Dispose() }
    try { $bmp.Save((Join-Path $output $name),[Drawing.Imaging.ImageFormat]::Png) }
    finally { $bmp.Dispose() }
}
function Run-Wizard([bool]$Upgrade) {
    $label=if($Upgrade){'upgrade'}else{'install'}
    $language=if($Upgrade){'english'}else{'turkish'}
    $finish=if($Upgrade){'Finish'}else{'Bitir'}
    $log=Join-Path $output "$label.log"
    $p=Start-Process -FilePath $setup -ArgumentList @('/SP-',('/LANG='+$language),'/NORESTART',('/FIXTUREROOT="'+$root+'"'),('/LOG="'+$log+'"')) -WindowStyle Hidden -PassThru
    Wait-Until { [InstallerWin]::Wizard() -ne [IntPtr]::Zero } 'wizard creation'
    $seenPath=$false; $finished=$false
    for($step=0; $step -lt 30; $step++) {
        $w=[InstallerWin]::Wizard()
        $children=@([InstallerWin]::Children($w))
        $edit=$children | Where-Object { [InstallerWin]::Class($_) -in @('TNewEdit','TNewPathEdit') -and [InstallerWin]::Text($_) -match '\\DeskTuk$' } | Select-Object -First 1
        if($edit -and -not $seenPath) {
            if($Upgrade -and [InstallerWin]::Text($edit) -ne $destination) { throw 'Upgrade did not preserve selected path' }
            [void][InstallerWin]::SendMessage($edit,0xB1,[IntPtr]::Zero,[IntPtr](-1))
            [void][InstallerWin]::SendMessage($edit,0xC2,[IntPtr]1,$destination)
            if([InstallerWin]::Text($edit) -ne $destination) { throw 'Unicode directory edit failed' }
            $seenPath=$true
            Screenshot $w "$label-path.png"
        }
        $button=$children | Where-Object { [InstallerWin]::Class($_) -eq 'TNewButton' -and [InstallerWin]::IsWindowEnabled($_) -and ([InstallerWin]::Text($_).Replace('&','') -match '^(Sonraki|İleri|Next|Kur|Install|Bitir|Finish)') } | Select-Object -First 1
        if($button) {
            $caption=[InstallerWin]::Text($button).Replace('&','')
            if($caption -eq $finish) {
                if(-not $seenPath) { throw 'Destination page was skipped' }
                $finished=$true
                Screenshot $w "$label-finished.png"
            }
            [void][InstallerWin]::SendMessage($button,0xF5,[IntPtr]::Zero,[IntPtr]::Zero)
            if($finished) { break }
        }
        Start-Sleep -Milliseconds 180
    }
    if(-not $finished) { throw 'Wizard never displayed Bitir/Finish' }
    if(-not $p.WaitForExit(10000) -or $p.ExitCode -ne 0) { throw 'Installer did not exit successfully after Finish' }
    Wait-Until { Test-Path -LiteralPath (Join-Path $destination 'DeskTuk.exe') } 'installed EXE'
    Write-Host "$label Unicode path page and final $finish button : PASS"
}
$ws=New-Object -ComObject WScript.Shell
$shell=New-Object -ComObject Shell.Application
foreach($dir in @('Pins','Desktop','Start Menu','State')) { $null=New-Item -ItemType Directory -Path (Join-Path $root $dir) }
$oldPin=Join-Path $root 'Pins\DeskTuk desktop Nodes.lnk'
$sc=$ws.CreateShortcut($oldPin); $sc.TargetPath=(Join-Path $root 'Old\DeskTuk.exe'); $sc.Save()
$oldHash=(Get-FileHash -LiteralPath $oldPin).Hash
$customPin=Join-Path $root 'Pins\DeskTuk custom.lnk'
$sc=$ws.CreateShortcut($customPin); $sc.TargetPath=(Join-Path $root 'Old\DeskTuk.exe'); $sc.Arguments='--diag'; $sc.Save()
$customHash=(Get-FileHash -LiteralPath $customPin).Hash
$null=New-Item -Path $runKey -Force
New-ItemProperty -Path $runKey -Name DeskTuk -Value '"C:\Old\DeskTuk.exe"' -PropertyType String -Force | Out-Null
$cfg=Join-Path $root 'State\desktuk.cfg'; 'fixture-config-preserve' | Set-Content -LiteralPath $cfg
$cfgHash=(Get-FileHash -LiteralPath $cfg).Hash
Run-Wizard $false
foreach($link in @($oldPin,(Join-Path $root 'Desktop\DeskTuk.lnk'),(Join-Path $root 'Start Menu\DeskTuk.lnk'))) {
    $sc=$ws.CreateShortcut($link)
    # WScript.Shell reads target paths through a legacy ANSI conversion. Inspect
    # the Shell link's Unicode fields for dotless-i and other non-ANSI paths.
    $item=$shell.NameSpace((Split-Path $link)).ParseName((Split-Path $link -Leaf))
    if($item.GetLink.Path -ne (Join-Path $destination 'DeskTuk.exe') -or $item.GetLink.WorkingDirectory -ne $destination) { throw "Wrong shortcut path: $link" }
    if($sc.IconLocation -notmatch 'desktuk-[a-f0-9]+\.ico,0$') { throw "Wrong icon: $link" }
    $id=$shell.NameSpace((Split-Path $link)).ParseName((Split-Path $link -Leaf)).ExtendedProperty('System.AppUserModel.ID')
    if($id -ne 'DeskTuk.Desktop') { throw "Wrong AppUserModelID: $link" }
}
if((Get-FileHash -LiteralPath (Join-Path $root 'State\ShortcutBackups\DeskTuk desktop Nodes.lnk')).Hash -ne $oldHash) { throw 'Original pin was not backed up' }
if((Get-FileHash -LiteralPath $customPin).Hash -ne $customHash) { throw 'Custom argument link was modified' }
if((Get-ItemProperty -LiteralPath $runKey).DeskTuk -ne ('"'+(Join-Path $destination 'DeskTuk.exe')+'"')) { throw 'Startup was not migrated' }
if((Get-FileHash -LiteralPath (Join-Path $destination 'DeskTuk.exe')).Hash -ne (Get-FileHash -LiteralPath (Join-Path $workspace 'target\release\DeskTuk.exe')).Hash) { throw 'Packaged executable differs' }
$ico=Get-ChildItem -LiteralPath $destination -Filter 'desktuk-*.ico' | Select-Object -First 1
if((Get-FileHash -LiteralPath $ico.FullName).Hash -ne (Get-FileHash -LiteralPath (Join-Path $workspace 'res\desktuk.ico')).Hash) { throw 'Packaged icon differs from brand resource' }
Write-Host 'Desktop/menu/pinned path, icon, AppID, backup and startup migration : PASS'
$sentinel=Join-Path $destination 'keep-user-file.txt'; 'preserve' | Set-Content -LiteralPath $sentinel
Run-Wizard $true
$uninstaller=Join-Path $destination 'unins000.exe'
$p=Start-Process -FilePath $uninstaller -ArgumentList @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART',('/FIXTUREROOT="'+$root+'"')) -WindowStyle Hidden -PassThru -Wait
if($p.ExitCode -ne 0) { throw 'Uninstall failed' }
if(Test-Path -LiteralPath (Join-Path $destination 'DeskTuk.exe')) { throw 'Uninstall retained installed EXE' }
if(-not (Test-Path -LiteralPath $sentinel) -or (Get-FileHash -LiteralPath $cfg).Hash -ne $cfgHash) { throw 'Uninstall removed user data' }
if(Test-Path -LiteralPath (Join-Path $root 'Desktop\DeskTuk.lnk')) { throw 'Uninstall retained desktop shortcut' }
if((Get-ItemProperty -LiteralPath $runKey -ErrorAction SilentlyContinue).DeskTuk) { throw 'Uninstall retained startup entry' }
Write-Host 'Upgrade and uninstall preserve user files/config and remove owned shortcut/startup : PASS'
Write-Host "Fixture retained for inspection: $root"
