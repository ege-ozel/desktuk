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
- A and D through D20 are implemented/installed; evidence is in Plan D.
  B1/B2 and C were superseded by circles;
  tray/startup (B3) was delivered through D9. Do not start postponed plans.
- D21 visible drag grid and D22 configurable always-on-top / Peek protection are
  implemented/tested; deployment pending. See active Plan D.

## Delivered behavior and evidence

- Circles group Desktop file references; Recent, groups, suggestions, hover rings,
  paging, native-sized shell icons, menus, persistence, tray and startup are present.
- Capture cleanup preserves the selected circle band: always-on-top (default) or
  desktop-only (apps cover circles). Panel ordering is unchanged.
- Installed runtime source: `d1e1174` (D17-D20 interactions). Windows CI passed;
  local tray/open-centre/grid/missing-reference runtime fixtures passed.
- Required MSVC checks: 56 passing tests, one ignored preview; the preview was also
  run explicitly and inspected. Inner rim blending/symmetry pass at 96/120/144/192 DPI.
- Pointer entry opens immediately: no hover dwell or opening animation. Closing
  remains 110 ms. Topmost circles reject minimize/demotion; stale focus events skip.
- Internal item drag shows a click-through icon. Right-click Izgara selects Off /
  2x2 / 3x3 / 4x4; release snaps to cell centres in that monitor's taskbar-free work
  area. Grid defaults Off; holding a circle shows cell/landing guides until release.
- Closed circles, ring centres and group discs now use centered pixel-coverage
  blending. The outer premultiplied alpha mask remains; text/icons retain native
  rendering. No new dependencies, idle timers, threads or persistent buffers.
- Installation/restart succeeded; responding process/tray and unchanged
  configuration, startup command and desktop visibility preference were verified.
- Installed executable: D:/CodeSpace/desktuk/DeskTuk.exe. Full release SHA256,
  preview paths and deployment evidence are recorded under Plan D D15/D16/D20.
- The reported hard inner edge was reproduced in a native regression and fixed;
  the owner's final visual assessment on the running desktop is still pending.

## Observed unresolved behavior and unperformed checks

- Owner reported Win+D toggling circles and corner-button problems. D22 changes
  ordering and Peek exclusion; physical Win+D / corner click / Peek remain unverified.

- Physical mouse capture/drag, including an open ring and tray recovery.
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
