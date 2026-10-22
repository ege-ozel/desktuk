# Repository context
Keep this guide under 100 lines. Live Git/CI outrank snapshots.
## Start here
- Inspect `git status --short --branch` and `git worktree list`.
- Main is the integration baseline; owner permits direct main commits/pushes.
  Do not open/reopen PRs unless requested. Preserve task-by-task commits/checks.
- Reuse the checkout owning the work. Normal workspace: D:/CodeSpace/desktuk.
- A and D through D47 are implemented; D44-D47 await owner manual acceptance.
  B1/B2/C stay on hold; B3 was delivered through D9.
- Owner requires circles and open rings always topmost, including Show Desktop.
## Delivered behavior and latest changes
- Desktop file references are grouped in circles; Recent, hover rings, paging,
  native shell icons, context menus, persistence, tray and startup are implemented.
- Left/middle drag moves circles freely, snapping on release; centre click pages.
- Grid Off / 2x2 / 3x3 / 4x4 uses the current monitor's taskbar-free work area;
  guides mark the target cell and landing centre until release/cancel.
- D47 installed SHA256: 1D972121E8CCEE8792DB5FA01C67B1BDC9B16261031608A526385C9A7592A399.
  D44-D47 CI intentionally skipped; owner requests manual tests first. Installed: LOCALAPPDATA/Programs/DeskTuk/DeskTuk.exe.
  D47 config unchanged; installed dock is 153 px wide at system 96 DPI.
- Dock uses native icon-title text. Menu collapse is session-only; right-click/tray
  restore it. D45 eases transitions and retains its top edge within the work area.
- Pointer entry opens immediately with a short eased reveal; closing takes 110 ms.
  Circle surfaces use pixel-coverage blending. No new dependencies or idle polling.
- Product proposals and priorities: ../reviews/2026-10-18-product-review.md and D30 notes.
- D32 repairs item press cancellation on denied capture, outside-ring release/dismissal
  and overlap-covered drop targets. Isolated cross-circle/group/Back/Recent/cancel tests
  pass; referenced files remain unchanged. D33 grid/cleanup optimization passes local checks.
- D33 uploads grid lines once and moves a reusable target-cell window. Circle/Desktop
  refresh, metadata repaint and memory cleanup defer until interaction ends. Native
  geometry/reuse tests, held-refresh regression and explicit 500-cell/4K probe pass.
- D34 adds left dragging for the collapsed square/expanded DeskTuk heading. The dock
  follows the current monitor's reserved taskbar edge and persists dock_pos=x|y.
  Four-edge/negative-coordinate tests and native drag/restart/cancel checks pass locally.
- D35 shares Segoe UI and slate/periwinkle/mint styling across dock/circles/rings;
  original logo/card and icon rebuild instructions are in res/brand/README.md.
- D37 adds fixed-slot reference reordering in circles/groups and Name/Type/Added sorts.
- D41 adds transient typing search over the hovered ring (including grouped files):
  prefix matches first, stable existing order; Backspace/Escape, reset on close.
  MSVC checks pass (72 tests); physical keyboard/IME acceptance is unperformed.
- D42 shares global/circle actions in the dock menu; locked circles have a static
  gradient rim/small lock, and Recent has the standard distinct centre/outer colors.
  Shifted ring dragging preserves its visible origin and uses coalesced samples.
  MSVC 73 tests and Grid=3/topmost native fixture pass; physical feel is unverified.
- D43 uses linear search ranks, a transient source snapshot, page icon reuse and
  one coalescing shell worker which exits when idle. Added sort caches path ranks.
  76 MSVC tests and both native fixtures pass. Idle medians: 0 ms CPU/3s, 32 DLLs;
  commit +4/+32 KB vs D40 (Recent/300 refs). Config preserved; installer rebuilt.
- D44 aligns numeric labels at 100%, highlights search in fixed bands, and centres
  a grey lock above the count. Owner defers tests/CI until manual assessment.
- D45 adds left lock/right menu in both modes, rounded hover, 8-DIP bottom padding
  and transient eased menu/lock/hover transitions. Only fmt/Clippy/build performed;
  user expressly defers tests/CI. Manual acceptance and later fixture updates pending.
- D46 stabilizes transition frames/timer; static gradient rim and immediate lock.
- D47 keeps centre rims/text aligned, separates compact controls (64x92 DIP), adds
  left centre/background drag and lazy 32-file dock launch pages. Manual checks pending.
## Observed defects / checks not performed
- Installer: dist/DeskTuk-0.1.0-Setup-x64.exe; wizard/install checks passed in D40.
  Shell links migrated; physical taskbar cache refresh is unverified. See installer/README.md.
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
- Active plan: 2026-10-13-phase2-d-circles.md; D44-D47 installed for manual acceptance.
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
