# DeskTuk – Phase 0 & 1 technical feasibility report

Date: 2026-10-13 · Branch: `claude/desktuk-phase-0-1-ief5sl`

## 0. Read this first: what was and was not verified

* This work was done in a **Linux container with no Windows machine**. DeskTuk.exe was
  cross-compiled with mingw-w64 and tested under **Wine 9.0 on a virtual X display (Xvfb)**.
* Wine implements the Win32 API but **not Windows Explorer**. Under Wine there is no real
  `Progman`/`WorkerW`/`SHELLDLL_DefView`, no Win+D and no DWM. **Nothing below about Win+D,
  Explorer restarts or Explorer window parenting has been verified on real Windows.** The Wine
  RAM and startup numbers are **not** Windows numbers.
* A Windows CI job (`.github/workflows/windows.yml`) and an in-app self-test were built to get
  those real measurements on GitHub's `windows-latest` runner. **They have not run yet**: pushing
  the branch failed with HTTP 403 (the Claude GitHub App has no write access to
  `ege-ozel/desktuk`). The commits exist only locally in the session until access is fixed.
* The "Master Development Prompt" mentioned in the task was not available in this session (the
  repository was empty). This phase was implemented from the Phase 0/1 brief alone.

## 1. Environment (Phase 0)

| item | found / chosen |
|---|---|
| Host OS | Ubuntu 24.04 (container), x86_64. No Windows available. |
| Rust | rustc 1.97.0, cargo 1.97.0 (stable) |
| Windows target | `x86_64-pc-windows-gnu` (added via rustup) + mingw-w64 GCC 13 / binutils 2.41 (`windres`). On Windows use the default MSVC toolchain; the CI job builds with MSVC. |
| Win32 bindings | **`windows-sys` 0.61.2**: Microsoft-maintained raw FFI bindings. No wrappers or runtime cost; functions are feature-gated so only the APIs used are compiled. |
| Resources | **`embed-resource` 3.0.12** (build-time only): embeds the manifest, icon and version info with `rc.exe` (MSVC) or `windres` (GNU). |
| Other crates | none |
| Test tools | Wine 9.0, Xvfb, ImageMagick (screenshots), a purpose-built fake `Progman` process (see §5.3) |

Win32 components used: user32 (windows, input, WinEvent hook), gdi32 (painting, screen
capture), shell32 (`SHGetFileInfoW`, `SHGetImageList`, `ShellExecuteExW`,
`SHGetKnownFolderPath`, `SHOpenFolderAndSelectItems`, `SHGetSetSettings`), comctl32 v6 (image
lists, via the manifest), ole32/combase (COM STA), dwmapi (Win11 rounded corners, cloak state),
advapi32 (registry reads) and kernel32/psapi (timing, memory counters). The executable's import
table contains only system DLLs.

## 2. Architecture

```text
main ─► app::run ──► config (CLI > %APPDATA%\DeskTuk\desktuk.cfg > defaults)
                 ├─► items::Item::load  (SHGetFileInfo: name, icon index, overlay)
                 ├─► controller window  (hidden, top-level: TaskbarCreated, timers, display changes)
                 ├─► WinEvent hook      (EVENT_SYSTEM_FOREGROUND → host)
                 └─► node::create ──► host.prepare() → CreateWindowExW → host.attached/place
```

The **replaceable hosting interface** (`src/host/mod.rs`):

```rust
pub trait DesktopHost {
    fn kind(&self) -> HostKind;
    fn prepare(&self) -> Result<CreateSpec, String>;          // parent/owner + styles
    fn attached(&self, node: HWND);
    fn place(&self, node: HWND, r: &RECT);                     // screen coordinates
    fn window_pos_changing(&self, node: HWND, wp: &mut WINDOWPOS) {}
    fn foreground_changed(&self, node: HWND, fg: HWND) {}
    fn is_healthy(&self, node: HWND) -> bool { true }          // parent/owner still alive?
    fn shell_restarted(&self, node: HWND) -> bool;             // re-create after Explorer restart?
}
```

The Node UI, items, configuration and self-test do not know which strategy is active.

Lifecycle and safety:

* If an experimental host cannot attach (no Explorer desktop found) DeskTuk falls back to
  `window`. It also falls back if the desktop does not come back within 30 s.
* A lost node is re-created. Loss is detected three ways: `WM_DESTROY`, an HWND turning
  invalid, or the parent/owner disappearing (2 s health check, experimental hosts only).
  `TaskbarCreated` also triggers re-creation.
* `ShellExecuteEx` runs with no internal borrow held, because it can pump messages and re-enter
  the window procedure.
* Clicking a Node never activates it (`MA_NOACTIVATE`), so it cannot jump above applications.

## 3. Desktop-hosting approaches compared

| approach | documented? | Win+D | interaction | Explorer restart | main risks | verdict |
|---|---|---|---|---|---|---|
| **A. Top-level tool window at bottom of z-order** (`window`) | Yes (only the "is the desktop foreground?" test uses Explorer class names) | Covered by the desktop unless handled. DeskTuk raises the Node to the topmost band while the desktop is foreground, then drops it back (Rainmeter "On desktop" technique). | Full | Unaffected | Raise/lower heuristic can misfire (e.g. stays topmost if a foreground event is missed). `HWND_BOTTOM` relies on the shell window being bottom-most (`SetShellWindow`). | **Implemented, default/fallback** |
| **B. Popup owned by `Progman`** (`owned`) | Owner relation is documented; `Progman` as owner is not | Expected to follow the desktop when Explorer raises it (unverified) | Full | Owner destroyed → node must be re-created | Cross-process owner attaches input queues (an Explorer hang can freeze DeskTuk and vice versa). Relies on `Progman` existing. | Implemented, **experimental** |
| **C. `WS_CHILD` of the icon-view host** (`child`) | No | Part of the desktop, so unaffected by Win+D | Full (above the icon view) | Destroyed or orphaned with its parent → re-create | Undocumented structure, which moved in Win11 24H2. Cross-process input-queue attach. Icon view may overdraw the node unless it has `WS_CLIPSIBLINGS` (logged at runtime). DWM/DPI behaviour of cross-process children is unverified. | Implemented, **experimental** |
| D. Behind the icons in the wallpaper `WorkerW` (spawned with undocumented message `0x052C`) | No | Unaffected | **None**: the icon list view receives the clicks | Same as C | Changes Explorer state for every user of the desktop; wallpaper engines collide | Not implemented: unsuitable for interactive Nodes |
| E. DLL injection / hooking into Explorer (Fences-style) | No | Full control | Full | Must re-inject | Stability, security-product flags, breaks with Explorer updates | Rejected for Phase 1/2 |
| F. AppBar (`SHAppBarMessage`) | Yes | Reserves screen edge | Full | Must re-register | Docks to edges and shrinks the work area; not a free-floating desktop panel | Not suitable |

## 4. Avoiding duplicate icons (desktop and Node) without touching files

Every option keeps the user's files where they are. They are ranked by safety:

1. **Do nothing (Phase 1 behaviour).** The Node references items and the desktop still shows them.
   Duplicates are visible.
2. **"DeskTuk manages the desktop" mode (recommended for Phase 2).** The user switches off
   Explorer's own *View → Show desktop icons* (a documented, per-user, reversible setting:
   `SHELLSTATE.fHideIcons`, readable via `SHGetSetSettings`). DeskTuk then shows **all** Desktop
   folder items in Nodes, plus an "Unsorted" Node for items not assigned anywhere, so nothing
   disappears from view. There are no duplicates, no file changes and one switch to undo.
   Phase 1 already logs the current value. Changing it programmatically and making Explorer
   refresh live still needs verification, so Phase 2 should first guide the user to toggle it.
3. **Folder-backed ("portal") Nodes.** A Node shows the contents of a user-chosen folder that
   isn't the Desktop, so nothing is duplicated. Users opt in to which folders they use.
4. Rejected options:
   * Setting `FILE_ATTRIBUTE_HIDDEN` on the files modifies user files and affects every app.
   * Moving files into a DeskTuk folder is explicitly forbidden.
   * Repositioning or hiding items in the desktop list view through cross-process `LVM_*`
     messages is undocumented, fragile, and reset by Explorer.
   * Hooking or injecting into Explorer to filter items: see §3 E.

## 5. Test results (actual)

### 5.1 Static checks (Linux, `x86_64-pc-windows-gnu`)

| check | result |
|---|---|
| `cargo build --release` | OK, no warnings |
| `cargo clippy --release` | OK, 0 warnings |
| `cargo fmt --check` | OK |
| `cargo test --release` (unit tests run under Wine) | 4 passed, 0 failed (layout, hit-testing, CLI position parsing, host names) |

### 5.2 Self-test under Wine 9.0 (`--selftest`, host `window`)

Binary SHA-256 `e2b457e5c3432941e7dae719c3dfff9ad82962eb0170cd33c81c54115adc92ec`. Raw log:
[`results/wine/selftest-window.log`](results/wine/selftest-window.log). Screenshots:
[Node](results/wine/node-initial.png), [stacking test](results/wine/stacking-test.png)
(the red test window covers half the Node; the clicked item stays below it).

| check | Wine result | meaning |
|---|---|---|
| initially_visible | PASS | hit-test and screen pixel both show the Node |
| drag_moves_node | PASS | moved exactly (−120, +80) |
| double_click_launch | PASS | synthetic double-click → `ShellExecuteEx` opened `hello.txt` in Notepad (pid reported) |
| original_file_untouched | PASS | path still exists after launch |
| app_window_covers_node | PASS | |
| click_does_not_raise_node | PASS | |
| visible_after_show_desktop | **FAIL (expected under Wine)** | Wine has no Win+D, so the test window stayed on top. **Must be measured on Windows.** |
| below_apps_after_show_desktop_toggle | PASS (trivially, since Win+D did nothing) | |
| fullscreen_covers_node | PASS | |
| visible_after_fullscreen_closes | PASS | |

### 5.3 Explorer-restart recovery logic (Wine + fake `Progman`)

To exercise the `owned`/`child` code paths, a small test program created
`Progman › SHELLDLL_DefView › SysListView32` windows and broadcast `TaskbarCreated`. That process
was killed and started again while DeskTuk ran. Logs:
[`results/wine/fakeshell-restart-child.log`](results/wine/fakeshell-restart-child.log),
[`results/wine/fakeshell-restart-owned.log`](results/wine/fakeshell-restart-owned.log).

* Attaching works for both hosts. The `child` Node renders above the fake icon view
  (top half of [this screenshot](results/wine/fakeshell-child-before-after-old-build.png);
  [after recovery with the fixes](results/wine/fakeshell-child-recovered.png)).
* **Bug found and fixed:** when the parent process was killed, the `child` Node window vanished
  **without `WM_DESTROY` reaching DeskTuk** (under Wine). The first build kept the stale handle
  and never re-created the Node
  (bottom half of [this screenshot](results/wine/fakeshell-child-before-after-old-build.png)). Fix: liveness checks
  (`IsWindow`) and a 2 s parent/owner health check for experimental hosts.
* **Second issue found and mitigated:** the re-created `child` Node was hit-testable but not
  painted, because the fresh desktop drew over it. Fix: a repaint 1 s after re-attaching. The
  fake icon view has no `WS_CLIPSIBLINGS`. Whether Explorer's real `SHELLDLL_DefView` has it
  decides how serious this is on Windows, so the `--diag` log now records it.
* After the fixes, both hosts re-created a visible Node about 1 s after the loss was detected,
  as soon as the fake desktop existed again. For `owned` this happened before `TaskbarCreated`
  arrived; for `child`, 6 ms after it.

### 5.4 Measurements

| metric | value | conditions |
|---|---|---|
| **Executable size** | **398,336 bytes (389 KiB)** | `x86_64-pc-windows-gnu`, `opt-level="s"`, LTO, 1 CGU, `panic=abort`, stripped; includes ~15 KB icon. The MSVC build size has not been measured yet (CI did not run). |
| Startup, `main()` → first painted Node | 122.8 / 133.1 / 125.9 / 124.6 / 133.1 ms (median 125.9) | **Wine 9.0**, 2 items, 5 runs. About 80–100 ms of this is `SHGetFileInfo` in Wine's shell32. Not a Windows figure. |
| Startup, process creation → first paint | 200.9 / 209.6 / 212.5 / 211.5 / 218.9 ms (median 211.5) | Wine, same runs |
| Working set, 2 s after start | 26.7–26.9 MB | **Wine**: includes Wine's own user-space DLL emulation. Wine reports `PrivateUsage` and GDI/USER object counts as 0. Not a Windows figure. |
| RAM / startup on Windows | **not measured** | `scripts/windows-integration-test.ps1` records them (self-reported and `Get-Process`) once it runs on Windows. |

## 6. Known limitations

* **No real-Windows verification yet**. This affects Win+D, Explorer restart, fullscreen
  interaction with real games/video, DWM rounding, mixed-DPI multi-monitor, Win10 vs Win11
  24H2+ differences, and memory and startup on Windows.
* `window` host: the Show-desktop handling is a heuristic driven by foreground changes. It is
  event-driven (no polling), but if Explorer changes what "Show desktop" foregrounds, the Node
  may be hidden under the desktop during Win+D.
* `owned`/`child` hosts: cross-process window relationships attach input queues. A hung
  Explorer can freeze DeskTuk's UI thread and vice versa. Item loading (`SHGetFileInfo`) is
  synchronous on the UI thread, so a slow network path would block it too.
* Only one Node. Its items come from the configuration file. There is no in-app add/remove, item
  drag-and-drop, keyboard navigation, shell context menu (`IContextMenu`), tray icon or
  autostart.
* Icons above 48 px use the 256 px "jumbo" list. Items that only have small icons look small at
  high DPI. Icons are not refreshed when a file's icon changes.
* Display names and icons are read once at startup. Renamed or deleted files show stale data
  (missing files are marked "(missing)" at load).
* The self-test needs an interactive desktop session (`SendInput`, screen capture). It changes
  real state (Win+D, Notepad, and an optional Explorer kill), so it is for test machines only.

## 7. Recommendation for Phase 2

1. **Ship the `window` host (conventional top-level tool window) as the default.** It uses only
   documented window-management APIs, cannot be taken down by Explorer, needs no re-attachment
   and keeps DeskTuk's UI thread independent of Explorer. Its only Explorer coupling is
   recognising the desktop as the foreground window during Show desktop, and failure there
   costs visibility during Win+D, not stability.
2. **Keep `owned` and `child` behind `--host` as experiments, and decide with data.** The first
   Phase 2 task is to run `scripts/windows-integration-test.ps1 -ExplorerRestart` on Windows 10
   22H2, Windows 11 23H2 and Windows 11 24H2/25H2, physical and VM, including mixed-DPI
   multi-monitor. Compare the per-host PASS/FAIL tables and the `defview_clipsiblings` value.
   Only adopt `child` if it passes everywhere, and given the input-queue coupling, not by
   default.
3. **Duplicates:** implement the "DeskTuk manages the desktop" mode (§4.2), with an Unsorted
   Node so every desktop item stays reachable, plus optional folder-backed Nodes (§4.3). Never
   modify file attributes or locations.
4. **Before adding features:** move item metadata/icon loading to a worker thread (COM STA),
   watch the Desktop folders (`SHChangeNotifyRegister`) for renames/deletes/icon changes, and
   add the shell context menu.
5. **Enable the CI job** by granting the Claude GitHub App write access, so every change gets a
   real-Windows self-test run, screenshots and RAM/startup numbers.
