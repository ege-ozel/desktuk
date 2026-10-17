# DeskTuk Phase 2 - Circles (design)

Status: **approved direction (owner, 2026-10-13)**. Answers the "What is DeskTuk for?"
question left open in `2026-10-13-phase-2-design.md`. Implemented by
`docs/superpowers/plans/2026-10-13-phase2-d-circles.md`.

## Purpose

Tidy the Windows Desktop **visually**. Desktop files stay exactly where they are; DeskTuk
only shows them grouped in round "circles" that open on hover. DeskTuk is for the Desktop
only - it is not a file browser for `C:\`.

## Owner decisions (2026-10-13)

| Topic | Decision |
|---|---|
| Layout | Radial: a round circle that opens into a ring of its files on hover (like the metronome dial). The block/Trello layout is not built. |
| Levels | At most three: circle > group > file. A group opens in the same ring. |
| Many files | A ring shows a page of 20; the wheel moves page by page ("21-40 / 300"). |
| New files | Collected in a built-in **Recent** circle. |
| Assigning | First by hand (drag & drop); then DeskTuk *suggests* rules ("12 .wav -> Music?") and the owner accepts. Nothing is assigned automatically. |
| Names | Circle and group names are editable. |
| Notes | Out of scope for now. |
| Resources | RAM and CPU must stay minimal: idle CPU 0, memory at or below v1. |
| Versioning | v1 = Phase 1 (commit `39ef8a9` on `main`). v2 = Phase 1 + Phase 2. If v2 uses more memory than v1, revert to v1. |

## Concepts

* **Circle** - named, freely positioned on the desktop; holds file references directly and in groups.
* **Group** - named, inside one circle. No groups inside groups.
* **Item** - a *path*. DeskTuk never moves, renames, deletes or changes a file. An item lives
  in at most one place.
* **Recent** - built-in, cannot be renamed or deleted: every entry of the user and public
  Desktop folders that is in no circle, newest first (newest of creation/modification time).
  New downloads therefore show up first without DeskTuk storing a "seen" list.
  *(This is simpler than "arrived since the last session", which was discussed; it needs no
  extra state and old unsorted files simply sit on later pages.)*

## Behaviour

**Closed circle** (88 px at 96 DPI): name and item count, accent rim. Recent is tinted blue.

* Pointer entry or click opens the ring immediately, without hover dwell or opening
  animation. Drag: moves the circle (position saved).
* Right-click: New circle, New group, Rename..., Delete circle (confirmation; items go back to
  Recent), Exit. On Recent: New circle, Exit.
  **Owner follow-up (2026-10-13):** all circle/Recent and file menus, plus the panel menu,
  include the checked **Masaüstü öğelerini göster** option. It toggles Windows' desktop
  icons via the desktop view's visibility flag and persists Explorer's own preference.
  DeskTuk circles stay visible; no file paths or attributes change. The checkmark is read
  from Explorer whenever the menu opens. If the desktop view cannot be reached the entry
  is disabled; a failed change reports an error. Startup and exit never change visibility.
* Files dropped from Explorer onto a circle are assigned to it; dropped onto Recent they are
  unassigned.

**Open ring**, one at a time, centred on its circle (shifted inwards
near a screen edge):

* Centre: title and page indicator. Up to 20 slots: up to 8 on an inner ring, 12 on an outer ring.
  **Owner follow-up (2026-10-13):** icons use the current Windows desktop icon size in pixels
  and size-specific shell images, with the desktop label font. The ring grows with the icons;
  large icons or small monitors reduce the page capacity to keep it inside the work area.
  Size changes take effect the next time a ring opens. When Explorer is unavailable,
  fall back to 48 pixels scaled for the circle's DPI. Source images without a high-resolution
  variant can still appear soft, as on the desktop.
* Circle view: its groups first, then its files. Group view: a Back slot, then the group's files.
  Recent view: suggestions first, then unassigned Desktop files.
* Click a file: opens it with its default verb, the ring closes. Click a group: enters it.
  Click Back: returns. Click the centre: next page (wraps) - for when Windows' "scroll inactive
  windows" is off. Wheel: next/previous page.
* Right-click a file: the real shell menu (Plan A) plus "Remove from circle". Right-click a
  group: Rename group, Delete group (files move up to the circle). Right-click a suggestion:
  Add / Not now. Right-click elsewhere: the circle menu.
* Drag a file slot onto a group slot, the Back slot, another circle or Recent: moves it there.
* Opens immediately, closes over 110 ms after the pointer leaves the disc. No timer runs
  when nothing animates.

**Suggestions:** an extension points at circle *c* when *c* holds at least 2 files with it and
strictly more than any other circle; generic extensions (`lnk`, `url`, `tmp`) never count. At
most 3 are shown. "Not now" hides one for the session.

**Rename box:** a small ordinary popup with an edit field over the circle. Enter or clicking
away commits, Esc cancels. Names: no `|`, no control characters, 1-40 characters, unique
(case-insensitive) among circles / among a circle's groups ("Music 2").

## Persistence

Same file, `%APPDATA%\DeskTuk\desktuk.cfg`, new keys (all paths are references):

```text
ui=circles
recent=1600|80
circle=Music|120|340
group=Music|Titled Project
citem=Music||C:\Users\me\Desktop\notes.txt
citem=Music|Titled Project|C:\Users\me\Desktop\song.wav
```

`host=`, `x=`, `y=`, `item=` keep their Phase 1 meaning. The panel's `item=` list and the
circle lines are both preserved whichever UI runs.

## UI selection and compatibility

* `ui=circles|panel` in the config, `--ui circles|panel` on the command line (the CLI choice
  is saved). Default: **circles**, also for a Phase 1 config without `ui=`.
* `--selftest` always runs the panel, so the CI integration test is unchanged.
* `--ui panel` gives the exact Phase 1 behaviour back without reinstalling v1.

## Architecture

* Pure, unit-tested: `circles::model` (board + config lines), `geom` (slots, hit testing,
  paging, disc mask, animation), `view` (board -> slots), `suggest`, `desktop::recent`.
* Win32: each circle is a small `WS_EX_LAYERED` tool window painted once with
  `UpdateLayeredWindow` (per-pixel alpha, so the corners are click-through); the painting
  surface is freed right after. Each circle has its own `WindowHost` (bottom of the z-order,
  raised during "Show desktop"). The ring is one layered window **owned by** its circle, so it
  always stays just above it; it is created on open and destroyed on close.
* Desktop folders are watched with `FindFirstChangeNotificationW`; the main loop waits on
  those handles with `MsgWaitForMultipleObjectsEx` (no extra thread, no polling), and re-lists
  250 ms after the last change.
* Shell renames (Plan A `watch.rs`) rewrite stored paths. Icons come from Plan A's loader
  thread, only for the 20 visible slots, and are released when the ring closes; then
  `CoFreeUnusedLibrariesEx` runs and the working set is trimmed.
* RefCell rule (AGENTS.md): all circles code uses `try_borrow*` and never keeps a borrow
  across a menu, `ShellExecuteExW`, `DestroyWindow` or a rename box.

## Resource budget (gate for merging)

Measured with `scripts/bench-memory.ps1` (3-run medians) against v1; numbers below are
from the prototype the plan was written from (Windows 11, 2026-10-13):

| Scenario | v1 (Phase 1) | v2 circles |
|---|---|---|
| idle, panel / Recent only | 4 644 KB commit, 46 modules | 2 040 KB commit, 30 modules |
| idle, 5 circles x 60 files | - | 2 132 KB, 30 modules |
| showing 20 real Desktop files | 7 780 KB, 63 modules (panel) | 9 344 KB while the ring is open, 8 748 KB after it closed (private working set 104 KB) |
| idle CPU over 10 s | 0 ms | 0 ms |

The jump when real icons are shown is the shell loading icon handlers for those file types
(thumbcache, cloud-drive extensions, WIC...). v1 pays the same for the same files.

Gate (checked by Plan D, Task D6): with no ring opened, commit and modules must not exceed v1's
panel; idle CPU must be 0. The ring-open number is reported next to v1 showing 20 Desktop files
(about 7.8 MB here). If the gate fails, the owner decides between fixing and reverting to v1.

## Out of scope

Notes; the remaining Plan C work (on hold);
a "generic icons" mode that would avoid loading per-file icon handlers
(possible follow-up if the owner wants even lower memory after browsing rings).

## Risks

* Wheel messages reach a non-activated window only when "Scroll inactive windows" is on
  (Windows default); the centre click is the fallback.
* Explorer drag & drop into layered windows relies on `WS_EX_ACCEPTFILES`; verified manually.
* Dropping files from outside the Desktop works and stores those paths as well.


## Owner follow-up: tray and startup (2026-10-14)

Plan D9 adapts the postponed B3 task to both UIs on the existing circles branch.
Normal instances register the resource icon on the hidden controller, re-register
on TaskbarCreated and remove it before destroying windows. The version-4 callbacks
use LOWORD(lParam) for events and HIWORD(lParam) for icon IDs; a legacy fallback
keeps mouse access if setting the callback version fails. Click/keyboard activation
and a second launch restore the selected UI. The tray menu reads the current startup
value, toggles the quoted UTF-16 per-user Run command, and reports registry errors.
Restoration performs no file or desktop visibility changes. Temporary foreground
ordering remains inside the host layer and returns to normal on a foreground change.
Startup is opt-in, with bounded retries if the taskbar is not ready at login; after
successful registration there is no retry timer or extra thread. The owner explicitly
requested enabling startup after deployment to address the empty desktop on login.


## Owner fixes: open-centre dragging and taskbar focus (2026-10-14)

The open ring overlays its circle. Centre presses now route to that circle's existing
drag capture; crossing the Windows drag threshold closes the ring and moves the
circle. A short click keeps centre paging. Pending centre presses suppress leave
closing, and event coordinates preserve the grab offset even near screen edges.
WindowHost treats taskbar, secondary taskbar and overflow roots/descendants as
desktop interactions, preserving the current ordering. Generic app/UWP/XAML windows
still trigger normal background ordering. No new polling, threads or dependencies.


The tray menu also includes Masaüstü öğelerini göster (owner, 2026-10-14). It reuses
the same desktop_view command, reads Explorer's live visibility when opened, and
changes it only on explicit selection. Startup and exit do not alter this preference.
