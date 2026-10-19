; Build through scripts/build-installer.ps1. Fixture builds redirect all shell writes.
#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef BrandRevision
  #error BrandRevision must identify the packaged ICO
#endif
#ifndef BuildRoot
  #error BuildRoot must point to the checked release directory
#endif
#ifndef OutputRoot
  #define OutputRoot "..\dist"
#endif
#ifdef Fixture
  #define Identity "DeskTuk.InstallerFixture"
  #define OutputName "DeskTuk-InstallerFixture"
  #define MenuDir "{param:FIXTUREROOT}\Start Menu"
  #define DesktopDir "{param:FIXTUREROOT}\Desktop"
  #define StateDir "{param:FIXTUREROOT}\State"
  #define PinDir "{param:FIXTUREROOT}\Pins"
  #define RunKey "Software\DeskTuk.InstallerFixture\Run"
#else
  #define Identity "DeskTuk.Desktop"
  #define OutputName "DeskTuk-" + AppVersion + "-Setup-x64"
  #define MenuDir "{userprograms}"
  #define DesktopDir "{userdesktop}"
  #define StateDir "{localappdata}\DeskTuk"
  #define PinDir "{userappdata}\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar"
  #define RunKey "Software\Microsoft\Windows\CurrentVersion\Run"
#endif

[Setup]
AppId={#Identity}
AppName=DeskTuk
AppVersion={#AppVersion}
AppPublisher=DeskTuk
#ifdef Fixture
DefaultDirName={param:FIXTUREROOT}\Programs\DeskTuk
#else
DefaultDirName={localappdata}\Programs\DeskTuk
#endif
DisableDirPage=no
UsePreviousAppDir=yes
AppendDefaultDirName=no
AllowRootDirectory=no
AllowNetworkDrive=no
AllowUNCPath=no
DisableProgramGroupPage=yes
DisableWelcomePage=no
DisableFinishedPage=no
AlwaysShowDirOnReadyPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
WizardStyle=modern
SetupIconFile=..\res\desktuk.ico
UninstallDisplayIcon={app}\desktuk-{#BrandRevision}.ico
UninstallDisplayName=DeskTuk
OutputDir={#OutputRoot}
OutputBaseFilename={#OutputName}
Compression=lzma2
SolidCompression=yes
CloseApplications=yes
RestartApplications=no
SetupLogging=yes
ChangesAssociations=yes
VersionInfoDescription=DeskTuk Setup
VersionInfoProductName=DeskTuk

[Languages]
Name: "turkish"; MessagesFile: "compiler:Languages\Turkish.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Messages]
turkish.ButtonFinish=&Bitir
turkish.FinishedHeadingLabel=[name] kurulumu tamamlandı
turkish.ClickFinish=Kurulumdan çıkmak için Bitir düğmesine tıklayın.
english.ButtonFinish=&Finish

[CustomMessages]
turkish.DesktopIcon=Masaüstü kısayolu oluştur
turkish.Launch=DeskTuk'u başlat
turkish.CloseFailed=DeskTuk kapatılamadı. Sistem tepsisinden DeskTuk'u kapatıp yeniden deneyin.
english.DesktopIcon=Create a desktop shortcut
english.Launch=Launch DeskTuk
english.CloseFailed=DeskTuk could not close. Exit DeskTuk from the system tray and retry.

[Tasks]
Name: "desktopicon"; Description: "{cm:DesktopIcon}"

[Files]
Source: "{#BuildRoot}\DeskTuk.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\res\desktuk.ico"; DestDir: "{app}"; DestName: "desktuk-{#BrandRevision}.ico"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Icons]
Name: "{#MenuDir}\DeskTuk"; Filename: "{app}\DeskTuk.exe"; WorkingDir: "{app}"; IconFilename: "{app}\desktuk-{#BrandRevision}.ico"; AppUserModelID: "DeskTuk.Desktop"
Name: "{#DesktopDir}\DeskTuk"; Filename: "{app}\DeskTuk.exe"; WorkingDir: "{app}"; IconFilename: "{app}\desktuk-{#BrandRevision}.ico"; AppUserModelID: "DeskTuk.Desktop"; Tasks: desktopicon

[Run]
#ifndef Fixture
Filename: "{app}\DeskTuk.exe"; WorkingDir: "{app}"; Description: "{cm:Launch}"; Flags: nowait postinstall skipifsilent
#endif

[Code]
procedure NotifyPath(Event: Integer; Flags: Cardinal; Path: String; Unused: Integer);
  external 'SHChangeNotify@shell32.dll stdcall';
function PostClose(Window: HWND; Msg: Cardinal; WParam, LParam: Integer): Boolean;
  external 'PostMessageW@user32.dll stdcall';

#ifdef Fixture
function InitializeSetup: Boolean;
var
  Root: String;
begin
  Root := ExpandConstant('{param:FIXTUREROOT}');
  Result := (Root <> '') and DirExists(Root);
  if not Result then MsgBox('Fixture build requires an existing /FIXTUREROOT directory.', mbError, MB_OK);
end;
#endif

function CloseDeskTuk: Boolean;
var
  Window: HWND;
  Attempt: Integer;
begin
  Result := True;
#ifndef Fixture
  Window := FindWindowByClassName('DeskTuk.Controller');
  if Window <> 0 then begin
    PostClose(Window, $0010, 0, 0);
    for Attempt := 1 to 80 do begin
      if not CheckForMutexes('Local\DeskTuk.SingleInstance') then exit;
      Sleep(100);
    end;
    Result := False;
  end;
#endif
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := '';
  if not CloseDeskTuk then Result := CustomMessage('CloseFailed');
end;

procedure RepairPinnedLinks;
var
  Find: TFindRec;
  Folder, Link, Backup, Target, Canonical: String;
  Shell, Shortcut: Variant;
begin
  Folder := ExpandConstant('{#PinDir}');
  Canonical := ExpandConstant('{#MenuDir}\DeskTuk.lnk');
  if not FileExists(Canonical) then exit;
  Shell := CreateOleObject('WScript.Shell');
  if FindFirst(Folder + '\*.lnk', Find) then begin
    try
      repeat
        Link := Folder + '\' + Find.Name;
        try
          Shortcut := Shell.CreateShortcut(Link);
          Target := Shortcut.TargetPath;
          // Only plain DeskTuk launchers, never arbitrary user links/arguments.
          if (CompareText(ExtractFileName(Target), 'DeskTuk.exe') = 0) and
             (Pos('desktuk', Lowercase(Find.Name)) = 1) and
             (Shortcut.Arguments = '') then begin
            Backup := ExpandConstant('{#StateDir}\ShortcutBackups\') + Find.Name;
            if not FileExists(Backup) then begin
              ForceDirectories(ExtractFileDir(Backup));
              if not FileCopy(Link, Backup, True) then
                RaiseException('Cannot back up pinned DeskTuk shortcut');
            end;
            // Copy the canonical shell link, including AppUserModelID, retaining
            // the pinned filename/order. Do not force a pin/unpin operation.
            if not FileCopy(Canonical, Link, False) then
              RaiseException('Cannot refresh pinned DeskTuk shortcut');
            NotifyPath($00002000, $0005, Link, 0);
            Log('Updated pinned DeskTuk shortcut: ' + Link);
          end;
        except
          Log('Pinned shortcut refresh: ' + GetExceptionMessage);
        end;
      until not FindNext(Find);
    finally
      FindClose(Find);
    end;
    NotifyPath($00001000, $0005, Folder, 0);
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Previous: String;
begin
  if CurStep = ssPostInstall then begin
    RepairPinnedLinks;
    // Preserve the owner's startup opt-in, only update its executable location.
    if RegQueryStringValue(HKCU, '{#RunKey}', 'DeskTuk', Previous) and (Previous <> '') then
      if not RegWriteStringValue(HKCU, '{#RunKey}', 'DeskTuk',
        '"' + ExpandConstant('{app}\DeskTuk.exe') + '"') then
        RaiseException('Cannot migrate DeskTuk startup path');
  end;
end;

procedure CurPageChanged(CurPageID: Integer);
begin
  Log('Wizard page: ' + IntToStr(CurPageID));
  if CurPageID = wpFinished then begin
    WizardForm.NextButton.Caption := SetupMessage(msgButtonFinish);
    WizardForm.BackButton.Visible := False;
    Log('Finished button: ' + WizardForm.NextButton.Caption);
  end;
end;

function InitializeUninstall: Boolean;
begin
  Result := CloseDeskTuk;
  if not Result then MsgBox(CustomMessage('CloseFailed'), mbError, MB_OK);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Current: String;
begin
  if CurUninstallStep = usUninstall then
    if RegQueryStringValue(HKCU, '{#RunKey}', 'DeskTuk', Current) and
       (CompareText(Current, '"' + ExpandConstant('{app}\DeskTuk.exe') + '"') = 0) then
      RegDeleteValue(HKCU, '{#RunKey}', 'DeskTuk');
  // APPDATA config and pre-existing shortcut backups deliberately remain.
end;
