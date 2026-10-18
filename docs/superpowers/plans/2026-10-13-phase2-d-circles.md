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

- [ ] Draw the grid frame once per layout; move a separate reusable target-cell surface
      instead of rebuilding a monitor-sized bitmap at each cell crossing.
- [ ] Defer shell unloading/working-set trimming until interaction ends; avoid rendering
      a moving circle back at its saved position during background refresh.
- [ ] Verify native guide reuse/geometry and measure repeated cell crossings, retain
      grid Off and release-only snapping; run required MSVC checks and regressions.
- [ ] Push, confirm code CI, deploy the checked release and preserve configuration;
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
