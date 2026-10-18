# Repository context
Keep this guide under 100 lines. It routes reading; live Git/CI outrank snapshots.

## Start here

- Use `git status --short`, `git branch --show-current`, `git worktree list`.
- Main is the integration baseline. Owner requested merging completed branches;
  direct main commits are allowed and PRs are not required. PR #3 is historical.
  Do not reopen it or create a PR unless the owner requests one.
- Use latest main directly or a plan branch when isolation helps; follow AGENTS.md.
- Do not rely on hard-coded checkout paths. The normal startup workspace is
  D:/CodeSpace/desktuk; keep it synchronized with main after merging.
- A and D through D24 are installed; D25 is implemented, checked, deployed and CI-passed.
  D26/D27 dock changes are built/deployed; fmt, Clippy, release tests, build and GitHub CI
  pass. Owner-side visual assessment remains pending. B1/B2/C were superseded; B3 via D9.
- Owner confirmed circles should always remain topmost, including open rings. D25 removes
  the older desktop-order default and makes the current dock compact and blue.

## Delivered behavior and evidence

- Circles group Desktop file references; Recent, groups, suggestions, hover rings,
  paging, native-sized shell icons, menus, persistence, tray and startup are present.
- D25 deployed `DeskTuk.exe` SHA256 `5FC17D25D611E06A2476BBC42026B50AEBF09DA8CD9CC5C55B203693B3AEC230`;
  runtime enumeration confirmed the compact dock and all three circles visible/topmost.
- D27 keeps a fixed `DeskTuk` heading and Recent/circle list, with a square click/300 ms
  hover visibility tile. The pill switch is removed and row-only invalidation limits flicker.
  Deployed `DeskTuk.exe` SHA256 `8676BF6F0360C0002876BAFAB55634F7CFF6AC564F3E8D736FD4851D2379B85D`;
  source uncommitted.
- D25 MSVC release checks, isolated circle/grid/topmost/tray fixtures and GitHub Windows
  CI run 39128090469 passed; physical Win+D, taskbar corner and Peek still need assessment.
- D24 runtime source: `9876047`. CI 39124382613 caught stale topmost fixtures; 39124809152
  passed. D23 CI 39118544698 and D24's 62 tests/ignored preview passed; rim checks passed
  at 96/120/144/192 DPI.
- Pointer entry opens immediately: no hover dwell or opening animation. Closing
  remains 110 ms. Topmost circles reject minimize/demotion; stale focus events skip.
- Internal item drag shows a click-through icon. Right-click Izgara selects Off /
  2x2 / 3x3 / 4x4; release snaps to cell centres in that monitor's taskbar-free work
  area. Grid defaults Off; holding a circle shows cell/landing guides until release.
- Circle surfaces use centered pixel-coverage blending; text/icons retain native rendering.
  No new dependencies, idle timers, threads or persistent buffers.
- Install/restart and no-activate topmost dock were verified. Deployment/preview evidence
  and full hashes are recorded under Plan D D15/D16/D20/D23/D24.
- The reported hard inner edge was reproduced in a native regression and fixed;
  the owner's final visual assessment on the running desktop is still pending.

## Observed unresolved behavior and unperformed checks

- Owner reports the installed D24 build still hides circles on Win+D. D25 changes circles
  and their rings to always-on-top; the installed D25 build's physical Win+D / corner click /
  Peek behavior remains unverified pending owner assessment.

- Physical mouse capture/drag, including an open ring and tray recovery.
- Owner reports that dragging a circle after opening its ring can stall and leave the grid
  visible. D28 adds a temporary low-level mouse fallback when foreground capture is denied;
  the isolated open-ring drag/grid fixture and GitHub CI 39160762962 pass. Physical drag
  feel on the owner's desktop remains pending. The checked release is installed at the
  existing `DeskTuk.exe` path; the prior binary is backed up under target/d28-drag-fallback.
- Owner reports D28 still cannot move an open circle and asks for gentler grid behavior.
  D29 uses middle-button drag and defers snap until release; free-move/snap fixture and local
  MSVC checks pass. GitHub CI 39163071663 passes. Dock labels use the Windows icon-title
  font. D29 release is installed at DeskTuk.exe; prior binary is backed up under
  target/d29-middle-drag. Physical owner review remains pending.
- Owner requested a review list before choosing later product changes; proposals are recorded
  in `docs/superpowers/reviews/2026-10-18-product-review.md`.
- Actual reboot/login with startup enabled.
- Real Explorer restart and tray/window recovery on the owner's machine.
- Physical menu/show-hide interaction, alternate sizes/DPI and visual quality.
- D5 menu/rename/delete/live-update/drag-drop/persistence interaction checklist.
- Local tray fixture needs the normal installed instance absent; it passed in CI.
  Do not alter production single-instance behavior to bypass that precondition.

## Source routing

| Topic | Files |
|---|---|
| Drawing/alpha/geometry | src/circles/draw.rs, surface.rs, geom.rs |
| Hover, paging, animation | src/circles/circle_win.rs, ring.rs |
| Capture and desktop ordering | src/circles/drag_capture.rs, src/host/window.rs |
| Drag icon preview | src/circles/drag_preview.rs, draw.rs |
| Grid snap / persistence | src/circles/grid.rs, grid_overlay.rs, circle_win.rs, model.rs |
| References and config | src/circles/model.rs, view.rs, src/config.rs |
| Desktop updates/Recent/suggestions | src/circles/desktop.rs, suggest.rs, src/watch.rs |
| Shell icons/menus | src/items.rs, shell.rs, shell_menu.rs |
| Tray/startup/desktop visibility | src/tray.rs, autostart.rs, desktop_view.rs |
| Bootstrap/message loop | src/app.rs |
| Isolated runtime fixtures | scripts/*regression-test.ps1 |

Short names share the preceding directory. Tests live with their Rust module.
The largest runtime module is ring.rs (about 900 lines); search symbols and read
the affected functions. Do not split stable code solely to reduce whole-file reads.

## Bounded reading

- Active follow-up: D27 in 2026-10-13-phase2-d-circles.md; physical Win+D / taskbar corner
  click, Peek, hover/menu behavior and visual scale remain pending owner assessment.
- Active plan: 2026-10-13-phase2-d-circles.md in this directory (short index).
- Product decisions: ../specs/2026-10-13-circles-design.md.
- Original implementation/evidence: ../archive/2026-10-13-phase2-d-circles-implementation.md.
  This archive contains historical code, not current implementation instructions.
- Read the relevant design/task once for implementation; retain safety constraints.
- Search src/ first, then scripts/ and the active plan. .rgignore skips historical
  archives/captures/generated output. Use explicit paths for history when needed.
- Example: `rg -n 'capture_finished|fn finish' src`.
- rg fallback: `git grep -n -e capture_finished -e 'fn finish' -- src`.
- Bounded PowerShell read:
  `Get-Content -LiteralPath <file> | Select-Object -Skip <start> -First <count>`.
- Root config.rs and .capture-* are ignored local scratch copies; src/config.rs is
  authoritative. target/, dist/, test-results*, logs, executables and bundles are
  generated/local artifacts, not default reading targets.
- AGENTS.md is authoritative for safety, branches, commits and required MSVC checks.
  Update only changed facts here; keep long evidence in the archive/plan.
