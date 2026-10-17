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
- A and D implementation are complete. B1/B2 and C were superseded by circles;
  tray/startup (B3) was delivered through D9. Do not start postponed plans.

## Delivered behavior and evidence

- Circles group Desktop file references; Recent, groups, suggestions, hover rings,
  paging, native-sized shell icons, menus, persistence, tray and startup are present.
- Capture cleanup restores the circle host (or ring owner's host) ordering:
  tray-revealed circles stay visible on the desktop; real apps remain above them.
- Installed runtime source: `7e8c557`; both Windows CI runs passed, including tray,
  open-ring and runner-only desktop/Explorer checks. Later consolidation is limited
  to docs/search/CI/ignore rules; query live checks for the current main revision.
- Latest drawing MSVC checks: 47 passing tests, one ignored visual preview;
  the native preview was also run explicitly and inspected.
- Installed executable: D:/CodeSpace/desktuk/DeskTuk.exe.
  Source SHA256/deployment details: active Plan D and its historical archive.
- Owner approved installation/restart; responding app/tray and unchanged config,
  startup command and desktop visibility preference were verified.
- Circle smoothness is owner-reported; edge vs animation vs dragging is unconfirmed.
  Outer alpha masking exists; inner GDI ellipse boundaries lack the same smoothing.
  This is a possible cause, not a diagnosis. Owner will test; appearance is unchanged.

## Unperformed manual checks (not observed failures)

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
