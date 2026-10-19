# DeskTuk Windows installer

Build on Windows with the MSVC toolchain. The only additional build tool is the
official, signed Inno Setup **6.7.3** compiler. DeskTuk Setup itself is offline and
does not install or require Inno Setup, PowerShell, Python or Rust.

```powershell
./scripts/install-inno-compiler.ps1
./scripts/build-installer.ps1
```

An existing compiler can instead be supplied with `-Compiler C:\path\ISCC.exe`.
`-SkipBuild` packages an already checked `target/release/DeskTuk.exe`. The version
comes from Cargo metadata. Output: `dist/DeskTuk-<version>-Setup-x64.exe` and its
SHA256 sidecar. The installer is not code-signed; signing requires the publisher's
own signing certificate and is not supplied by the Inno compiler's signature.

## Installation behavior

- Per-user, no administrator prompt. Default: `%LOCALAPPDATA%\Programs\DeskTuk`.
  The destination page remains editable on first install and updates; Windows
  root/network locations are disallowed. The selected folder is shown before install.
- Turkish and English wizard. Final button is **Bitir / Finish**, with optional
  app launch; silent installations do not launch the app.
- Start menu shortcut and optional desktop shortcut use the embedded brand's
  matching multi-resolution ICO. The ICO filename includes its content hash to
  avoid reusing a stale icon-cache entry after palette changes.
- App process and shortcuts share `DeskTuk.Desktop` AppUserModelID. Window classes
  supply the embedded large/small brand icons as well.
- Existing plain DeskTuk taskbar links are backed up under
  `%LOCALAPPDATA%\DeskTuk\ShortcutBackups`, then refreshed from the canonical
  shortcut (Unicode target, icon and AppID). Links with custom arguments are skipped.
  Windows is notified of the changed item/folder; Setup never forces taskbar pinning
  or restarts Explorer. Windows may retain an already loaded taskbar image until
  its next refresh; unpin/re-pin from the new Start menu shortcut is a fallback.
- A running DeskTuk receives normal WM_CLOSE before replacement. An existing
  startup opt-in is redirected to the installed EXE; absent startup stays absent.
- Uninstall removes owned binaries/shortcuts and its own matching startup value.
  `%APPDATA%\DeskTuk` configuration, referenced user files, unknown files in the
  install folder, and pre-existing pinned-shortcut backups remain intact.

## Isolated regression fixture

```powershell
./scripts/installer-regression-test.ps1 -Compiler target/tools/InnoSetup/ISCC.exe
```

This builds the same script with a fixture-only AppId and redirects desktop,
Start menu, pins, backups, startup registry and destination defaults into a unique
TEMP fixture. It operates the wizard using routed control messages, without moving
the physical pointer. It verifies a path with spaces/Turkish characters, visible
final Bitir, same-path upgrades, Unicode shell link targets/AppID/icon, pin backup,
custom-link exclusion, startup migration, uninstall and user-data preservation.
Logs/screenshots are retained in `target/installer-test`; fixture data is kept in
the printed TEMP location. Windows interactive-session access is required.

Official references: [Inno shortcut metadata](https://jrsoftware.org/ishelp/topic_iconssection.htm),
[Windows AppUserModelID](https://learn.microsoft.com/en-us/windows/win32/shell/appids).
