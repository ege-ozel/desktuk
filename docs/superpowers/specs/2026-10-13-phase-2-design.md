# DeskTuk Phase 2 - design / decomposition

Status: **draft, written without a stated product purpose** (the owner skipped the
"what is DeskTuk for?" question). Ordering below is chosen to be useful under every
reading; see "Open question".

## Baseline (what exists, measured on Windows 11 25H2)

* One Node window, GDI-painted, items loaded synchronously at start-up.
* Hosts: `window` (default, 10/10 on the integration test), `owned` (10/10, experimental),
  `child` (7/10, **not viable** - Explorer's icon layer paints over it).
* ~4.7 MB private commit, ~46 modules after removing icon-overlay handler loading.
* Known gaps (from `docs/PHASE-0-1-REPORT.md` section 6): no add/remove, no drag-drop,
  no shell context menu, no tray icon, no autostart, names/icons read once, Explorer
  restart recovery **not yet verified on real Windows**.

## Decomposition (each is an independent plan that ships working software)

| Plan | Goal | Depends on |
|------|------|------------|
| **A - item layer** | Items load off the UI thread, stay in sync with the file system, and have the real shell context menu | - |
| **B - node management** | Add/remove items by drag-and-drop and menu, tray icon, start with Windows | A (for stable item model) |
| **C - desktop mirror** | "DeskTuk manages the desktop" mode: all Desktop items shown, scrolling for large counts, guidance to hide Explorer's icons | A, B |

Deliberately **out of scope** (YAGNI until A-C are proven): multiple Nodes / groups,
folder-backed "portal" Nodes, theming, search.

## Open question

What is DeskTuk for? (1) a Fences-like desktop organiser -> C is central; (2) a small
launcher panel -> B is central, C optional; (3) unknown -> A first. The plans keep A -> B -> C
because A is needed in every case.

## Cross-cutting rules

* **Never** modify, move, rename, delete or change attributes of user files.
* **RefCell discipline:** never keep `app().items.borrow()` / `borrow_mut()` alive across
  anything that can pump messages (`ShellExecuteExW`, `TrackPopupMenu`, `SetWindowPos`,
  `DestroyWindow`). A `Ref` in a `match`/`if let` scrutinee lives for the whole block.
  This caused the `0xC0000409` crash fixed in commit `d1b5d4b`. Read counts/paths into locals first.
* Host-specific z-order logic stays inside `src/host/*`.
* Keep the idle footprint: no new always-loaded DLL families (watch the module count with
  `scripts/memmap.ps1` if present, otherwise `Process.Modules`). Do not request
  `SHGFI_ICON` / `SHGFI_OVERLAYINDEX` (loads every overlay handler).
