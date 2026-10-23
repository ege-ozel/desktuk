# Repository context
Keep this guide under 100 lines. Live Git/CI outrank snapshots.
## Start here
- Inspect `git status --short --branch` and `git worktree list`.
- Main is the integration baseline; owner permits direct main commits/pushes.
  Do not open/reopen PRs unless requested. Preserve task-by-task commits/checks.
- A and D through D60 implemented; README native renders fade into the viewer at edges.
  D56 scoped D55 review found no blocker; final automated/install checks pending.
- Owner requires circles and open rings always topmost, including Show Desktop.
## Delivered behavior and latest changes
- Desktop file references are grouped in circles; Recent, hover rings, paging,
  native shell icons, context menus, persistence, tray and startup are implemented.
- Left/middle drag moves circles freely, snapping on release; centre click pages.
- Grid Off / 2x2 / 3x3 / 4x4 / 6x6 / 8x8 uses the current monitor's taskbar-free work area;
  guides mark the target cell and landing centre until release/cancel.
- D57 installed SHA256: E9F61DF6B2158EB6711435AC5421760D10351712405DEB3C953AB98C2620833C.
  D44-D57 CI intentionally skipped; owner requests manual tests first. Installed: LOCALAPPDATA/Programs/DeskTuk/DeskTuk.exe.
  D57 config unchanged; installed dock is 168 px wide at system 96 DPI.
- Dock uses native icon-title text. Menu collapse is session-only; right-click/tray
  restore it. D50 changes modes immediately, retaining the top within the work area.
- Pointer entry opens immediately with a short eased reveal; closing takes 110 ms.
  Circle surfaces use pixel-coverage blending. No new dependencies or idle polling.
- Release review: ../reviews/2026-10-23-release-readiness.md (D55 candidate, unsigned).
- D32/D33 harden item capture/drop cancellation, defer refresh while interacting,
  and reuse grid guide windows; prior native fixtures pass. See plan evidence.
- D34 adds left dragging for the collapsed square/expanded DeskTuk heading. The dock
  initially used the taskbar rail; D51 saves any edge with dock_pos/dock_edge.
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
- D45/D46 establish left lock/right menu, hover easing, padding and gradient rims.
- D47 keeps centre rims/text aligned, separates compact controls (64x92 DIP), adds
  left centre/background drag and lazy 32-file dock launch pages. Manual checks pending.
- D48 adds bilingual settings/guide, lock hints and Align now. Grid selection
  preserves positions; first installs use 4x4. Guide: docs/user-guide.md.
- D49 fixes lazy menu notifications/hint placement and caches antialiased tools.
- D50 guards guide re-entry and removes resize easing; hover effects remain.
- D51 makes rows launch menus, keeps compact Circles, cleans borders/insets and
  enables blank-area dragging on all four rails with saved edge/cancel restoration.
- D52 adds positive hints and dock search; D54 uses one virtual scrollbar, locked
  search, rounded tool/field borders and blue drag hints with larger semibold text.
  D55 adds single AA frames, visible hover editing, result keys and teal hover.
  D57 reuses hover styling for initial/key selection; D58 shares native paint for docs.
  Native text selection/Ctrl+A; D53 fixes the omitted
  popup class name that prevented both lists and edit boxes opening.
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
| Settings, language, lock tips | src/settings.rs, i18n.rs, hint.rs |
| Bootstrap/message loop | src/app.rs |
| Isolated runtime fixtures | scripts/*regression-test.ps1 |
## Bounded reading
- Active plan: 2026-10-13-phase2-d-circles.md; D57 manual-first delivery.
- Required phase design: ../specs/2026-10-13-phase-2-design.md.
- Product decisions: ../specs/2026-10-13-circles-design.md.
- Search src/ first, then scripts/ and the active plan. Use `rg -n` / `rg --files`;
  fallback: `git grep -n`. .rgignore excludes generated/history files by default.
- Bounded PowerShell reads: `Get-Content <file> | Select-Object -Skip <start> -First <count>`.
- Root config.rs and .capture-* are ignored scratch; src/config.rs is authoritative.
  target/, dist/, captures, logs, executables and bundles are generated/local artifacts.
- AGENTS.md governs safety, branches, commits and required Windows MSVC checks.
  Keep evidence in the plan; update changed facts here in the same commit.
