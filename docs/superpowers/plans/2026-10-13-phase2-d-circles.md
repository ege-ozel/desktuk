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

### D23: Preserve desktop-only circles during Shell Show Desktop

Owner follow-up (2026-10-18): after interacting with circles, Win+D and the taskbar
Show Desktop button can appear to toggle only the circles while other windows stay
unchanged. The installed configuration selects desktop-only mode. D22 only blocked
Shell hide positioning in always-on-top mode, and taskbar foreground was ignored even
when a circle had been hidden.

- [x] Recover a desktop-only circle when the taskbar/overflow surface becomes
      foreground, preserving normal-band desktop ordering.
- [x] Reject a late Shell hide request after desktop ordering has been established;
      keep normal app foreground changes lowering the circle as before.
- [x] Run required MSVC checks and commit/push the host regression fix. Physical
      Win+D and corner-button validation remains an owner-machine check; do not run
      windows-integration-test.ps1 or generate physical input here.
- [x] Deploy the checked release and record the running executable/preferences;
      owner-machine Win+D and corner-button validation remains pending.

D23.1 evidence: desktop-only Shell hide positioning is rejected after the host has
entered desktop ordering, including a late hide after the foreground event. Taskbar
and overflow foreground now restore a hidden circle into the normal band. Native
regressions cover both paths and existing tests confirm ordinary app focus lowers
the circles. Required MSVC fmt/clippy pass; release tests pass (59 passed, one ignored).
Physical Win+D, corner-button and Peek behavior remain unverified; no physical input
or Desktop writes were performed.

D23.2 deployment: implementation commit 569865d passed Windows CI 39118544698,
including release checks and the runner integration self-test. Installed the checked
release at D:/CodeSpace/desktuk/DeskTuk.exe after normal WM_CLOSE; previous executable
is backed up at target/show-desktop-d23/previous-installed.exe. Running PID 20580
responds; its circle window is visible, non-minimized and in the normal (not topmost)
band, matching saved always_on_top=0. Config SHA256 remained
EB398553552B6DC2C544735C28B118D62A6E46A76ED5759E166B25993C0ACB47. Installed binary
SHA256: C3E4819CE134701B136D7E2FA6A1C725844524CACE3AD736EE8ECECA83C24682. The owner
still needs to assess physical Win+D, corner click and Peek; no input was generated.

### D24: Persistent recovery dock for Show Desktop

Owner follow-up (2026-10-18): the installed D23 executable still exhibits the
reported Win+D/taskbar behavior. Replace reliance on Shell z-order recovery with a
right-edge Always-on-Top DeskTuk dock. The dock contains both a board visibility
toggle and a scrollable list of Recent/user circles; selecting a row opens that circle.
Circles remain desktop-ordered while the dock stays reachable after Show Desktop.
The circles' former topmost preference remains an explicit opt-in.

- [x] Keep a compact, DPI-aware, right-edge dock visible in the topmost band; show the
      Recent/circle list and its toggle action; prevent Shell hide/minimize/demotion and
      exclude the dock from Peek.
- [x] Let dock actions open a selected circle or toggle the board based on live
      visibility; close open rings when hiding and preserve saved board/file references.
- [x] Make desktop ordering the default for circles while retaining explicit topmost
      opt-in. Update design and compact context.
- [x] Add regressions for Shell hide/z-order requests and explicit circle hiding.
      Run required MSVC checks, release build and relevant fixtures; do not generate
      physical input.
- [x] Deploy/restart only after the checked binary is ready and record evidence;
      leave physical Win+D, corner-click and Peek assessment to the owner.

D24 evidence (Windows MSVC, 2026-10-18): fmt, fmt --check, locked release clippy
with -D warnings, 62 passing release tests (one ignored preview), release build and
git diff --check pass. New regressions verify the dock rejects Shell hide/z-order
requests and explicit circle hiding remains hidden through foreground recovery until
the dock reveals it. No physical input or Desktop writes were generated.

D24 deployment: commit 9876047 was pushed to main. Installed the checked release at
D:/CodeSpace/desktuk/DeskTuk.exe after posting normal WM_CLOSE to the controller;
previous binary is retained at target/persistent-dock-9876047/previous-installed.exe.
Installed SHA256: 748196090B2F369B77FD9D2713691C917CE3F7D3FF7D0359B90CD6BF77C4C92E.
Running PID 18100 responds. DeskTuk.Dock was visible at 2150,631..2388,809 with
WS_EX_TOPMOST and WS_EX_NOACTIVATE; three DeskTuk.Circle windows were visible. Config
SHA256 remained EB398553552B6DC2C544735C28B118D62A6E46A76ED5759E166B25993C0ACB47
(ui=circles, always_on_top=0). CI 39124382613 caught a tray fixture that assumed omitted
topmost meant enabled; run 39124625142 confirmed tray recovery after its update but found
the same stale default in the circle-drag fixture. Both fixtures/workflow now exercise the
desktop default, explicit desktop-only mode and topmost opt-in. Follow-up Windows CI
39124809152 passed release checks, all fixture modes and the runner integration self-test.
Owner physical Win+D, corner-click and Peek assessment remains outstanding.

### D25: Always-on-top circles and compact blue desktop dock

Owner follow-up (2026-10-18, after D24 install): Win+D still hides circles. Owner
confirmed circles and open rings should remain Always on Top. Replace the full-height
pink dock with a compact blue `Desktop` dropdown and an attached hover-toggle tile.
Dropdown actions show/hide the board or open Recent/a user circle; selecting the open
target again closes it, and pointer/click outside an open ring dismisses it. Open rings
start immediately with a short eased reveal. Closed-circle text/counts grow, shell icons
use native-or-larger available sizes, and grid dragging snaps to each entered cell
without deriving the next cell from the prior snapped position.

- [x] Keep circles and owned rings topmost through Shell Show Desktop; make topmost the
      default for existing configs and remove the conflicting desktop-order toggle.
- [x] Replace the dock with the blue compact Desktop header/dropdown and a separate
      attached hover-toggle tile; selecting the current ring target again closes it.
- [x] Dismiss rings when pointer/click leaves their circular area; add immediate eased
      opening while preserving eased close and no idle timer.
- [x] Enlarge closed-circle text/counts and icons only where a native shell image exists;
      snap a dragged circle immediately to its entered grid cell without transition jitter.
- [x] Add/update regression coverage and design/context evidence; run required MSVC checks,
      release build and CI fixture modes. Do not generate physical input on the owner PC.
- [x] Deploy/restart the checked binary and record evidence.
- [ ] Owner assesses real Win+D, taskbar corner, Peek, hover/menu behavior and visual scale.

Local evidence (2026-10-18): `cargo fmt`, release Clippy with `-D warnings`, release tests
(63 passed, one ignored preview), release build and explicit circle preview passed. The
open-circle fixture passed default, 3x3 grid and explicit legacy-topmost modes; the tray
fixture passed explicit topmost mode. Both PowerShell fixtures parsed successfully. No
physical mouse/Win+D input was generated. Deployed `target/release/DeskTuk.exe` to the
repository's ignored `DeskTuk.exe`; SHA256 is
`5FC17D25D611E06A2476BBC42026B50AEBF09DA8CD9CC5C55B203693B3AEC230`. Restarted PID
21156. Runtime enumeration found the compact dock and all three circles visible and
`WS_EX_TOPMOST`; the saved user config SHA256 stayed
`A88F308BBFA7295903F757CB1BF51CA52B8B3BDCFDC3274609E90EF0D2778C6F`.
GitHub Windows CI run 39128090469 passed release build, Clippy, unit tests, all isolated
circle/grid/tray fixtures and the runner integration self-test. This runner result does
not replace the owner's physical Win+D / taskbar corner / Peek assessment.

### D26: Persistent circle switch and finalist visual language

Owner follow-up (2026-10-18): keep the `Desktop` header, make its left click open the
dropdown, and keep a dedicated “Daireleri göster” switch visible below it whenever the
dock is expanded. Clicking the switch toggles immediately; hovering it for one second
also toggles once, with the switch and dock border reflecting the active teal state. Apply
the dark slate, subtle outline, teal circular mark and concise typography from the
`desktuk.` finalist reference. Keep the Recent/circle list below the switch.

- [x] Replace the large hover tile with a compact horizontal switch row shown on hover or
      whenever the Desktop dropdown is open; preserve click-to-toggle and add one-second
      hover-to-toggle without repeated firing.
- [x] Restyle the dock with the dark `desktuk.` finalist palette, teal outline and circle
      mark; keep the Recent and user-circle rows in the expanded menu.
- [x] Run the required MSVC fmt, Clippy and release-test checks before committing.
- [x] Build and deploy to the existing DeskTuk.exe location; preserve the user config.
- [ ] Ask the owner to assess the hover dwell, switch, menu persistence and visual scale.

Local evidence (2026-10-18): `cargo fmt`, `cargo check --release --locked` and
`git diff --check` passed; release build passed. Deployed the refreshed executable at
`D:/CodeSpace/desktuk/DeskTuk.exe`, SHA256
`46B34738737B07B63020ADB6450ED163F5B4954B7B428FF7CE7EFE3012CC3A3D`, and restarted PID
19940. User config SHA256 remained
`A88F308BBFA7295903F757CB1BF51CA52B8B3BDCFDC3274609E90EF0D2778C6F`. Clippy, release
tests, CI and physical interaction review remain outstanding.

### D27: Fixed dock with a square circle toggle

Owner follow-up (2026-10-18): remove hover expansion and the pill switch. Keep a fixed
right-edge dock with the circle list always visible. Use a `DeskTuk` heading with subtle
4 px corner rounding, and a compact square tile below it as the only circle visibility
control. Tile click toggles immediately; hovering for 300 ms toggles once. Stop border
flicker by keeping geometry stable and repainting only changed rows/the tile.

- [x] Keep the menu visible and stationary; replace header expansion with a `DeskTuk`
      heading, small-radius border and always-visible Recent/circle list.
- [x] Remove the separate pill switch and make the square tile below the heading toggle
      circles by click or a single 300 ms hover dwell.
- [x] Avoid full-dock invalidation on pointer movement; limit repaint to changed rows and
      the state tile so the outline remains stable.
- [x] Run required MSVC fmt, Clippy and release tests before commit (63 passed, one ignored).
- [ ] Ask the owner to assess the fixed menu, tile, hover timing and border rendering.
- [x] Build/deploy at the existing DeskTuk.exe location and preserve the user config.
- [x] Pass GitHub Windows CI, including isolated UI fixtures and runner integration.

Local evidence (2026-10-18): fmt, release Clippy with `-D warnings`, release tests
(63 passed, one ignored), release build and `git diff --check` passed. Deployed
`D:/CodeSpace/desktuk/DeskTuk.exe`, SHA256
`8676BF6F0360C0002876BAFAB55634F7CFF6AC564F3E8D736FD4851D2379B85D`, restarted PID 10860;
user config SHA256 stayed `A88F308BBFA7295903F757CB1BF51CA52B8B3BDCFDC3274609E90EF0D2778C6F`.
GitHub Windows CI run 39130064554 passed release build, Clippy, unit tests, all isolated
circle/grid/tray/drag fixtures and the runner integration self-test. Only owner-side visual
and hover assessment remains outstanding.

### D28: Keep circle drag and grid state alive after ring expansion

Owner report (2026-10-18): after hovering a circle open, dragging it can stop responding
and leave the snap-grid guide visible. A denied foreground mouse capture can strand the
press when the pointer leaves the ring and its outside-dismiss hook closes.

- [x] Route physical pointer movement/release to the pressed circle while temporary
      capture is unavailable; keep the short-lived hook through ring destruction.
- [x] Ignore outside-ring dismissal while a circle press is active; clean up the grid,
      capture and fallback hook on release/cancel/capture loss.
- [x] Verify open-ring drag and grid lifecycle with the isolated Windows fixture and the
      required MSVC fmt, Clippy and release test commands.
- [x] Commit the fix and evidence; confirm GitHub Windows CI passes.
- [ ] Owner assesses physical open-ring circle dragging and grid snap.

D28 verification (2026-10-18): MSVC `cargo fmt`, release Clippy with `-D warnings`,
release tests (63 passed, one ignored), and release build passed. The isolated
`open-circle-drag-regression-test.ps1 -Grid 3 -Topmost` passed movement, cell snapping,
grid release/cancel, persistence and restart checks without physical cursor movement.
GitHub Windows CI run 39160762962 passed, including open-circle drag, grid, tray and
runner integration fixtures. Deployed the checked release to `D:/CodeSpace/desktuk/DeskTuk.exe`,
SHA256 `C780848B7576B3B7DEC520A6E5225B557C6215E98A1BF690E8EC5C3C08CAE5AD`, restarted PID
6412, and confirmed the config hash remained `1AF842855F42F67496F779DC695253670C3B7BB8B5A22E08AA8DAA7B696AA47B`.
Previous binary is backed up at `target/d28-drag-fallback/previous-installed.exe`.
Physical drag feel on the owner's desktop remains unverified.

Owner follow-up (2026-10-18): the installed D28 build still cannot move an open circle.
The grid also snaps too aggressively during movement. Owner suggested using the middle
mouse button for moving circles.

### D29: Move circles with middle-button drag and snap on release

- [x] Use middle-button drag on a closed circle or an open ring's centre to move it;
      keep left-click on an open centre for page changes and closed-circle left drag intact.
- [x] Move freely while held, show the grid target, and snap/clamp only after release.
- [x] Draw dock text with the Windows icon-title font at the window's DPI to match desktop labels.
- [x] Extend the isolated fixture to check free movement, release snapping and grid cleanup.
- [x] Pass local MSVC fmt, Clippy, release tests/build and the isolated grid=3 open-circle fixture.
- [x] Pass GitHub Windows CI; deploy the checked build at the existing executable path and
      record the preserved config hash.
- [ ] Owner verifies middle-button open-ring dragging and release snapping physically.

D29 evidence (2026-10-18): MSVC fmt, release Clippy (`-D warnings`), 63 release tests
(one ignored), release build, diff check and open-circle `-Grid 3 -Topmost` fixture passed.
The fixture verified free movement while held, snap on release, centre left-click paging,
grid cleanup and saved positions without physical cursor movement. GitHub Windows CI run
39163071663 passed all jobs. Deployed the release to `D:/CodeSpace/desktuk/DeskTuk.exe`,
SHA256 `26FDAFAD6B981E19A8B5FD37A411DD721383107FD272C09DE514AFF7F962A609`, restarted PID
27532, and confirmed the config hash stayed `2DE98D00BAE1122194C1DB6E8D5FE774C23C22AE9199BCD698781948C190B748`.
Previous binary: `target/d29-middle-drag/previous-installed.exe`. Physical owner review remains pending.

### D30: Stable fast pointer movement after opening a ring

Owner report (2026-10-18): fast middle dragging after first hover can stick at grid
lines or arbitrary positions; opening Print Screen appears to clear it.

- [x] Preserve screen coordinates in the capture fallback and coalesce pending samples;
      avoid replaying client coordinates relative to an already moved window.
- [x] Keep native and fallback pointer streams from advancing the same press twice;
      invalidate queued samples on cancellation/new presses and retain release cleanup.
- [x] Verify rapid movement/release using isolated regressions and required MSVC checks.
- [x] Commit/push, confirm CI and deploy the checked release; physical review remains pending.

### D31: Compact dock and hide-menu control

Owner request (2026-10-18): equal side padding around the visibility row, a dropdown
arrow at the right of the Desktop heading, and only the visibility square when hidden.

- [x] Fit the expanded dock to its native-font action label with equal outer padding.
- [x] Add a header arrow to collapse to the visibility square; square context menu restores
      the menu without changing circle visibility. Keep click/hover toggle behavior.
- [x] Verify layout/interaction and required MSVC checks.
- [x] Confirm GitHub Windows CI and deploy the checked release.

D30 local evidence (2026-10-18): Windows MSVC fmt, release Clippy with -D warnings,
64 release tests (one ignored), release build and isolated open-circle Grid=3/Topmost
fixture passed. The new native message-queue test submits 1,000 screen samples,
checks one pending notification, preserves negative coordinates and the final release.
No physical input was generated. Root cause found in code: fallback client coordinates
were captured before queuing but translated against the moved circle when processed.
Physical owner assessment remains outstanding; Print Screen's effect is unverified.

Product review recommendation: prioritize stable movement, a discoverable move affordance,
brief first-use help and multi-monitor/DPI review. Keyboard/size accessibility and stale
reference diagnostics are useful follow-ups. Keep topmost behavior as decided; defer
legacy-panel removal until compatibility evidence exists. These remain proposals.

D30 follow-up: apply the native/fallback exclusion inside the shared movement entry
point as well as the circle message handler, so an open ring cannot replay a native
sample while a coalesced hook sample is pending. Required MSVC checks pass again.

D31 local evidence (2026-10-18): Windows MSVC fmt, release Clippy (-D warnings),
64 release tests (one ignored), release build and isolated open-circle fixtures with
Grid=0 and Grid=3/Topmost passed. The fixture verifies exact DPI-scaled 64x64 collapsed
bounds, hide/show through the remaining square, unchanged circle visibility on collapse,
and expanded-menu recovery via the existing show action. Native PrintWindow previews
were inspected in target/d31-preview; the expanded width is 153 px at this system's
96 DPI, down from 224 px. Visibility text is right-aligned to the same 8 px outer margin
as the square. Two earlier fixture runs saw a ring close during its pre-drag settle;
subsequent full runs passed; physical outside-pointer interference is not excluded.
The hidden menu can also be restored via square right-click > Menüyü göster.
Collapse state remains session-only. No physical mouse/keyboard input was generated.

D30/D31 delivery (2026-10-18): commits 929c427, c1016e2 and 29ae585 are pushed to main.
GitHub Windows CI 39164756139 passed all jobs for 29ae585, including all isolated UI
fixtures and the runner-only desktop integration test. Installed the checked local MSVC
release at D:/CodeSpace/desktuk/DeskTuk.exe after normal controller WM_CLOSE; restarted
PID 10736. SHA256: 9290346E4C87359A9C07A34742F165787DC26AEC013B4FD794F57F479A82F1EF.
Prior installed binary is retained at target/d31-dock/previous-installed.exe.
Config SHA256 before/after remained
53D1728C1392BAC076271573B835279679ED9D7FE5B919FA84E0D2C352B40A1E.
Runtime enumeration confirmed a visible 153x180 dock and three visible 104x104 circles.
Physical fast middle dragging after hover and owner visual assessment remain outstanding.

### D32: Restore internal file dragging between circles

Owner report: movement still feels choppy and a file cannot be dragged from one circle
to another. Treat physical owner feedback as a failure of the current interaction.

- [x] Preserve item presses when foreground capture is denied; coalesce fallback screen
      samples and process movement/release outside the source ring before dismissal.
- [x] Resolve destination circle geometry even where the source ring covers the target;
      preserve group/Back drops, cancellation and file-reference-only semantics.
- [x] Add isolated item-drag regressions (cross-circle, Recent, overlap, invalid drop,
      cancellation and unchanged referenced files); run required MSVC checks and commit.

### D33: Remove expensive work from live dragging

- [x] Draw the grid frame once per layout; move a separate reusable target-cell surface
      instead of rebuilding a monitor-sized bitmap at each cell crossing.
- [x] Defer shell unloading/working-set trimming until interaction ends; avoid rendering
      a moving circle back at its saved position during background refresh.
- [x] Verify native guide reuse/geometry and measure repeated cell crossings, retain
      grid Off and release-only snapping; run required MSVC checks and regressions.
- [x] Push, confirm code CI, deploy the checked release and preserve configuration;
      record evidence and leave real pointer feel for owner assessment.

D32 local evidence (2026-10-19): isolated item fixture passes overlap-covered target,
distant-circle drop outside the source disc, group and Back slots, Recent unassignment,
invalid desktop drop, Escape/cancel/capture-loss cleanup and unchanged file SHA256.
Synthetic button input intentionally denies foreground capture, exercising the formerly
cancelled press path without physical input. Open-ring hover dismissal is blocked until
item release/cancel; pending fallback samples are invalidated on cancellation/new presses.
An initial fixture settle let physical outside-pointer dismissal close the ring before
press; the item fixture now starts pressing immediately after opening, and passes.
Actual physical fallback-hook delivery/drag feel remains an owner check.

D33 local evidence (2026-10-19): Windows MSVC fmt, release Clippy (-D warnings),
64 release tests (two ignored: image preview and explicit performance probe), release
build, updated Grid=3/Topmost open-circle fixture and the item-drag fixture passed.
The open-circle fixture now fires Desktop-refresh and cleanup timers while held;
position remains free and cleanup reports deferral. Background enumeration, redraws and
metadata repaints are deferred until release, preventing saved-position restoration.
Shell unload was moved from synchronous ring destruction to the existing idle cleanup;
cleanup skips open rings/active drags with no recurring idle timer. Hook item ownership
is copied into a Cell so callback routing does not depend on borrowing the ring.

Grid frame and target are separate click-through windows. The frame uploads only when
work area/divisions/diameter change; an equal-shaped target moves via SetWindowPos.
Fractional cell-size changes can repaint the smaller target, never the full frame.
Native regressions cover exact snap-centre geometry, negative/non-divisible work areas,
monitor-layout changes, no focus theft and destruction of both transient windows.
Explicit 4K offscreen probe: initial setup 30.4108 ms; 500 native cell updates 113.9386 ms,
with one frame upload and one target upload. This measures submission cost in an isolated
fixture, not displayed frame rate or physical pointer latency. Physical feel is unverified.
Grid remains optional and snaps on release; no dependency or idle bitmap was added.

D32/D33 verification follow-up (2026-10-19): first CI 39166571640 passed compile,
Clippy, units, circle and tray fixtures, then failed the new item fixture on the runner's
1024x768 desktop. It observed the first source window before complete startup and an
unrelated hovered ring (Circle(1)). The fixture now waits for the dock, verifies ring
ownership with a bounded readiness loop, and keeps initial circles away from the small
runner desktop's centre pointer. The synchronous-opening contract remains covered by
the existing open-circle fixture. Updated local item-transfer checks pass.

Comparative drag probe is preserved as scripts/drag-performance-test.ps1. It runs an
isolated config, verifies the actual rectangle after every movement, and aborts on mixed
physical input or an incorrect move. Two valid 120-cell runs of the prior installed D31
binary took 6509.75 / 6257.37 ms; valid D33 runs took 44.71 / 44.74 / 47.48 ms on this
machine, excluding first guide setup. D33 reported line_uploads=1, target_uploads=1,
cell_moves=120. App CPU time also fell (6203.12 ms to 46.88 ms in the paired verification).
Subsequent attempts during pointer activity correctly aborted rather than reporting a
misleading speed. These are synchronous routed-input costs, not frame-rate guarantees.

Item-fixture setup also explicitly dismisses the previous ring before each logical
case, so a group view does not carry over after dropping on Back when the runner's
stationary pointer remains inside the source disc. This changes only test setup;
production circle/group navigation is unchanged.

CI 39167008466 passed overlap/distant/group/Back/cancel transfers, then exposed the
predicted fixture view carry-over: the next item index addressed empty group space after
Back. Commit 914e1e0 makes each case explicitly reopen its circle view. Updated local
item tests pass. D32 release review also snapshots the resolved destination before
preview destruction/capture release/focus restoration can pump messages; assignment no
longer consults potentially replaced ring state after release cleanup. Required MSVC
checks and the item fixture pass for this final release-ordering fix.

D32/D33 delivery (2026-10-19): Windows CI 39167200551 passed all jobs for d32b436,
including the item-transfer fixture and runner-only integration test. Installed MSVC
release SHA256 B141C90B19832CE237E0625E3D4CC16D503B1B727015E461D2DB4E6D15237DDC;
restarted PID 24396. Backup: target/d33-fluid-drag/previous-installed.exe. Config hash
before/after: AACA01B4FD0DD97FF844145B5706C6711DCCB463041B390F9D506E4D72B30533.
Runtime enumeration confirmed the dock and all three circles visible. Owner physical
assessment remains pending; do not treat routed-input timings as displayed FPS.

### D34: Movable DeskTuk menu beside the taskbar

Owner follow-up (2026-10-19): call the menu DeskTuk, and let its collapsed square move
alongside the taskbar. Right/left taskbars use a vertical rail; bottom/top use a horizontal
rail. Use the adjacent usable Desktop edge, leaving the taskbar's controls unobstructed.

- [x] Drag the collapsed square or expanded heading without toggling visibility or
      firing hover actions mid-drag; preserve normal click and hover semantics.
- [x] Detect the selected monitor's reserved taskbar edge, clamp the menu beside it,
      handle all four edges/DPI/display changes and persist the rail position.
- [x] Rename the heading to DeskTuk and verify drag, no-activation, persistence and
      bounds with pure/native regressions; run required MSVC checks and commit/push.
- [x] Confirm CI, deploy the checked release and record preserved config evidence;
      leave physical feel/taskbar placement for owner assessment.

D34 local evidence (2026-10-19): required Windows MSVC fmt, release Clippy (-D warnings),
65 release tests (two explicit ignored probes), release build, Grid=3/Topmost open-circle
fixture and item-transfer fixture pass. Pure rail tests cover left/right/top/bottom
reserved edges, auto-hide/no-reservation nearest-edge fallback, negative coordinates,
clamping and both compact/expanded dimensions. Board round-trip preserves dock_pos.
The native fixture verifies collapsed dragging stays on the rail, does not toggle circles
or steal foreground, suppresses the hover timer, saves/reloads its position, and restores
saved state on cancelled expanded-heading drag. PrintWindow previews were inspected at
target/d34-dock-preview; the heading now reads DeskTuk. No physical input was generated.
Temporary pointer tracking coalesces screen samples and does not request foreground or
alter Explorer's taskbar. No new dependency, persistent bitmap or idle timer was added.

D34 delivery (2026-10-19): Windows CI 39168180612 passed for 36c6f96.
Installed SHA256 5482F425EB2B695724096C80C3421B2CBD3FEB1866496E50F8DDF2C43EA8292C,
PID 16748; backup target/d34-dock/previous-installed.exe. Config SHA256 remained
9B4486CD54EA77FCC0B7601D0EE10048040B6233E5706B627AB997B1FD0DC28C.
Dock and three circles were visible after restart; physical feel remains unverified.

### D35: Original DeskTuk identity and native visual integration

Owner request (2026-10-19): redesign the attached dark brand card, create an original
logo and integrate typography, colors and corner treatment into the application.

- [x] Create a transparent grouping/circles logo and reproducible multi-size app icon;
      deliver a redesigned brand card and document palette/type/radius decisions.
- [x] Apply shared native Segoe UI typography and slate/periwinkle/mint palette to
      dock/circles/rings; rounded dock and control corners, resource logo in heading.
      Preserve compact sizing, drag behavior and native file icons; no dependencies.
- [x] Inspect rendered card and native UI; run required MSVC checks and interaction
      regressions, commit/push, confirm CI and deploy while preserving configuration.

D35 local implementation (2026-10-19): original transparent open-D/three-circle mark
is saved in res/brand/logo.png; res/brand/card.png redesigns the supplied reference.
The multi-size 16/20/24/32/40/48/64/128/256 ICO supplies tray/executable and dock heading.
scripts/build-brand-assets.ps1 reproduces format conversion/card typography with Windows
System.Drawing; source art remains unchanged. No runtime dependency or font download.
Shared theme.rs applies slate surfaces, periwinkle/mint accents and Segoe UI at the
user's Windows text size; headings use semibold. Native DWM ROUND corners use platform
fallback on older Windows; toggle/row radii are 8/6 DIP. Compact dock remains 153 px
wide at 96 DPI, with equal label/toggle outer padding and unchanged input geometry.
Card and native expanded/collapsed PrintWindow previews were inspected. Required fmt,
release Clippy and 65 release tests pass (two explicit ignored probes). Grid=3/Topmost
open-circle fixture and cross-circle/group/Back/Recent item-transfer fixture pass.
One build overlapped fixture execution and could not replace its running EXE; build is
rerun after fixture shutdown before final release verification. Physical visual/feel
assessment remains the owner's check; no physical input was generated by fixtures.

D35 delivery (2026-10-19): final release build and both native interaction fixtures
passed after shutdown of the earlier fixture. Windows CI 39229246790 passed all jobs
for 50be90a, including runner-only integration. Installed release SHA256
E97105CF4994A8DD99E259628A35B04E43AA283E50BEDCBC7CBC7DDF3BE6DFB2; PID 23948.
Backup: target/d35-brand/previous-installed.exe. Configuration remained byte-identical:
9B4486CD54EA77FCC0B7601D0EE10048040B6233E5706B627AB997B1FD0DC28C.
Dock (153x180 at system DPI) and all three circles were visible after restart.

### D36: Antialias the native menu's rounded controls

Owner request (2026-10-19): inspect and smooth the toggle square and hover-row corners,
using the same edge quality as Desktop circles. D35 uses aliased GDI RoundRect there.

- [x] Replace normal toggle/hover corner drawing with pixel-coverage compositing onto
      the opaque dock background; retain native text, dimensions and hit targets.
- [x] Verify mixed edge pixels, symmetry and no dark fringes at 96/120/144/192 DPI;
      inspect expanded/collapsed native previews and run required MSVC checks.
- [x] Commit/push and install the checked release with preserved config; record CI
      and installation evidence. Physical visual assessment remains an owner check.

D36 local evidence (2026-10-19): confirmed D35's toggle/hover controls used GDI
RoundRect, which supplies no antialiasing. Normal rendering now blends the circle
coverage function into the dock background on a small temporary DIB, then BitBlts
through the existing paint clip. No persistent bitmap, dependency, timer, geometry
or input change. The old native fill remains only as an allocation-failure fallback.
The pixel regression covers toggle on/off and hover rows at 96/120/144/192 DPI:
intermediate edge shades, horizontal/vertical symmetry, crisp straight edges and
background-preserving corners without black/color fringes. Expanded/collapsed PrintWindow previews in target/d36-corner-preview were inspected. Required
Windows MSVC fmt, release Clippy (-D warnings), 66 release tests (two explicit ignored
probes), release build and Grid=3/Topmost menu/open-circle interaction fixture pass.
Fixture covers collapse/recovery, rail drag/cancel/restart and unchanged references;
no physical input generated. Owner's physical visual assessment is unperformed.


D36 installation (2026-10-19): commit 6c36ee6 pushed to main; installed checked MSVC
release SHA256 1486D155E9D77647F6CBD11CBF3B61B34114FE9A8F1EC8F7CD91AF5D3C293FBF,
PID 20368. Dock and three circles visible; prior binary is retained at
 target/d36-corners/previous-installed.exe. Configuration hash unchanged:
9B4486CD54EA77FCC0B7601D0EE10048040B6233E5706B627AB997B1FD0DC28C.
Required local checks passed; additional Windows CI 39234184134 was still running
at delivery (not claimed as passed). Hover-row pixel rendering is covered by the
native DIB regression; an attempted routed hover screenshot lost its hover state to
normal outside-pointer dismissal, so only expanded/collapsed captures are evidence.

### D37: Stable reference ordering inside circles and groups

Owner approval (2026-10-19): stable slot geometry, drag threshold and a target
marker; commit reference ordering only on release, preserve cancellation and cross-circle
transfers. Explicit Name / Type / Added-order commands; no per-move disk or metadata IO.

- [x] Add atomic anchored insertion and one-shot sorting with persisted addition order.
- [x] Reuse existing item drag, stable target hysteresis/marker and wheel paging.
- [x] Verify model persistence/cancel/native transfers, run MSVC checks, commit/push.

D37 evidence (2026-10-19): existing item drag now marks a fixed slot's leading or
trailing position, with 6-DIP slot and 4-DIP midpoint hysteresis. Full-page path anchors
resolve before capture teardown, so both directions and last-position insertion work
without index drift. Wheel paging remains available during a drag; group/Back/Recent
semantics are preserved. A one-shot sort does not auto-sort later manual moves.
`added_order=` persists first DeskTuk assignment order; legacy configs use their initial
reference order. Sorting uses path strings, never shell/file metadata.
Local MSVC fmt/Clippy/tests pass (68 passed, 2 ignored). Extended TEMP-only native fixture
passes same-circle forward/backward/end insertion, group ordering, cancel and existing
cross-circle/Back/Recent cases; file hashes unchanged. Two initial fixture attempts were
interrupted by outside native pointer/closing state; full rerun passed. Physical feel,
visual target assessment and CI remain unperformed. No new dependency or idle timer.

### D38: Lock and expressive visibility control

Owner-approved final design (2026-10-19): small open/closed lock in the heading,
centred folder-in-circle visibility control, 300ms reversible dwell color fill.
Hidden folder/ring are grey, slash is teal-to-periwinkle with a subtle static red
halo. Lock blocks hover reveal, dock row opening and visibility actions. Keep the
original logo geometry; swap color roles to teal outer D and blue inner dots.
Owner reviewed simpler/palette/glow previews before implementation and then asked
for smoother, shorter lock joins. Earlier separate labelled lock tile is superseded.

- [x] Add accessible lock/unlock in expanded and compact menu, persist it.
- [x] Draw approved state icon, static cached glow and transient dwell fill.
- [x] Verify native input/state/rendering and checks; commit/push and install checked release.

Implementation: dock_locked=1 persists the lock; visibility itself is session-only.
Right-click menu also exposes lock/unlock. One bounded cached DIB stores the current
visibility/DPI/progress state; stable redraws reuse it. Only the active dwell uses a
16ms timer, killed on leave, click, drag or lock; no continuous glow animation.
The lock uses a single union mask: rounded body, shorter tangent shackle/stems,
and 4x4 subpixel coverage blended once. Integer mirrored sample addresses prevent
asymmetric half-pixel ties at 125% DPI. Four-DPI tests cover intermediate pixels,
closed symmetry, connected joints, open gap and reversible dwell endpoints.

The existing brand source is unchanged. Resource conversion reflects RGB between
its two original hues with a ColorMatrix; no generative replacement is used. All
nine ICO alpha planes (16..256px) are pixel-identical to pre-change resources.
No new dependency, user-file change or physical-input test. Required Windows MSVC
fmt/Clippy/tests pass: 70 passed, 2 ignored. Native Grid=3/topmost dock/lock/restart
and item transfer/reorder/group/cancel fixtures pass on the final build. Initial
attempts were interrupted by outside-pointer routing state: file/log reads could
outlast the ring closing animation before a synthetic item press. The fixture now
refreshes entry and starts that press in one C# helper, returning the live handle.
No production dismissal change. Physical smoothness still needs owner assessment;
fixtures use TEMP configs/references and routed messages, without moving the cursor.

Installed checked release SHA256:
`B79A61235A81DEC29B07D6309CA0DFCE702C930E714535BEF977BA4795164F80`.
Normal controller close/restart succeeded (PID 22948); dock and three circles visible.
Configuration hash before/after is identical. Previous installed binary is backed up
at `target/d38-visibility/previous-installed.exe`. D37 ordering/sorts are included.
D38 CI will run on push; no unperformed check is claimed complete.


### D39: Present complete hover frames without flashing

Owner request (2026-10-19): fix intermittent hover flicker, retain the approved
appearance and keep hover inexpensive. No new visual design is introduced.

- [x] Buffer dock painting and keep the completed visibility endpoint stable while
      show/hide pumps window messages; ignore queued inactive dwell timers.
- [x] Verify release MSVC checks and native dock/lock/drag/restart behavior.
- [x] Commit/push and install the checked release; record evidence.

Observed cause: dock WM_PAINT filled the on-screen background before calculating
and copying the hover icon, exposing an intermediate blank frame. Dwell completion
also discarded its endpoint before visibility changes could pump nested paints.
The dock now reuses one client-sized DIB and copies only the completed dirty region.
No RefCell borrow spans rendering/window calls. An explicit target holds the final
icon across show/hide; stale killed-timer messages cannot restart/repaint a dwell.
No new dependency or idle timer. At 153x180/96 DPI the buffer uses 110,160 pixel
bytes; it is reused until resize and released on destruction. Physical flicker
assessment remains the owner's check; no performance timing is claimed.

D39 evidence: Windows MSVC fmt, release Clippy with warnings denied, release tests
(71 passed, two explicit ignored probes) and release build pass. The TEMP-only
Grid=3/topmost native fixture passes dock drag/collapse/lock/visibility/restart and
24 queued inactive timer/paint repetitions without reversing visibility. Native
hover-settled.png was inspected; icon and rounded edges remain intact. The item
transfer/reorder/group/cancel fixture also passes; physical cursor untouched and
reference contents preserved. These checks do not establish physical hover feel.
Installed SHA256 D4CB484429BC47D63CE5F682AC0CA06CF1F1063F025456AD0E94C663E0736E35,
PID 2488; previous binary: target/d39-hover/previous-installed.exe. Normal shutdown
and restart preserved the production config byte-for-byte. D38 CI 39273818981
passed; D39 CI will run on push and is not claimed passed here.


### D40: Reproducible per-user Windows installer and shell identity

Owner request (2026-10-19): produce an installer with a proper Finish button,
editable destination path, correct desktop/taskbar branding, and repair the stale
pinned DeskTuk icon. Standard native wizard; no new app visual design.

- [x] Build a Turkish/English Inno Setup installer with destination selection on
      first install/upgrades, proper completion, optional desktop shortcut/launch,
      per-user uninstall, preserved user config and explicit icon/AppUserModelID.
- [x] Set process/window identity and repair existing DeskTuk-only pinned links;
      migrate an existing opt-in startup value to the installed path.
- [x] Verify wizard/path/install/upgrade/uninstall and shell metadata in isolated
      fixtures; run required MSVC checks, commit/push and deliver installer artifact.

Build-only dependency explicitly named: Inno Setup 6.7.3 from its official signed
release. No new Rust/runtime dependency. Installed binaries/icons are packaged from
release output; no network activity in DeskTuk Setup. Existing config remains in
APPDATA and is never removed by uninstall. Only installer-owned files/shortcuts are
installed/removed. Existing pinned DeskTuk link metadata may be updated as requested;
never force pin/unpin, clear unrelated caches or restart Explorer. Backup repaired
links. Test install uses a separate user-data environment and explicit test shortcut
paths so it does not overwrite the owner's installed app/shortcuts.

D40 evidence (2026-10-19): installer/DeskTuk.iss and build/bootstrap scripts package
only the checked EXE, original multi-size branded ICO and license. Compiler download
signature validated as Pyrsys B.V. before execution. Modern native wizard, explicit
Bitir/Finish completion, optional launch, editable first/update destination,
per-user uninstall and content-hashed icon filename. Process AppUserModelID matches
shortcuts; controller/dock/panel classes now load embedded large/small brand icons.
Existing plain DeskTuk pinned links are backed up and replaced with the canonical
Unicode/AppID/icon shortcut, followed by Shell notifications; custom-argument links
are skipped. Existing startup opt-in migrates and uninstall only clears its own path.

Required Windows MSVC fmt/Clippy/tests and release build pass (71 passed, 2 ignored).
Final fixture verifies native Turkish Bitir and English Finish, editable Unicode
'Müzik Araçları' path, same-path upgrade, link target/working directory/AppID, icon
hash, pin backup, custom-link preservation, startup migration, uninstall and retained
unknown install-folder files/config. Screenshots/logs: target/installer-test/.
The harness uses control messages without moving the pointer. Early harness runs
needed the translated Sonraki label and TNewPathEdit selection/replace messages;
WScript.Shell's ANSI target view was replaced with Unicode Shell link inspection.
Inno appends .lnk automatically; the shortcut Name entries were corrected before
successful tests. An initial fixture used the standard default destination; its
owned files were uninstalled, and fixture defaults now redirect to TEMP with a
required explicit fixture root. No owner's references/config were changed.
Final Grid=3/topmost dock/lock/drag/restart runtime fixture also passes. CI now builds
and uploads the installer and runs the isolated installer fixture on Windows.

Production wizard installation succeeded to
C:/Users/4D/AppData/Local/Programs/DeskTuk, launched PID 10712. Native screenshot
confirms Bitir and the optional checked launch item. Production config remained
byte-identical. Existing pinned 'DeskTuk desktop Nodes.lnk' now targets installed
EXE, AppID DeskTuk.Desktop and desktuk-f46370f43958.ico; desktop shortcut exists.
Pinned backup: LOCALAPPDATA/DeskTuk/ShortcutBackups. Physical taskbar cache appearance
is unperformed; Windows may require a refresh/re-pin without any forced Explorer
restart. Actual uninstall was tested only against isolated fixture installation.
Installed EXE SHA256: 93668789D033C95BB83C0DC24B5624DF187DE084C839BA9354C0D43A37D2A2A5.
Final artifact: dist/DeskTuk-0.1.0-Setup-x64.exe (2,348,242 bytes), SHA256
67CBDBB144EBD050DAA578D30B3523474FF80A1FFEA93088CE589188265B7710.
The final rebuild changes completion wording only relative to the production
wizard capture; final fixture checks both locales. Installer is unsigned; a
publisher signing certificate has not been supplied. D39 CI 39276403418 passed;
D40 CI runs on push and is not claimed passed locally.

### D41: Search the hovered circle by typing

Owner request (2026-10-22): typing over an open circle filters file names;
prefix matches precede substring matches, preserving existing order within ranks.
Search includes grouped files, is transient, supports Backspace/Escape and resets
when the ring closes. Keep other applications' shortcuts/focus intact.

- [x] Implement transient hover keyboard input, ranked filtering and query feedback.
- [x] Verify ranking/reset behavior and required Windows MSVC checks; commit/push.

D41 evidence: Windows MSVC fmt/release Clippy/tests pass (72 passed, 2 ignored).
Ranking tests cover prefix-before-substring, stable existing order, grouped files,
case/Unicode names, empty/no-match query and unchanged board serialization.
A temporary keyboard hook exists only while a ring is open, checks actual pointer
geometry and skips menus/drags/Ctrl/Alt/Win shortcuts (AltGr remains text input).
It translates with the foreground thread keyboard layout, posts Unicode scalar
values to the UI thread, and never activates the ring. Backspace removes a scalar;
Escape clears, target changes/close reset, each edit resets paging and icon loads.
Physical keyboard/IME/dead-key composition and hover feel are unperformed; direct
Unicode layout translation supports ordinary Turkish typing, not an IME editor.
The sandbox initially denied the existing isolated registry test; the full required
suite passed with sandbox elevation. No user files/startup settings changed.

### D42: Reliable dragging, shared menus and consistent circle appearance

Owner request (2026-10-22): smooth middle dragging near taskbar/at speed, expose
circle-wide commands in the dock context menu, visibly mark locked circles and
use the same distinct centre/outer colors for Recent and ordinary rings.

- [x] Preserve the visible grab position and coalesce circle pointer input.
- [x] Share global commands with the dock; render a static gradient/lock state.
- [x] Verify required Windows MSVC checks and relevant isolated fixtures; commit/push.

D42 evidence: Windows MSVC fmt/release Clippy/tests/build pass (73 passed, 2 ignored).
Grid=3/topmost TEMP native fixture passes shifted-centre free movement/snap,
lock while visible/hidden, dock movement/restart, hover frame retention, paging,
search no-match/Backspace/Escape restoration and reference hashes. Four-DPI pixel
test verifies equal Recent/ordinary ring palettes and a distinct locked circle.
The first sandbox fixture could not persist dock movement; elevated runs exposed
and fixed lock repaint revealing a hidden board. Final full native fixture passes.
Real pointer is untouched; physical high-speed capture/keyboard feel remains an
owner acceptance check. The keyboard hook still needs physical layout assessment.

Middle press now works anywhere inside the ring and preserves its visible centre
as the drag origin after the threshold. A pending short press does not relocate it.
The mouse hook coalesces screen samples even with capture; native duplicate release
cannot outrun the queued final sample. Other circles cannot hover-open during a drag.
Dock right-click shares New circle/Grid/Desktop/Exit commands and exposes each
circle's group/rename/delete/sort commands under Cemberler. Lock adds a static
periwinkle/mint rim plus the existing antialiased small lock, with no idle timer.
Recent uses the ordinary dark outer area and lighter centre.

### D43: Keep typing, sorting and icon work bounded

Owner follow-up (2026-10-22): lightweight behavior is an acceptance constraint.
Avoid repeated ranking sorts, repeated Recent enumeration/suggestions while typing,
reloading unchanged page icons and multiple concurrent shell workers. Keep idle
CPU/timers/threads unchanged; record measured checks separately from physical feel.

- [x] Use linear stable prefix/substring ranks and reuse page items/icons.
- [x] Coalesce shell requests in a single temporary worker; discard obsolete work.
- [x] Remove quadratic Added-sort lookups without changing ordering semantics.
- [x] Verify stress/correctness and required Windows MSVC checks; commit/push.

D43 evidence: final Windows x86_64-pc-windows-msvc fmt/release Clippy with warnings
denied/tests/build pass (76 passed, 2 intentionally ignored). Tests include 10,000
stable ranked matches, Turkish I/Idot tolerance, 1,000 coalesced worker requests,
cancellation/idle exit, Added-sort ASCII-case/first-rank/stable-unknown semantics.
Search takes one transient candidate snapshot, filters names without disk/metadata
queries or ranking sorts, and releases the snapshot on clear/target change/close.
Page changes reuse loaded icons; incomplete loads are reissued with current request
IDs. One temporary worker holds only the newest pending page, checks cancellation
between shell calls and exits after its queue drains. No new dependency/idle timer.
Keyboard translation corrects only modifiers, replacing 256 async key-state calls
per character with one keyboard-state read and bounded modifier reads.

Final Grid=3/topmost and item transfer/reorder/group/Back/Recent/cancel native
fixtures both pass, reference hashes unchanged. Earlier item fixture interruptions
(closing before press or fallback pointer routing before synthetic release) also
reproduced on D40; the final isolated run passes on D43. The original physical
high-speed mouse/hover/keyboard/IME acceptance checks remain unperformed. The
locked-circle raster preview was inspected: small top-right lock, legible text,
static mint/periwinkle rim and smooth boundary. Full physical feel is not inferred.

Three-run closed-circle benchmark medians, 3-second idle windows, old installed
D40 vs final D43 (target/d43-perf/memory-closed.txt):

| Scenario | D40 commit KB | D43 commit KB | D40/D43 private WS KB | Modules | Handles | Idle CPU ms |
| --- | ---: | ---: | --- | ---: | ---: | ---: |
| Recent only | 2268 | 2272 | 148 / 136 | 32 / 32 | 156 / 156 | 0 / 0 |
| Five circles, 300 references | 2540 | 2572 | 144 / 140 | 32 / 32 | 156 / 156 | 0 / 0 |

The commit increases are 4 KB and 32 KB; both remain below the recorded v1 idle
panel commit baseline (~4644 KB). Open-ring benchmark attempts were interrupted by
pointer-free fixture dismissal, so no open-ring memory figure is claimed. The
benchmark now offers CirclesOnly/ClosedOnly and terminates test processes on error.
No user's files, attributes, Desktop entries or startup preferences were changed.

Updated both the workspace's ignored DeskTuk.exe (previously running D39) and the
canonical installed EXE via normal controller shutdown/restart; launched installed
PID 7160. Configuration was byte-identical across restart. Previous binaries and
config snapshot: target/d43-perf/. Installed/release SHA256:
752B2694EC6A0392FFFF0584A8D1B985164CA8D939A5F48A5EEDF5448D9542DF.
Rebuilt installer: dist/DeskTuk-0.1.0-Setup-x64.exe, SHA256:
EB3B30A84469184BA71A61A663A3ED29EDF8BB1F6233D5D10F1FC70D51D07C99.
D40 CI 39280110892 passed. D41-D43 CI runs on push; not claimed passed locally.

### D44: Consistent circle typography and search feedback

Owner request (2026-10-22): use the open ring's small number size in both states;
highlight query/results without moving the title. Keep one-character search.
Place a grey lock between the closed circle title/count with a thin teal rim;
retain readable text. Owner explicitly defers automated/runtime/screen tests until
manual acceptance, overriding the usual pre-commit cargo test step for this work.

- [x] Implement shared count typography, stable highlighted search and centred lock.
- [x] Format/static-check/build, commit/push and deliver for owner manual assessment.
- [ ] Owner manual acceptance, then automated/runtime checks when requested.

### D45: Aligned dock controls and lightweight eased transitions

Owner clarified: lock on the left and show/hide menu on the right in both modes;
keep panel top aligned, add bottom padding, borderless rounded hover treatments.
Ease menu expand/collapse, lock changes and hover feedback. Animations are short,
reversible, event-driven and stop at endpoints; no new dependencies/idle polling.
Owner defers tests until after manual assessment; run only format/static/build now.

- [x] Implement layout/padding, hover treatment and transient eased animations.
- [x] Format/static-check/build, commit/push and update running app/installer.
- [ ] Owner manual acceptance, then automated/runtime checks when requested.

D44 implementation: both numeric labels use the open ring's regular 100% font.
Query and result count occupy fixed centre bands (mint query; accent zero count),
leaving title bounds unchanged. Single-character search is retained. Locked circles
use a thin static mint rim and a grey 14-DIP lock between title and count. No idle
work or dependency added. cargo fmt, release Clippy (-D warnings), release build
pass. Owner expressly deferred tests; no cargo test/native/screen test was run.
Commit uses [skip ci] to preserve owner-first manual acceptance. Final deployment
follows D45 so both visual changes are delivered together. D43 CI status is not
refreshed here; no new CI/test pass is asserted.

D45 delivery (2026-10-22): left lock/right menu controls exist in both panel modes.
Controls are borderless at rest with rounded 120ms eased hover fills. Bottom list
padding is 8 DIP and is included in work-area fit calculations. Panel dimensions,
visibility-tile position and both header control positions interpolate over 190ms
with ease-out cubic; reversing samples the current visible geometry. The top edge
is retained across collapse/expand unless the expanded window would exceed the
work area. Moving the dock returns it to its taskbar rail; drag cancellation
restores the mode-specific top anchor as well as the saved position. Tray/menu
show recovery uses the same eased expansion. Header logo gives its space to the
left lock; original icon resources and the central visibility glyph are unchanged.

Lock changes crossfade cached open/closed glyph coverage with the current hover
background; a reused 20-DIP frame avoids per-frame bitmap allocation. Panel backing
buffer allocates at the transition's maximum size and shrinks at completion.
Row/control hover reversals sample current intensities, and the visibility dwell
fill now uses the same easing family. One 16ms UI timer runs only during the
120/190ms transitions, is killed on completion/cancellation/destruction and ignores
queued inactive ticks. No new dependency, thread, persistence key or idle polling.

Only cargo fmt, cargo clippy --release --locked -- -D warnings and cargo build
--release --locked were run. All pass. At owner's explicit instruction, no cargo
test, native fixture, screenshot/Playwright interaction or benchmark was run.
D44/D45 commits carry [skip ci] to avoid automatic test runs before owner manual
acceptance. Existing native fixture header coordinates/immediate resize assertions
will need adapting when the owner requests subsequent automated checks. Appearance,
feel, alignment at all monitor edges/DPIs and timer/resource behavior are proposed
implementation behavior, not runtime test evidence. Manual acceptance remains open.

Installed the compiled D44/D45 release for owner testing via normal controller
shutdown/restart (PID 2212), updating both installed and ignored workspace EXEs.
Config hash unchanged across restart. Backups: target/d45-delivery/. Release SHA256:
6FDCFC2AB66DFE521A66B9F075BC892DE9DB7EA65AE5B7051CE0FC4EE0F47814.
Rebuilt dist/DeskTuk-0.1.0-Setup-x64.exe SHA256:
797AA163CBEE01556B9118D0C3D32AABFFB0D2B0A343D4245307EEDC226E3E9A.

### D46: Clear lock glyphs and consistent menu transition frames

Owner manual feedback (2026-10-22): enlarge the small lock, remove its state
crossfade, repair stuttering menu expansion/collapse, add a teal outlined menu
button with a larger centred chevron, and revisit the locked-circle gradient.
Prior owner instruction still defers automated/runtime tests until requested.

- [x] Enlarge locks, remove lock-state easing, outline/centre teal menu glyph.
- [x] Keep one UI timer running without resetting it on hover changes; sample
  panel/control geometry once per frame and present each completed resize frame.
- [x] Fade expanded labels in fixed slots instead of dropping them at collapse
  start; preserve sampled content opacity when reversing a transition.
- [x] Restore a static mint-to-accent diagonal rim on locked circles.
- [x] Format/static-check/build, deliver installed app/installer, commit and push.
- [ ] Owner manual acceptance; automated/runtime tests only when requested.

cargo fmt, cargo clippy --release --locked -- -D warnings, and cargo build
--release --locked pass. No cargo test, UI interaction, screenshot or performance
benchmark performed. [skip ci] preserves the owner's manual-test-first request.
Smoothness is an implementation change awaiting physical acceptance, not a test
result. No dependencies, background threads or idle animation work added.
Circle lock raster size increases 14 to 22 DIP; panel lock 20 to 26 DIP. Lock
state switches immediately, retaining hover feedback. Menu uses pixel geometry
instead of font-dependent chevron metrics, with a static rounded teal border.
The 16ms timer is no longer restarted by hover state changes. Window bounds and
control rectangles share a sampled ease progress, followed by synchronous paint.

Installed PID 15324 from LOCALAPPDATA/Programs/DeskTuk/DeskTuk.exe; also updated
ignored workspace EXE. Initial copy encountered a still-running workspace instance;
its normal controller close completed before replacing the file. Backups are in
target/d46-delivery/. Config comparison differs only in saved APPS circle position
(1903,448 to 2133,823); no config rewrite/restore was performed by delivery.
Release/installed SHA256:
294E07EEFCC7ECF8F226C18586EAEF13483E96149871FDC409E098BC48BD5EF0.
Installer dist/DeskTuk-0.1.0-Setup-x64.exe SHA256:
53A39EB81C816F9BDC9B1ABBC0E928138864A3BEFB0A24C6B001BA3B5B29F54D.

### D47: Stable circle centres, touchpad dragging and lazy dock launch menus

Owner manual feedback (2026-10-22): retain a lighter/thinner centre border when
open, stop title/count jumping, give lock/menu matching controls and separate
compact controls above the square. Support left dragging for touchpads. Owner
clarified that content access belongs in dock right-click > Circles, not a new
folder-backed circle model, and requested click-to-open for files/programs/folders.

- [x] Keep a 2-DIP centre accent rim in open rings; share title/count font and
  diameter-relative vertical bands between closed/open states.
- [x] Match rounded teal lock/menu outlines; give compact controls their own
  28-DIP strip above the visibility square (64x92 DIP compact panel).
- [x] Left drag from ring centre/background uses existing threshold/capture and
  coalesced pointer routing; retain middle drag, item drag and centre-click paging.
- [x] Populate circle/group launch menus only on WM_INITMENUPOPUP; use text-only
  stored references, including Recent. Retain circle management in a submenu.
- [x] Bound native menu construction to 32 files per page; further pages share the
  source snapshot and populate on entry. Release transient session on dismissal.
- [x] Open selected references with the existing Windows ShellExecute path.
- [x] Format/static-check/build, install, rebuild installer, commit and push.
- [ ] Owner manual acceptance; automated/runtime tests only when requested.

No folder enumeration, icon extraction or metadata lookup is needed for the new
launch menu. Thus no spinner/worker/dependency/idle timer was added. Filenames are
shown directly (including extension); literal ampersands are escaped. This exposes
existing references and does not alter user files or persist new model fields.

cargo fmt, cargo clippy --release --locked -- -D warnings, cargo build --release
--locked and git diff --check pass. Owner's test-first preference means manual
assessment first: no cargo test, native/UI/screen test or benchmark was performed.
Commit uses [skip ci]. Physical left drag, lazy native menu delivery, launch actions,
compact layout and typography remain unverified; adapt old size/coordinate fixtures
when automated testing is requested. No runtime pass is inferred from compilation.

Installed PID 25880 using normal controller shutdown, also updated ignored root
EXE. Configuration hash unchanged. Backups: target/d47-delivery/. Installed/release
SHA256: 1D972121E8CCEE8792DB5FA01C67B1BDC9B16261031608A526385C9A7592A399.
Installer dist/DeskTuk-0.1.0-Setup-x64.exe SHA256:
C480854B840DD51DE10C45E4DF5C8E3A65F63842E9EBC28006B9A56FDA7C61DE.

### D48: Bilingual settings, quick guide, lock hints and explicit grid alignment

Owner approved native settings with Turkish/English selection, an in-app short
guide, optional themed lock hints, and a grid menu capped at 8x8. Grid selection
must not move current circles; a separate Align now command performs placement.
Owner's previous manual-first instruction still defers automated/runtime tests.

- [x] Persist language and usage-tip preferences with backward-compatible defaults.
- [x] Add expanded-panel gear/info buttons and settings/help context-menu entries.
- [x] Translate app-owned circle/panel/tray/menu/search captions and messages using
  static text; preserve user names, paths and OS-owned shell captions.
- [x] Include a native quick-guide dialog and bilingual docs/user-guide.md.
- [x] Show a delayed dark lock hint with a muted red border/target-facing arrow over
  disabled panel actions/rows; make it optional. Stop timers on show/cancel and
  release the transient layered window on leave, interaction or shutdown.
- [x] Support Off/2x2/3x3/4x4/6x6/8x8 in config, snap and existing guide rendering.
- [x] Add Align now: nearest suitable free cells per circle's monitor, reserving
  other circle positions and leaving an item in place when no candidate fits.
- [x] Use 4x4 on first installation; preserve existing grid choices, including Off.
- [x] Format/static-check/build, install, rebuild installer, commit and push.
- [ ] Owner manual acceptance; automated/runtime checks only when requested.

Language selection applies immediately and saves language=en when English is
selected; Turkish remains the legacy default. usage_tips=0 disables hints; older
configs enable them. Settings/help use existing user32 native UI. No dependency,
worker or filesystem enumeration added. Built-in Recent is localized; user-created
circle/group names are not. Explorer/shell-owned menus retain Windows' own language.

Hints allocate only on demand after a 550ms hover over a disabled panel control.
They are click-through, do not activate, clamp to the target monitor work area and
choose a left/right arrow placement. The one-shot timer stops before rendering;
queued duplicate ticks do not allocate another window. No idle polling added.
The guide is also available from right-click when the compact panel hides buttons.

Grid selection only saves the size; drag release uses it later. Align now includes
Recent, closes the open ring and applies one saved placement update. It considers
at most 64 cell candidates per circle; candidates account for circle diameters and
reserve current positions of unprocessed/excess circles. Dense grids may leave
circles in place when they cannot fit without intersecting another circle. Existing
config files without a grid entry still mean Off; only a truly new install gets 4x4.

cargo fmt, cargo clippy --release --locked -- -D warnings, cargo build --release
--locked and git diff --check pass. Per owner request, no cargo test, native/UI/
screen check or benchmark performed, and CI is skipped. Translation completeness,
icon legibility, tooltip placement at alternate font sizes/DPIs, settings persistence
and grid placement behavior await manual acceptance; build success is not runtime
evidence. Existing automated fixture text/coordinate expectations may need updating
when subsequent testing is requested.

Installed PID 13404 via normal controller close/restart; ignored root EXE also
updated. Configuration hash unchanged across delivery. Backups: target/d48-delivery/.
Installed/release SHA256:
2BFF8E806741F01D2ADAC5ACA801B477E22E7BF8456B14C23A500A65DCE03DFB.
Installer dist/DeskTuk-0.1.0-Setup-x64.exe SHA256:
3D967A5BA4ABD5F9F29B24E1E0565593F1BABEB2A43159F590EA4690366D88E8.

### D49: Separate dock tools, smooth glyphs and restore lazy menu notifications

Owner screenshots show the centre lock hint cut off by the dock and Circles
submenus stuck at their placeholder. Owner requests antialiasing on the info icon
and a grid shortcut beside settings/info in a row below the centre visibility tile,
with the circle list below. Existing manual-first test preference remains active.

- [x] Place lock hints outside the whole dock rather than its inset hovered tile,
  retaining work-area clamping and the target-facing arrow without z-order polling.
- [x] Remove TPM_NONOTIFY from the circle popup so WM_INITMENUPOPUP can populate
  lazy contents; retain return-command dispatch, bounded pages and session cleanup.
- [x] Put settings/grid/info in a dedicated 30-DIP row below the visibility tile.
  Share list origin across painting, hit testing, scrolling and work-area sizing.
- [x] Open grid choices directly below its shortcut; reuse grid menu construction.
- [x] Supersample all three tool glyphs at 4x4 coverage; cache masks per DPI/size
  and reuse one compositing surface for hover paints. Release cache on shutdown.
- [x] Format/static-check/build, install, rebuild installer, commit and push.
- [ ] Owner manual acceptance; runtime/automated checks only when requested.

This is a small placement fix for the tooltip rather than new topmost arbitration:
its body and arrow now fit outside panel bounds when side space is available.
WM_INITMENUPOPUP is already routed by the controller; TPM_NONOTIFY had suppressed
menu preparation notifications, leaving disabled '...' items visible in the user's
screenshot. Removing that flag retains lazy text-only population without filesystem
I/O or icon loading. Runtime confirmation is explicitly left to the owner.

Glyph coverage is computed once on first use or DPI change. Three byte masks and
one reused 26-DIP DIB cost roughly 4.7 KB at 96 DPI; there is no new idle timer,
worker or dependency. Pixel coverage blends glyph edges with the current hover
background, preserving sharp interiors rather than applying a blur filter.

cargo fmt, cargo clippy --release --locked -- -D warnings, cargo build --release
--locked and git diff --check pass. No cargo test, live screenshot, routed-input
fixture or benchmark run. Only owner-provided screenshots were inspected. CI is
skipped pending manual assessment. Verify lazy file launching, toolbar hit regions,
info edges and lock hint placement physically; no runtime pass is inferred.

Installed PID 20472 via normal controller close/restart; ignored root EXE updated.
Config hash unchanged. Backups: target/d49-delivery/. Installed/release SHA256:
7F371D8FB5D3341AB689A2CB80A0E80D953F8F973DA9CC113FEBF5405067BA66.
Installer dist/DeskTuk-0.1.0-Setup-x64.exe SHA256:
9B33B57709B52BEB0EFB1C99822E53179028ECF23B887E48FC05A3546094E59E.

### D50: Single quick guide, spacious shortcuts and immediate dock mode changes

Owner reports repeated help windows and expansion redraw glitches; approves the
proposed wider single-row layout with a compact Circles shortcut. Owner explicitly
allows removing resize easing if it adds complexity/cost; retain hover feedback.

- [x] Guard the native quick-guide call against re-entry during MessageBox's nested
  message loop. Repeated clicks restore/foreground only the existing owned dialog;
  reset guard on return. Match both language captions, preserving locale changes.
- [x] Widen expanded dock from 153 to 184 DIP; place Settings/Grid/Info/Circles
  44 DIP apart. Add a subtle one-pixel divider below the four shortcuts.
- [x] Add Circles below the compact square, keeping compact width 64 DIP and
  increasing height to 132 DIP. Retain aligned top placement/work-area clamping.
- [x] Share on-demand content menu construction/dispatch with the existing dock
  context menu. References/groups/actions remain lazy and text-only.
- [x] Cache 4x4 supersampled coverage for the new circle glyph too; existing gear,
  grid and info use the same antialiasing path.
- [x] Remove panel resize/content interpolation and its intermediate geometry.
  Apply final layout once with a completed buffered redraw; retain hover/dwell ease.
- [x] Format/static-check/build, install, rebuild installer, commit and push.
- [ ] Owner manual acceptance; automated/runtime tests only when requested.

No worker, dependency or persistent model field added. Guide guard is transient
and set before the message-pumping native call. Re-entry searches only native
message dialogs owned by this controller, never another application's dialog.
Removing resize animation removes repeated SetWindowPos and intermediate backing
buffer sizes; one 120ms timer remains for active hover transitions only. Circle
opening/reveal animations are unchanged. Four 26-DIP coverage masks and a reused
frame remain small; no idle icon computation added.

cargo fmt, cargo clippy --release --locked -- -D warnings, cargo build --release
--locked and git diff --check pass. No cargo test, UI interaction/screenshot test,
native fixture or benchmark performed, as owner requests manual acceptance first.
CI skipped. Guide re-entry/focus, exact mode-toggle presentation, divider/spacing
and compact Circles launch behavior await physical assessment. Existing old panel
size/coordinate fixture assertions will need adapting when tests are requested.

Installed PID 15128 via normal controller close/restart; ignored root EXE updated.
Configuration hash unchanged. Backups: target/d50-delivery/. Installed/release SHA256:
519FB658DF9F8C8B527A5CD6B2A44BF76C1A5E904F267AE8FD6400558269148D.
Installer dist/DeskTuk-0.1.0-Setup-x64.exe SHA256:
6F3C0C51D9B2DBB7BD6DA9209063D94DCAD272118B4388A2E9D11D34D7BE5889.

### D51: Row launch menus, perimeter dragging and consistent compact insets

Owner prefers using expanded dock rows as content launch menus instead of a
separate Circles shortcut, keeping the compact Circles button. Requests dragging
from any blank panel area along all monitor edges/corners, unchanged right-click
menus, cleaner compact borders and aligned toggle padding. Owner confirms instant
mode changes are preferred over further resize easing work.

- [x] Remove expanded Circles shortcut; use three spaced tools in a 168-DIP panel.
  Circle rows get native submenu chevrons and directly open their file/group menu
  without revealing the circle board. Existing right-click choices remain.
- [x] Keep compact Circles with an outlined, cached antialiased glyph. Remove compact
  upper control borders, retaining hover fills; expanded borders are unchanged.
- [x] Give header controls matching 7-DIP top/4-DIP side insets and 26-DIP bounds
  in both modes, fixing the old compact toggle's 6-DIP vertical/2-DIP lateral shift.
- [x] Start dock dragging from all client areas except active buttons/list rows;
  blank padding cannot accidentally activate a row on release.
- [x] Choose the nearest usable monitor edge during drag rather than preferring
  the taskbar edge; keep the entire panel clamped to its rail/corners, never free
  in the middle. Reuse existing coalesced pointer routing and drag threshold.
- [x] Persist dock_edge with dock_pos on release. Cancel restores old edge, anchor
  and top; older configs retain their original edge until the first new drag.
- [x] Preserve rail attachment across mode/DPI/display changes. Left/right modes
  retain top where possible; top/bottom modes stay attached to their chosen rail.
- [x] Format/static-check/build, install, rebuild installer, commit and push.
- [ ] Owner manual acceptance; runtime/automated checks only when requested.

Native menus follow Windows' appearance as the owner permits, avoiding owner-drawn
menu resources. They share existing 32-entry lazy launch/group/action handling.
Unused dock-to-ring toggle functions are removed now that rows open menus instead.
No file enumeration, dependency, worker or idle timer added. The compact outline
is part of its cached supersampled mask so drawing the glyph cannot erase it.

The matching button insets are internal padding; the existing screen/work-area gap
is separate. On a right-edge panel with a retained top, the toggle's screen centre
now matches between modes. On other edges, preserving rail attachment takes priority
when different mode sizes necessarily change the window's opposite edge.

cargo fmt, cargo clippy --release --locked -- -D warnings, cargo build --release
--locked and git diff --check pass. No cargo test, native fixture, screen/UI test
or benchmark performed under the owner's manual-first request. CI skipped. Perimeter
feel/corner transitions, edge persistence/cancel, row launch actions, compact borders
and toggle alignment at alternate edges/DPIs remain for owner acceptance; no runtime
pass is inferred from compilation. Older dock fixtures assumed taskbar-only rails.

Installed PID 14916 via normal controller close/restart; root EXE also updated.
Config hash unchanged across delivery. Backups: target/d51-delivery/. Release SHA256:
C1E591AFD030FE8D90602326D6763FA34F762B4EDA8FFB95C39C76B2E53D8EB1.
Installer dist/DeskTuk-0.1.0-Setup-x64.exe SHA256:
F10C9380667FED23B9E423BDD4946594245E5F99C5D44D903F4AE469146EB9FE.


### D52: Useful usage tips, explicit file activation and native text selection

Owner requests positive usage tips as well as lock explanations, a search text field
above dock row contents, selection before file activation, and normal text selection
shortcuts in circle text fields. The manual-first/no automated tests request remains.

- [x] Keep Usage tips as the shared preference; add guidance for lock, menu, settings,
  grid, guide, compact Circles, visibility and row search. Use teal for guidance,
  retaining red for disabled locked controls, the existing 550ms dwell and no idle timer.
- [x] Replace direct dock-row file popups with one on-demand native EDIT/LISTBOX
  popup. Typing filters using existing stable prefix/substring ranks; a single click
  selects, Enter/double-click opens. Groups/Back navigate references. Right-click
  menus and circle management actions remain available through the existing menus.
- [x] Limit materialized result controls to 128 rows with previous/next pages and
  result count; snapshot candidates once per view, without directory reads or icons.
- [x] In remaining native file menus, handle WM_MENUCHAR with MNC_SELECT instead
  of executing a unique filename initial. Explicit Enter/click still launches.
- [x] Open an actual native EDIT field when ring typing starts. Keep the ring busy
  while editing, release hover keyboard interception, and update its existing linear
  search on EN_CHANGE. Enter dismisses keeping query; Escape clears. Query centre
  click and hover Ctrl+A reopen editing; native selection supports Shift/arrows/mouse.
- [x] Add explicit Ctrl+A to native rename EDIT controls too; exclude open rename/
  search popups from the hover keyboard route. Preserve ordinary Ctrl/Alt/Win shortcuts.
- [x] Use existing native font/theme, a teal field frame and work-area-clamped popup
  beside the dock; release popup handles, font, brush and snapshots on close/shutdown.
- [x] Format/static-check/build, install, rebuild installer, commit and push.
- [ ] Owner manual acceptance. No automated runtime, native fixture, screenshot,
  cargo test or benchmark until requested. CI intentionally skipped.

No dependencies, background workers or idle polling added. Search never changes
user files. Shared borrows are released before Create/DestroyWindow, SendMessage,
SetFocus/foreground and shell opening; local snapshots bridge native calls. Repeated
identical ring queries do not reload the page. Existing icon coalescing is retained.
The native text field deliberately takes keyboard focus when the user starts typing;
hover alone does not activate it. Native foreground restrictions, first-key handoff,
fast input, IME, selection/clipboard, alternate DPI/work areas, menu initial cycling,
page/group navigation and placement remain runtime acceptance items, not inferred
passes. No resource benchmark was performed; idle footprint is structurally unchanged
until the on-demand popup or pending tooltip exists.

cargo fmt, cargo clippy --release --locked -- -D warnings, cargo build --release
--locked and git diff --check pass. Owner performs usage tests first.

If Windows refuses foreground permission after hover typing, the editor closes and
restores hover interception instead of leaving a misleading unfocused edit box.
The filtered query remains; clicking its centre can reopen editing through normal
mouse input permission. Dock mode/drag/lock changes dismiss stale search popups.

Installed PID 30864 through normal controller close/restart; ignored root EXE
updated too. Configuration hash unchanged; backups target/d52-delivery/.
Installed/release SHA256:
19DA879713AED3BE2098112622AFACBAD49517ED7267E1E4C0B203BA1A3027EA.
Installer dist/DeskTuk-0.1.0-Setup-x64.exe SHA256:
A5F63712EF9A6E04BB8AEB10447CB811E3035CE3DCC692EE8DDCCE3446B08623.


### D53: Restore creation of file and ring text popups

Owner reports dock file rows do nothing and ring Ctrl+A still cannot select the
painted query. Inspection identifies a shared D52 defect: the search popup's
WNDCLASSW omitted lpszClassName, so RegisterClassW and CreateWindowExW failed.
The hover fallback could still paint/filter a query, but no real EDIT was created.

- [x] Supply the stable DeskTuk.Search class name during registration.
- [x] Accept ERROR_CLASS_ALREADY_EXISTS for repeat opens, but log/return on other
  registration errors. Log popup/control creation failures rather than silently return.
- [x] Format, release Clippy/build and diff checks; install and rebuild installer,
  commit and push under the owner's manual-first acceptance workflow.
- [ ] Owner manual acceptance: dock row opens searchable list; ring typing displays
  native edit; Ctrl+A replacement, Shift+arrows/mouse and repeated close/reopen.

The fix is shared by dock lists and ring editing; it adds no polling, dependency,
worker or filesystem access. No automated tests, UI input/screenshots or benchmark
performed, and CI remains skipped as requested. Compilation/static checks establish
build correctness, not physical focus/selection acceptance.

cargo fmt, cargo clippy --release --locked -- -D warnings, cargo build --release
--locked and git diff --check pass. Installed PID 2072 via normal controller close;
configuration hash unchanged, ignored root EXE updated. Backups target/d53-delivery/.
Installed/release SHA256:
587C02869A315C3C1C17A87BF35CCE95B19CAE980B61A0D5D06E7129CFA58E1F.
Installer rebuilt at dist/DeskTuk-0.1.0-Setup-x64.exe. Runtime acceptance remains
with the owner; no automated interaction/test run performed.
