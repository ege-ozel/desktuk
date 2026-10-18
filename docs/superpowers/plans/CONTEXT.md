# Repository context
Keep this guide under 100 lines. Live Git/CI outrank snapshots.

## Start here
- Inspect `git status --short --branch` and `git worktree list`.
- Main is the integration baseline; owner permits direct main commits/pushes.
  Do not open/reopen PRs unless requested. Preserve task-by-task commits/checks.
- Reuse the checkout owning the work. Normal workspace: D:/CodeSpace/desktuk.
- A and D through D29 are delivered; D30/D31 address fast dragging and the dock.
  B1/B2/C are superseded/on hold; B3 was delivered through D9.
- Owner requires circles and open rings always topmost, including Show Desktop.

## Delivered behavior and latest changes
- Desktop file references are grouped in circles; Recent, hover rings, paging,
  native shell icons, context menus, persistence, tray and startup are implemented.
- D29 uses middle-button drag to move closed circles or an open ring's centre.
  Move freely while held; snap only on release. Left centre click turns pages.
- Grid Off / 2x2 / 3x3 / 4x4 uses the current monitor's taskbar-free work area;
  guides mark the target cell and landing centre until release/cancel.
- D29 installed SHA256: 26FDAFAD6B981E19A8B5FD37A411DD721383107FD272C09DE514AFF7F962A609.
  CI 39163071663 passed. Prior binary: target/d29-middle-drag/previous-installed.exe.
- D30 fixes stale queued fallback client coordinates using coalesced screen samples,
  preserves release, and prevents native/routed streams from moving the same press twice.
  Local MSVC checks, 1,000-sample queue test and Grid=3 open-circle fixture pass.
- D31 sizes the dock to its Windows icon-title font, with equal side padding. The
  Desktop heading's right arrow collapses it to the visibility square. Right-click
  offers menu show/hide; tray/show recovery restores the expanded dock.
  Collapse state is session-only. Physical visual/interaction assessment is pending.
- Pointer entry opens immediately with a short eased reveal; closing takes 110 ms.
  Circle surfaces use pixel-coverage blending. No new dependencies or idle polling.
- Product proposals and priorities: ../reviews/2026-10-18-product-review.md and D30 notes.

## Observed defects / checks not performed
- Owner reports D29 fast middle dragging after first hover can stick at grid lines
  or arbitrary positions; Print Screen appears to clear it. D30 addresses a concrete
  queued-coordinate defect; the physical symptom and Print Screen effect need assessment.
- Physical mouse drag/capture, alternate DPI/sizes, menu/hover feel and visual quality.
- Physical Win+D, taskbar corner click and Peek on the installed always-topmost build.
- Actual reboot/login with startup enabled; real Explorer restart and recovery.
- D5 menu/rename/delete/live-update/drag-drop/persistence interaction checklist.
- Local tray fixture needs the normal installed instance absent; it passed in CI.
  Do not alter production single-instance behavior to bypass that precondition.
- Do not run scripts/windows-integration-test.ps1 without an explicit task: it generates
  physical input and Desktop writes. Isolated routed-message fixtures are separate.

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
| Dock/tray/startup/desktop visibility | src/dock.rs, tray.rs, autostart.rs, desktop_view.rs |
| Bootstrap/message loop | src/app.rs |
| Isolated runtime fixtures | scripts/*regression-test.ps1 |
Short names share the preceding directory. Tests live with their Rust module.

## Bounded reading
- Active plan: 2026-10-13-phase2-d-circles.md; current follow-ups D30/D31.
- Required phase design: ../specs/2026-10-13-phase-2-design.md.
- Product decisions: ../specs/2026-10-13-circles-design.md.
- Historical implementation/evidence: ../archive/2026-10-13-phase2-d-circles-implementation.md.
  Historical code is not current implementation instruction.
- Read required design/task once, then relevant functions. Do not dump whole modules.
- Search src/ first, then scripts/ and the active plan. Use `rg -n` / `rg --files`;
  fallback: `git grep -n`. .rgignore excludes generated/history files by default.
- Bounded PowerShell reads: `Get-Content <file> | Select-Object -Skip <start> -First <count>`.
- Root config.rs and .capture-* are ignored scratch; src/config.rs is authoritative.
  target/, dist/, captures, logs, executables and bundles are generated/local artifacts.
- AGENTS.md governs safety, branches, commits and required Windows MSVC checks.
  Keep evidence in the plan; update changed facts here in the same commit.
