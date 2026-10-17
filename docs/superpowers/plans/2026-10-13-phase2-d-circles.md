# Phase 2 / Plan D - Circles

Status: implementation D1-D13 complete; owner-authorized consolidation D14 below.
Branch: `phase2/d-circles`. Merge target: `main`, through PR #3.
Design: [circles design](../specs/2026-10-13-circles-design.md).
Start with [CONTEXT.md](CONTEXT.md), not the historical implementation samples.

## Completed implementation

| Task | Delivered behavior |
|---|---|
| D1 | Circle board, reference assignment and backwards-compatible config |
| D2 | Ring geometry, paging, alpha mask and animation curve |
| D3 | Read-only Desktop listing, Recent, suggestions and ring contents |
| D4 | Layered surface and circle/ring drawing |
| D5 | Circle/ring windows, hover, menus, rename and start-up |
| D6 | v1/v2 resource gate, integration perf runs and usage |
| D7 | Native desktop icon size/resolution and adaptive ring capacity |
| D8 | Explicit Windows desktop-icon visibility toggle |
| D9 | Tray recovery, second launch and optional Windows startup |
| D10 | Open-centre drag, taskbar focus recovery and input-state corrections |
| D11 | Desktop visibility in the tray menu |
| D12 | Compact repository context and targeted reading instructions |
| D13 | Host-aware capture cleanup preserving visibility after tray reveal |

- [x] D1-D13 implementation and recorded automated checks.
- [x] Owner-approved deployment of runtime source `7e8c557`.
- [x] Configuration, startup command and desktop visibility preserved.

Detailed original steps, code samples, measured resource tables, manual checklists,
CI corrections and deployment evidence are retained without removal in
[the implementation archive](../archive/2026-10-13-phase2-d-circles-implementation.md).
Search the archive for a task heading/evidence label only when that history is needed.
Historical unchecked/manual claims there are not new implementation tasks.

## Current validation and limits

Runtime source `7e8c557`: MSVC fmt/clippy/release build and 46 passing tests
(one ignored visual preview). Both Windows CI runs passed, including tray,
open-ring and runner-only desktop/Explorer integration. Installed executable:
`D:/CodeSpace/desktuk/DeskTuk.exe`, SHA256:
`50EAAFD553F87F07CF4E1D7F8C20576FE984F582252917C88356175FFB0BB151`.

The owner's physical mouse, login/reboot, actual Explorer restart, menu/show-hide,
alternate icon-size/DPI and D5 manual interaction checks remain unperformed.
These are verification limits, not reproduced defects. The reported circle
smoothness issue awaits owner testing; no appearance change was made.

PR #3 was previously closed without merge at the owner's request. The owner then
explicitly requested merging all work; that newer instruction authorizes reopening
and merging it. The newer owner instruction then removed the PR requirement and permits direct main commits. Query GitHub for its live state; do not infer it from historical notes.

## Task D14: Consolidate branches and reduce default reading (owner, 2026-10-17)

Goal: collect all unique repository work into main through commits/branches under the latest owner instruction, leave no
open PR/issues, and remove repeated long-plan/stale-checkout reads without a runtime
refactor. Do not discard user data or claim that manual checks have been performed.

- [x] Incorporate the old report/gitignore branch while preserving its commit ancestry.
      Its root report is byte-identical to docs/PHASE-0-1-REPORT.md; keep one copy.
- [x] Retain the complete D1-D13 plan in the archive; replace this active plan with
      a small task/evidence index. Keep CONTEXT.md under 100 lines.
- [x] Add default search exclusions for historical code, captures and generated files.
      Ignore known local scratch copies without deleting their contents.
- [x] Preserve the untracked, workstation-specific partitions benchmark as an archived
      experiment, with its original assumptions and lack of runtime validation stated.
- [x] Incorporate the startup checkout's unique CI change: run CI for PRs and main,
      avoiding duplicate phase2 push + PR runs. Retain all Windows checks.
- [x] Run required MSVC fmt/clippy/tests, archive/duplicate checks and PowerShell
      syntax validation before the consolidation commit.

Reconciliation: startup tracked changes are the reading-guidance block already in
the feature branch, the CI trigger change incorporated above, and line-ending-only
differences. Before resetting those superseded copies, retain exact file snapshots
and a diff under ignored target/repo-consolidation/. Retain untracked executables,
captures, config copies and scripts through ignore rules; do not delete them.
After integrating, fast-forward the startup checkout to main and retire only branches
whose commits are confirmed ancestors of origin/main. Preserve the circles checkout
and any unrelated work; direct main integration is explicitly owner-authorized.

Validation: cargo fmt, cargo clippy --release --locked -- -D warnings,
cargo test --release --locked (46 pass, one ignored preview). Archived plan content
matches the original; duplicate report blob IDs match. Archived benchmark is syntax
checked only, not a supported general benchmark (fixed 96 DPI/48-pixel icons,
geometry and historical source labels). Runtime behavior/dependencies are unchanged.
Current CI and merge/closure status are live GitHub facts, not promises in this file.


## Task D15: Smooth the circle edges (owner screenshot, 2026-10-17)

Preserve round closed circles, native text/icons, dimensions and hit testing.
Replace GDI's hard inner ellipse boundaries with per-pixel coverage blending;
use the same centered coverage as the existing premultiplied outer alpha mask.
Apply it to ring/centre/group discs too, without new DLLs/dependencies, idle timers
or persistent supersampled surfaces. Keep output opaque until the final disc mask.

- [x] Reproduce the aliased inner rim with a native DIB regression at desktop DPIs.
- [x] Implement coverage blending and verify centered/symmetric inner boundaries.
- [x] Run required MSVC checks, render/inspect before-after previews, release build
      and isolated circle/open-ring fixtures. Use CI for the tray/integration fixture.
- [x] Commit/push directly to main under the owner's current workflow preference.
- [x] Install the checked release and restart the app under the owner's explicit
      update/restart authorization; preserve layout, startup and desktop settings.


D15 evidence (Windows MSVC, 2026-10-17):

* RED: the native DIB regression failed at 96 DPI on the original hard GDI ellipse.
* GREEN: inner-edge intermediate colors and mirror symmetry pass at 96/120/144/192
  DPI. MSVC fmt, locked release clippy with -D warnings and tests pass: 47 tests,
  one ignored preview. The ignored native preview was also run explicitly and passed.
* Before/after circle and ring BMPs were visually inspected. The inner rim is now
  blended and centered; native text/icons, layout and hit testing are unchanged.
  No new dependencies, DLL families, threads, idle timers or persistent buffers.
* Release SHA256: 5E7BC9BE63F2EA12141D4F453FC06FED96AC6D9ED8E73976014EDC42B0D30028.
  Ready at target/capture-cleanup/release/DeskTuk.exe in the secondary checkout.
* Isolated circles/open-ring fixtures passed UI persistence, missing-reference
  refresh, exact Recent/user circle movement, centre-click paging, restart position
  persistence and unchanged reference files. Physical input is not established.
* Automatic approval review rejected installation: it judged the prior install
  approval insufficient for this new runtime drawing change. No install command
  executed, so the installed app is still the D13 version. New installation needs
  explicit owner approval; do not retry or alter the installed app before that.
  Tray/integration are covered by CI without touching the owner's running app.


D15 owner-approved deployment (2026-10-17):

* The owner approved installing the newly smoothed drawing release after the prior
  automatic-review block. Revalidated main runtime source bafb842 and its successful
  Windows CI before installation; source SHA256 matched the checked D15 release.
* Closed the stable installed app through normal controller WM_CLOSE; backed up its
  prior executable under target/capture-cleanup/previous-installed.exe; installed
  the checked release and restarted it with its stable D:/CodeSpace/desktuk path.
* Installed SHA256: 5E7BC9BE63F2EA12141D4F453FC06FED96AC6D9ED8E73976014EDC42B0D30028.
  Process 7588 responded and logged tray initialization. Configuration hash,
  Run/DeskTuk and Explorer HideIcons preference were unchanged.
* Local deployment evidence: target/capture-cleanup/deployment.json in the secondary
  checkout. Preview artifacts there: circle-before.bmp, circle-after.bmp, ring-after.bmp.
  No physical cursor movement, Explorer restart, desktop preference change or user
  file change occurred. The owner's final visual assessment remains a manual check.
* Refresh CONTEXT.md to remove superseded pre-D15 drawing/deployment statements.
  This follow-up changes records only; the installed runtime source remains bafb842.


## Task D16: Reduce hover dwell (owner, 2026-10-17)

- [x] Reduce closed-circle hover dwell from 250 ms to 150 ms; retain the separate
      160 ms opening animation, click behavior and drag handling.
- [x] Update the behavior spec/context; run required MSVC fmt/clippy/release tests
      and release build. No timing-constant-only unit test or new dependency.
- [x] Install/restart the checked release with configuration/startup/visibility
      preserved; record the result and commit/push directly to main.

D16 validation: MSVC fmt, locked release clippy with -D warnings, 47 passing
release tests (one ignored preview) and release build passed. Only HOVER_MS changed;
OPEN_MS remains 160 ms. Ready binary: target/release/DeskTuk.exe, SHA256
C7AD4AC8AEE34616B417A2990FBDC960DB4F8ADD2CAE218FF7A0F75B8C569FB4.
Automatic approval review initially rejected installation pending explicit approval.
The owner then explicitly approved this release's installation/restart. Installed
the checked binary at D:/CodeSpace/desktuk/DeskTuk.exe after normal controller
WM_CLOSE; process 7140 responded and initialized its tray. Installed SHA256 matches
the checked release above. Configuration hash, Run/DeskTuk and Explorer HideIcons
were unchanged. Backup and deployment evidence: target/hover-delay/previous-installed.exe
and target/hover-delay/deployment.json. No physical cursor or user files were touched.
For handoff, start at CONTEXT.md; installed runtime source is fae06ba. Owner's
physical hover/visual assessment remains an unperformed manual check.

## Owner interaction follow-up (2026-10-17)

Execute D17-D20 in order, one task per commit. No new dependencies. Keep user files
as references, idle timers absent, and RefCell borrows outside pumping Win32 calls.
The owner paused the one-ring-layer idea; do not change ring capacity/layout here.

### D17: Immediate opening

- [x] Open on first pointer movement over a circle, remove hover dwell and opening
      reveal, preserve drag/click and the 110 ms closing animation.
- [x] Update native regression stimuli and behavior spec; run required checks and
      relevant isolated Windows fixtures, then commit/push.

D17 evidence: fmt/clippy and 47 release tests pass (one ignored preview); locked
release build passes. Open-circle fixture asserts ring creation before the first
WM_MOUSEMOVE returns; centre drag/page/persistence and circles missing-reference
fixtures pass. No physical input was generated. Installed runtime remains D16
until the complete follow-up release is deployed.

### D18: Desktop ordering

- [x] Replace Win+D topmost promotion with normal-band desktop ordering so restored
      applications cover circles immediately; preserve tray and capture recovery.
- [x] Add native z-order regression, run required checks and commit/push. Physical
      Win+D/taskbar interaction remains for the owner, without automated input here.

D18 evidence: fmt/clippy and 48 release tests pass (one ignored preview). The native
test raises a circle for a desktop fixture, restores an app before delivering its
foreground hook, and verifies the app is already above the circle. Capture cleanup
also removes legacy topmost state. Queued stale foreground notifications are ignored.
Actual Win+D animation on the owner's Explorer remains a manual check.

### D19: Visible item drag

- [x] Show a small icon preview following the captured pointer, using the existing
      loaded shell icon with a fallback; click-through, no activation or idle work.
- [x] Clean up preview on drop, cancel, capture loss and ring destruction; verify
      native drawing/lifecycle and existing drop behavior, run checks, commit/push.

D19 evidence: fmt/clippy and 49 release tests pass (one ignored preview). Native
preview test verifies no activation/topmost, click-through styles, pointer offset,
movement and idempotent cleanup. Existing drop-target model tests pass. Explicit
ignored visual preview was rendered and inspected (TEMP/desktuk-preview-drag.bmp).
Loaded shell icon is drawn once onto a 210/255-opacity disc; native fallback is
visible before icons load. Position comes from queued event coordinates. No shell
query, timer, new dependency or persistent surface; only one transient window.
Actual physical cross-circle dragging remains an owner check.

### D20: Work-area grid snap

- [x] Add checked right-click grid submenu: Off / 2x2 / 3x3 / 4x4. Owner confirmed
      snap to cell centres. Persist selection, default Off; positions stay unchanged
      until a circle is dragged and released.
- [x] Snap/clamp the whole circle within the nearest monitor's actual work area
      (including a right/left/top/bottom taskbar); support negative coordinates/DPI.
- [x] Test geometry and config compatibility, run required checks/fixtures and build,
      then commit/push.
- [x] Install/restart checked release and record observed evidence.

D20 evidence: fmt/clippy, 52 release tests (one ignored preview), release build and
isolated circles/open-centre fixtures pass. The latter also ran with grid=3:
Recent and Music snapped to distinct work-area cell centres; saved positions and
all file-reference hashes survived restart. CI includes the grid fixture. Unit
tests cover all grid sizes, four taskbar edges, negative monitor origins, scaled
circles, clamping, legacy config/default Off and malformed grid values. No physical
cursor movement or Desktop writes. Physical menus/cross-circle drag/Win+D remain
unperformed checks. Ready target/release/DeskTuk.exe SHA256:
B984059EB9EDF1C197F141763F865DC0DB8974C4D7ED2E73CE805AF8ACE0F8BB.

D20 deployment: normal WM_CLOSE shutdown, previous executable backed up under
target/interaction-followup/previous-installed.exe. While the normal app was absent,
isolated tray/second-launch/TaskbarCreated fixtures passed for circles and panel,
including normal-band reveal and preserved references/preferences. Installed the
checked release at D:/CodeSpace/desktuk/DeskTuk.exe, same SHA256 as above, runtime
source d1e1174. Restarted process 8140 responds and initialized its tray. Config hash,
Run/DeskTuk and Explorer HideIcons were unchanged. Evidence is local at
target/interaction-followup/deployment.json. No physical input or Desktop writes.
Owner should assess actual Win+D transitions, drag feel/preview and right-click grid.

## Owner follow-up / compaction handoff (2026-10-17)

Installed runtime remains d1e1174, deployment record 03e787b; both Windows CI runs
passed. The owner reported these usability issues. D21/D22 are implemented/tested.
Continue with the direct-main/no-PR
workflow and required precommit MSVC checks. No new dependencies are authorized.

### D21: Visible grid during circle dragging

- [x] With grid enabled, display work-area cell boundaries while a circle is held /
      dragged and highlight its actual snap target. Owner needs to see where it
      will land. Preserve selected 2x2/3x3/4x4 centre snap, taskbar exclusion and DPI.
- [x] Remove overlay on release/cancel/capture loss/destruction; overlay must not
      steal activation, intercept pointer/drop targets, or leave idle timers/buffers.
- [x] Verify target highlight matches final position, cleanup and work-area routing;
      run required checks and commit/push.
- [x] Deploy together with D22 and record checked evidence.

### D22: Always-on-top setting and Show Desktop behavior

Observed: after the D22 installation, the owner verified circles still disappear
in both settings; opening another application makes them visible again. Win+D and
the bottom-right Windows Show Desktop button both trigger the symptom.
Distinguish hover Desktop Peek from clicking that button and Win+D toggling; native
ordering fixtures and green CI are not proof of these physical shell interactions.

- [x] Add persistent, checked **Her zaman ustte / Always on top** option, enabled
      by default per the latest owner instruction, with an opt-out desktop mode.
      This supersedes D18's blanket never-topmost product policy. Keep focus unchanged.
- [x] Investigate/correct visibility paths for Win+D and bottom-right Show Desktop;
      include open ring, tray reveal, drag capture cleanup and both topmost settings.
      Keep z-order/shell integration in the host layer; do not modify user files.
- [x] Update spec/context and meaningful native regressions; run required checks,
      commit/push and clearly distinguish verified behavior from physical
      Peek/click/Win+D checks requiring the owner's assessment. Do not run local
      windows-integration-test.ps1 or generate physical mouse/keyboard input.
- [x] Deploy D21/D22, run tray fixtures while installed app is stopped, record evidence.
- [x] Deploy owner-report visibility recovery; confirm the running executable and
      leave Win+D / corner-click / Peek physical assessment clearly pending.

D21 evidence: shared cell geometry drives highlight and final snap. Event-driven,
click-through layered guide sits behind the dragged circle; target fill, boundaries
and landing cross are painted only on cell/work-area changes. Native pixel/lifecycle
tests and isolated grid=3 routed-message fixture cover immediate visibility on press,
work-area bounds, release, cancel, capture loss and Escape, saved snaps and restart.
No physical input, user file changes, dependencies or idle work. Required MSVC checks
pass: fmt/clippy and 54 tests (one ignored preview). Deployment follows D22.

D22 evidence: default topmost circles preserve their band on shell ordering,
foreground changes, reveal and capture cleanup; SC_MINIMIZE is rejected in that
mode. Turning the checked circle-menu setting off persists always_on_top=0 and
restores the existing desktop-only policy. Default omitted to preserve legacy config
bytes; references/positions unchanged. Panel stays desktop-only. Host-layer
DWMWA_EXCLUDED_FROM_PEEK prevents Peek fading on circles/rings/drag surfaces, a
separate path from Show Desktop's ordering. DWM accepts the attribute in native tests.
Required MSVC fmt/clippy and 56 tests pass (one ignored preview); isolated runtime
fixtures pass with topmost + grid=3, desktop-only + grid=3, grid Off, and missing
references. They verify owned-ring bands, shell minimize/demotion requests, guide
cleanup, capture cleanup and saved snaps across restart without physical input.
These are not evidence of actual owner-machine Win+D / corner click / Peek, which
remain owner checks.

D21/D22 deployment: source acdc061, normal WM_CLOSE shutdown; prior executable
backed up at target/visible-grid-topmost/previous-installed.exe. Isolated tray tests
passed for default topmost and explicit desktop-only circles, plus unchanged panel
behavior: mouse/keyboard reveal, second launch, TaskbarCreated and graceful cleanup.
Installed/restarted D:/CodeSpace/desktuk/DeskTuk.exe; PID 25656 responds and its tray
initialized. Read-only native inspection: all three circles visible/topmost, none
minimized or DWM-cloaked. Config hash, Run/DeskTuk and Explorer HideIcons unchanged.
SHA256: 1317946E66997526F0ACD4AA80BC426D33F9BCC37C84E20464FA64D17C79B15B.
Local deployment/windows snapshots: target/visible-grid-topmost/{deployment,windows}.json.
No physical input or Desktop writes. Windows CI pending at deployment time.

D22 owner follow-up: acdc061 did not prevent Shell hide requests in either mode.
Added a native recovery path: topmost hosts reject hide positioning and foreground
changes restore a hidden circle without activation; desktop-only hosts restore
visibility on desktop/app foreground changes before applying their existing band.
The actual Windows 11 Show Desktop path still needs the owner's rerun after this fix.

D22 recovery deployment: source 86f510c; previous executable from acdc061 backed up
at target/show-desktop-recovery/previous-installed.exe. Synthetic ShowWindow(SW_HIDE)
is blocked in the topmost runtime fixture; desktop-only foreground recovery passes
native host tests. Both grid=3 drag fixtures and tray suites pass in both modes.
Installed/restarted DeskTuk.exe, PID 25216, responding. Owner config, startup and
HideIcons preferences match their pre-install values; saved always_on_top=0 selects
desktop-only mode and was intentionally preserved. Native inspection confirms all
three circles visible, normal-band, not minimized and not DWM-cloaked. SHA256:
434222962D484E653CEB793B2B20FB8D24B4F053DDFF95227381C61CB604284A. Deployment
evidence/snapshots: target/show-desktop-recovery/{deployment,windows}.json. The owner
must rerun physical Win+D and the taskbar corner button. Windows CI 39032509588 passed
the deployed runtime, including release checks, all fixture modes and runner integration.
Z-order native fixture tests are serialized to avoid their shared Windows desktop; the
updated test harness is pushed and its Windows CI run is pending.
