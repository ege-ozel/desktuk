# Repository context (read this before the long plans)

Snapshot: 2026-10-17. This is a routing note, not live CI or deployment evidence.
Keep this file under 100 lines. Update relevant facts in the same commit as changes.

## Locate the right checkout

- Run `git status --short`, `git branch --show-current`, `git worktree list`.
- Current feature work: `phase2/d-circles`, PR #3:
  https://github.com/ege-ozel/desktuk/pull/3
- At this snapshot the circles checkout is
  `C:/Users/4D/.codex/worktrees/circles/desktuk`.
  Discover its current location with Git; do not hard-code it in tools.
- `D:/CodeSpace/desktuk` is still on the older Plan A branch and contains local
  changes. Do not mistake it for the circles source or reset/switch a dirty checkout.
- Plan A is merged. B1/B2 and C are on hold; circles replaced that direction.
  Tray/startup (B3) was delivered through D9. Continue Plan D in its own checkout.

## Status: distinguish defects from missing verification

- D1-D11 are checked off in the Plan D implementation record.
- Last checked implementation commit: `47753f0`. PR #3 was open and its two
  Windows build-and-test checks passed when queried on 2026-10-17.
- Recorded committed-source tests: 44 passed, one ignored visual preview.
- Fresh local checks on 2026-10-17: fmt/clippy passed; 45 tests passed, one ignored.
  These include the existing uncommitted capture-cleanup regression.
  Re-query CI/PR for current claims; a green commit does not validate local edits.
- Owner reports a circle that is not smooth. Edge vs animation vs dragging is
  not confirmed. Outer alpha masking exists, but internal GDI ellipse boundaries
  lack the same smoothing. This is a possible cause, not a reproduced diagnosis.
  Owner will test; no drawing fix was applied for this request.
- Local, uncommitted capture-cleanup fix exists in drag_capture.rs and host code:
  a tray-revealed circle could be sent behind the desktop after a drag.
  The Plan D local note records a native regression; do not silently include
  these existing edits in unrelated commits or call them shipped/CI-validated.
- Installed DeskTuk.exe exists outside the circles checkout. Do not assume its
  source revision matches HEAD; deployment records/hashes alone are not a new test.

## Verification still needed (these are not observed failures)

- Physical mouse capture/drag, including an open ring and tray recovery.
- Actual reboot/login with startup enabled.
- Real Explorer restart and tray/window recovery (synthetic TaskbarCreated passed).
- Physical menu/show-hide interaction and checkmark agreement with Explorer.
- Visual icon parity at alternate sizes/DPI, paging, and circle edge quality.
- D5 manual interaction checks: shell menus while open, rename/delete ordering,
  live Desktop updates, drag/drop and persistence. Read the D5 manual checklist
  when doing that validation. Native/synthetic fixtures do not replace it.

## Read only the relevant source

| Topic | Start here |
|---|---|
| Circle/ring drawing | src/circles/draw.rs, geom.rs, surface.rs |
| Hover, opening, paging, animation | src/circles/circle_win.rs, ring.rs |
| Drag capture and desktop ordering | src/circles/drag_capture.rs, src/host/window.rs |
| File references, assignment, persistence | src/circles/model.rs, view.rs, src/config.rs |
| Desktop changes, Recent, suggestions | src/circles/desktop.rs, suggest.rs, src/watch.rs |
| Shell icons/context menus | src/items.rs, shell.rs, shell_menu.rs |
| Tray, startup, desktop visibility | src/tray.rs, autostart.rs, desktop_view.rs |
| Startup/message loop | src/app.rs |
| Runtime regression fixtures | scripts/*regression-test.ps1 |

The short names in a row share the preceding directory unless prefixed with src/.
Pure geometry/model tests live alongside the implementation.

## Plans, design and bounded reading

- Product decisions: docs/superpowers/specs/2026-10-13-circles-design.md.
- Active plan: docs/superpowers/plans/2026-10-13-phase2-d-circles.md.
  It contains thousands of lines of historical code samples; do not dump it.
- First inspect headings/checklists with `rg -n '^### Task|^- \[[ x]\]'`
  on the specific plan. Then read the chosen heading through the next task.
- If rg is unavailable, use `Select-String -LiteralPath <plan> -Pattern
  '^### Task','^- \[[ x]\]'` or `git grep -n -e '^### Task' -- <plan>`.
- Read bounded source ranges with
  `Get-Content -LiteralPath <file> | Select-Object -Skip <start> -First <count>`.
- Search tracked source/docs first. Exclude target, dist, .git, executables,
  bundles, test-results* and local capture/deployment scratch files.
- Before implementation, read the relevant design and complete active task once.
  Retain their constraints; reread only changed/needed sections.
- AGENTS.md contains the mandatory safety, branch, commit and check rules.
  This summary never overrides them. Add future evidence to the plan and keep
  this summary short; never turn missing verification into a claim of a bug.
