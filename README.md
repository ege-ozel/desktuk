# DeskTuk

DeskTuk groups Windows Desktop files into movable **circles** that open on hover. Files remain
where they are; circles hold references to them. The Phase 1 **Node** panel remains available
with `--ui panel`.

It is written in Rust against raw Win32 (`windows-sys`). It uses no Electron, Tauri, WebView
or GUI framework.

> Windows MSVC builds, unit tests and fixture checks have been run. The nine manual circles
> checks remain **not verified**; see [Plan D](docs/superpowers/plans/2026-10-13-phase2-d-circles.md).
> The original proof-of-concept report is [here](docs/PHASE-0-1-REPORT.md).

## Phase 1 panel (`--ui panel`)

* Shows one Node with 2–4 real items, using the same icons as Explorer (system image list,
  including the shortcut arrow) and the same display names.
* Double-click opens an item through `ShellExecuteEx`, exactly as Explorer would. Files are only
  **referenced by path**. DeskTuk never moves, renames, deletes, hides or changes them.
* Drag the Node by its header or any empty area. The position is saved.
* Right-click opens a menu: *Open*, *Open file location*, the active host, and *Exit DeskTuk*.
* Per-monitor DPI aware (PerMonitorV2).
* The desktop hosting strategy can be swapped (`--host`):
  * `window` (default): an ordinary top-level tool window kept at the bottom of the z-order. It
    raises itself while "Show desktop" is active.
  * `owned` (experimental): a popup owned by Explorer's `Progman` window.
  * `child` (experimental): a child window of Explorer's desktop icon-view host.

## Build

Windows (MSVC, recommended):

```powershell
# Rust from https://rustup.rs (default x86_64-pc-windows-msvc toolchain, Build Tools + Windows SDK)
cargo build --release
# -> target\release\DeskTuk.exe
```

Cross-compiling from Linux (this is how the PoC was built):

```sh
sudo apt install gcc-mingw-w64-x86-64 binutils-mingw-w64-x86-64
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
# -> target/x86_64-pc-windows-gnu/release/DeskTuk.exe
```

Checks: `cargo fmt --check`, `cargo clippy --release -- -D warnings`, `cargo test --release`.
From Linux, add `--target x86_64-pc-windows-gnu` and run the tests with Wine
(`CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER=wine64`).

## Run

Double-click `DeskTuk.exe`. The built-in Recent circle appears near the top-right of the primary
monitor and lists unassigned Desktop entries. The configuration is written to
`%APPDATA%\DeskTuk\desktuk.cfg`:

```text
host=window
ui=circles
recent=1500|40
circle=Music|300|300
group=Music|Live
citem=Music||C:\Users\me\Desktop\song.wav
item=C:\Users\me\Desktop\Notes.txt
item=C:\Users\me\Desktop\Visual Studio Code.lnk
```

For the panel, run `DeskTuk.exe --ui panel` and edit `item=` lines to choose its files. Both
item collections survive saves in either UI. Exit through the right-click menu.

```text
DeskTuk.exe [options] [item paths...]
  --host <window|owned|child>  Desktop hosting strategy (default: window)
  --ui <circles|panel>         circles (default) or the Phase 1 panel
  --pos <x,y>                  Node position in screen pixels
  --log <file>                 Write a diagnostic log (timings, memory, shell structure)
  --exit-after <ms>            Quit after the given time (for measurements)
  --diag                       Log desktop/shell diagnostics and exit
  --selftest                   Automated integration self-test (moves mouse, presses Win+D)
  --selftest-explorer-restart  Self-test that also kills and restarts explorer.exe
  --shots <dir>                Self-test screenshots
  --no-save, --allow-multiple
```

## Circles (Phase 2)

By default DeskTuk shows **circles** instead of the panel. *Recent* (blue) holds every Desktop
file that is in no circle, newest first. Right-click a circle for *New circle*, *New group*,
*Rename*, *Delete circle*. Hover a circle to open its ring of files (20 per page; the wheel or
a click on the centre turns the page). Drop files from Explorer onto a circle to add them, drag
a file out of a ring onto another circle or group to move it, right-click a file for the real
shell menu and *Remove from circle*. Ring icons follow the current Windows desktop icon size
and use the desktop label font. The ring grows as needed; large icons or small screens show
fewer items per page. Reopen a ring after changing the desktop icon size.
Files are never moved or changed; circles only reference
them. `--ui panel` (saved) brings back the Phase 1 panel. Design:
[`docs/superpowers/specs/2026-10-13-circles-design.md`](docs/superpowers/specs/2026-10-13-circles-design.md).

## Testing on Windows

Use a test machine or VM where possible. The automated test moves the mouse, sends Win+D and
opens and closes Notepad. With `-ExplorerRestart` it also kills and restarts `explorer.exe`.

### Automated

```powershell
cargo build --release
powershell -ExecutionPolicy Bypass -File scripts\windows-integration-test.ps1 `
    -Exe target\release\DeskTuk.exe -Out test-results -ExplorerRestart
```

For each host (`window`, `owned`, `child`) the script runs `DeskTuk.exe --selftest` with a
shortcut and a text file it creates on your Desktop. Each self-test checks:

| check | expected behaviour |
|---|---|
| `initially_visible` | Node is hit-testable **and** its accent strip is on screen (pixel probe) |
| `drag_moves_node` | Dragging the header moves the Node by the drag delta |
| `double_click_launch` / `original_file_untouched` | Double-click opens the shortcut; the file is still at its path |
| `app_window_covers_node` | A normal application window covers the Node |
| `click_does_not_raise_node` | Clicking the Node does not lift it above that application |
| `visible_after_show_desktop` | After Win+D the Node is visible on the desktop |
| `below_apps_after_show_desktop_toggle` | After Win+D again the application is back on top |
| `fullscreen_covers_node` / `visible_after_fullscreen_closes` | Fullscreen windows cover the Node; it returns afterwards |
| `survives_explorer_restart` | After Explorer is killed and restarted, the Node is visible again |

Then it takes 5 RAM/startup measurements and verifies that the test files' hash, attributes and
timestamps and the Desktop listing are unchanged. Logs and PNG screenshots go to `test-results\`.

The same steps run in CI (`.github/workflows/windows.yml`) on GitHub's `windows-latest` runner.

### Manual checklist

1. Start `DeskTuk.exe`. The Node shows Desktop items with the same icons/names as the desktop.
2. Double-click each item: it opens like a desktop double-click. Shortcuts honour their *Start in*.
3. Drag the Node, exit, restart: the position is kept.
4. Open some windows over it: they cover the Node. Click the Node: it stays below them.
5. Press Win+D: the Node should remain visible. Press Win+D again: the windows return on top.
6. Start a fullscreen video or game: the Node must not appear over it.
7. Task Manager → *Windows Explorer* → *Restart*: the Node should stay or come back within seconds.
8. Change display scaling or move the Node to a monitor with a different DPI: it rescales.
9. Repeat with `--host owned` and `--host child --log %TEMP%\desktuk.log` and compare.

## Layout

```text
src/main.rs        entry point
src/app.rs         state, start-up, controller window, Explorer-restart recovery
src/node.rs        Node window: layout, painting, input, context menu
src/items.rs       shell items: display name, icon, ShellExecuteEx, open location
src/host/          DesktopHost trait + window / owned / child strategies
src/shell.rs       read-only inspection of Explorer's desktop windows & settings
src/config.rs      command line and %APPDATA%\DeskTuk\desktuk.cfg
src/diag.rs        log file, startup timing, memory/GDI/USER counters
src/selftest.rs    automated integration self-test
res/               manifest (PerMonitorV2, Common Controls v6), icon, version info
scripts/           Windows integration test, icon generator
docs/              feasibility report and raw test logs
```
