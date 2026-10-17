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
