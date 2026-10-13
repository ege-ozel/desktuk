> **Status (2026-10-13): on hold.** The owner chose circles (Plan D) instead of mirroring the
> whole Desktop. Do not execute.

# Phase 2 / Plan C - Desktop mirror ("DeskTuk manages the desktop")

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax. Read `AGENTS.md` first.

**Goal:** An opt-in mode where the Node shows **every** Desktop item (user + public Desktop folders), stays live as files appear/disappear, scrolls when there are many, and tells the owner how to hide Explorer's own icons so nothing is shown twice.

**Architecture:** `mirror=desktop` in the config switches the item source from the saved list to a directory listing. Plan A's change notifications (`watch.rs`) keep it live (`Created`/`Removed` under the Desktop dirs add/remove items). Layout gains a pure "fit to work area + scroll offset" function. Hiding Explorer's icons is **guided**, not automated, until a spike proves a programmatic toggle refreshes live.

**Tech Stack:** Rust, `windows-sys 0.61` (already enabled features cover everything here).

**Spec:** `docs/superpowers/specs/2026-10-13-phase-2-design.md`, report section 4.2.
**Depends on:** Plan A (watch + loader) and Plan B (menu, config round-trip).

## Global Constraints

* Never set `FILE_ATTRIBUTE_HIDDEN`, move, rename or delete Desktop files. Mirror mode is read-only on the file system.
* `desktop.ini` and `Thumbs.db` are never shown.
* Mirror mode is **off by default**; `host`, `x`, `y`, saved `item=` lines keep working unchanged when it is off.
* Pre-commit: `cargo fmt`, `cargo clippy --release --locked -- -D warnings`, `cargo test --release --locked`.

## Review Focus

* A Desktop with 0 items, with 1 item, and with 2 000 items: Node must stay within the work area and remain scrollable; no per-item allocation storms on paint.
* Same file name in user Desktop and Public Desktop: both shown (different paths), not merged.
* File created while the app runs (e.g. a download finishes): appears without restart.
* Mirror mode on while Explorer's icons are still visible: Node says so, nothing else breaks.
* Saved config contains `mirror=desktop` AND `item=` lines: mirror wins, the `item=` lines are preserved on save so turning mirror off restores the old list.
* Item count changes while scrolled to the bottom: scroll offset is clamped, never negative.

## File structure

| File | Responsibility |
|------|----------------|
| `src/config.rs` (modify) | `mirror: bool` field, `mirror=desktop` key |
| `src/items.rs` (modify) | `desktop_items()` unbounded listing, `is_under_desktop` |
| `src/node.rs` (modify) | scroll offset, wheel handling, notice strip |
| `src/layout.rs` (create) | pure `fit_layout` + `clamp_scroll` (moved from `node::layout` callers incrementally) |
| `src/app.rs` (modify) | choose item source; apply Created/Removed under Desktop |

---

### Task C1: `mirror=desktop` item source

**Files:**
- Modify: `src/config.rs`, `src/items.rs`, `src/app.rs`

**Interfaces:**
- Produces: `Config.mirror: bool` (default `false`); `Args.mirror: Option<bool>` via `--mirror` / `--no-mirror`; `items::desktop_items() -> Vec<PathBuf>`.

- [ ] **Step 1: Failing tests** (`src/config.rs` tests)

```rust
#[test]
fn mirror_key_round_trips_and_defaults_off() {
    let c = parse_config("host=window\nmirror=desktop\nitem=C:\\a.txt\n");
    assert!(c.mirror);
    assert_eq!(c.items.len(), 1);
    assert!(!parse_config("host=window\n").mirror);
    assert!(render_config(&c).contains("mirror=desktop"));
    assert!(render_config(&c).contains("item=C:\\a.txt"), "saved list survives mirror mode");
}
```

- [ ] **Step 2: Run** `cargo test --release --locked config::tests::mirror` -> FAIL.

- [ ] **Step 3: Implement.** Refactor `config::load`/`save` into pure `parse_config(text: &str) -> Config` and `render_config(c: &Config) -> String` (the bodies already exist; `load`/`save` become file I/O around them). Add `pub mirror: bool` to `Config` and `mirror=desktop` parse/render (`mirror=off` or absent -> `false`). In `items.rs`:

```rust
/// Every entry of the user's and the public Desktop folders, no cap, `desktop.ini`/`Thumbs.db` excluded.
pub fn desktop_items() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for id in [&FOLDERID_Desktop, &FOLDERID_PublicDesktop] {
        if let Some(dir) = known_folder(id) {
            out.extend(listing(&dir).into_iter().filter(|p| {
                !p.file_name().map(|n| n.eq_ignore_ascii_case("thumbs.db")).unwrap_or(false)
            }));
        }
    }
    out
}
```
(`listing` already drops `desktop.ini` and sorts by name.) In `app::run()`: when `saved.mirror || args.mirror == Some(true)` use `items::desktop_items()` as `paths`, keep `Config.items` (the saved list) separate in `App` (new field `saved_items: RefCell<Vec<PathBuf>>`) so `save_config` writes the saved list, not the mirrored one.

- [ ] **Step 4: Run** tests -> PASS. Manual: `DeskTuk.exe --mirror --no-save` shows every Desktop entry.

- [ ] **Step 5: Commit** (`Task C1`)

---

### Task C2: Live updates under the Desktop folders

**Files:**
- Modify: `src/items.rs`, `src/app.rs`

**Interfaces:**
- Consumes: `watch::Change` (Plan A), `items::spawn_loader`.
- Produces: `pub fn under_desktop(path: &Path, desktops: &[PathBuf]) -> bool` (direct children only), `pub fn mirror_apply(items: &mut Vec<PathBuf>, change: &Change, desktops: &[PathBuf]) -> (Vec<PathBuf> /*added*/, bool /*removed any*/)`.

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn only_direct_children_of_a_desktop_dir_count() {
    let d = vec![PathBuf::from(r"C:\Users\x\Desktop")];
    assert!(under_desktop(Path::new(r"c:\users\x\desktop\a.txt"), &d));
    assert!(!under_desktop(Path::new(r"C:\Users\x\Desktop\sub\a.txt"), &d));
    assert!(!under_desktop(Path::new(r"C:\Users\x\Documents\a.txt"), &d));
}

#[test]
fn mirror_adds_created_and_drops_removed() {
    let d = vec![PathBuf::from(r"C:\D")];
    let mut v = vec![PathBuf::from(r"C:\D\old.txt")];
    let (added, _) = mirror_apply(&mut v, &Change::Created(PathBuf::from(r"C:\D\new.txt")), &d);
    assert_eq!(added, vec![PathBuf::from(r"C:\D\new.txt")]);
    let (_, removed) = mirror_apply(&mut v, &Change::Removed(PathBuf::from(r"C:\D\old.txt")), &d);
    assert!(removed);
    assert_eq!(v, vec![PathBuf::from(r"C:\D\new.txt")]);
    let (a2, _) = mirror_apply(&mut v, &Change::Created(PathBuf::from(r"C:\D\NEW.TXT")), &d);
    assert!(a2.is_empty(), "case-insensitive duplicate ignored");
}
```

- [ ] **Step 2: Run** -> FAIL.

- [ ] **Step 3: Implement** `under_desktop` (compare `path.parent()` with each desktop dir via `eq_ignore_ascii_case`), `mirror_apply` (Created under desktop and not present -> push + report; Removed under desktop -> remove entry; Renamed within desktop = Removed(from)+Created(to); ignore `desktop.ini`/`thumbs.db`). In `controller_proc`'s `WM_APP_SHELL_CHANGE` handler, when mirror mode is on call `mirror_apply` **instead of** Plan A's `apply_change`, then rebuild `a.items` entries for added paths (`Item::placeholder`), `reposition(node)` and `spawn_loader` for the added paths. Do not call `save_config` for mirror changes.

- [ ] **Step 4: Run** tests -> PASS. Manual: create / rename / delete a file on the Desktop while running; Node follows within ~1 s.

- [ ] **Step 5: Commit** (`Task C2`)

---

### Task C3: Fit to the work area and scroll

**Files:**
- Create: `src/layout.rs`
- Modify: `src/node.rs`, `src/main.rs`

**Interfaces:**
- Produces:
  `pub struct Fit { pub cols: i32, pub visible_rows: i32, pub total_rows: i32 }`
  `pub fn fit(count: usize, cell_w: i32, cell_h: i32, chrome_h: i32, chrome_w: i32, max_w: i32, max_h: i32, max_cols: usize) -> Fit`
  `pub fn clamp_scroll(offset_rows: i32, f: &Fit) -> i32`

- [ ] **Step 1: Failing tests** (`src/layout.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn few_items_need_no_scrolling() {
        let f = fit(5, 88, 92, 80, 16, 1200, 900, 4);
        assert_eq!((f.cols, f.total_rows, f.visible_rows), (4, 2, 2));
    }

    #[test]
    fn many_items_are_capped_to_the_work_area() {
        let f = fit(2000, 88, 92, 80, 16, 1200, 900, 8);
        assert_eq!(f.cols, 8);
        assert_eq!(f.total_rows, 250);
        assert_eq!(f.visible_rows, (900 - 80) / 92); // 8 rows
    }

    #[test]
    fn scroll_offset_is_clamped() {
        let f = fit(20, 88, 92, 80, 16, 1200, 900, 4); // 5 rows, all visible
        assert_eq!(clamp_scroll(3, &f), 0);
        let g = fit(2000, 88, 92, 80, 16, 1200, 900, 8);
        assert_eq!(clamp_scroll(-5, &g), 0);
        assert_eq!(clamp_scroll(10_000, &g), g.total_rows - g.visible_rows);
    }

    #[test]
    fn empty_and_tiny_work_areas_still_show_one_cell() {
        let f = fit(0, 88, 92, 80, 16, 100, 50, 4);
        assert!(f.cols >= 1 && f.visible_rows >= 1);
    }
}
```

- [ ] **Step 2: Run** `cargo test --release --locked layout::tests` -> FAIL.

- [ ] **Step 3: Implement**

```rust
pub struct Fit { pub cols: i32, pub visible_rows: i32, pub total_rows: i32 }

pub fn fit(count: usize, cell_w: i32, cell_h: i32, chrome_h: i32, chrome_w: i32, max_w: i32, max_h: i32, max_cols: usize) -> Fit {
    let n = count.max(1);
    let by_width = ((max_w - chrome_w) / cell_w.max(1)).max(1) as usize;
    let cols = n.min(max_cols).min(by_width).max(1) as i32;
    let total_rows = (n as i32 + cols - 1) / cols;
    let by_height = ((max_h - chrome_h) / cell_h.max(1)).max(1);
    Fit { cols, visible_rows: total_rows.min(by_height), total_rows }
}

pub fn clamp_scroll(offset_rows: i32, f: &Fit) -> i32 {
    offset_rows.clamp(0, (f.total_rows - f.visible_rows).max(0))
}
```

In `node.rs`: add `scroll: Cell<i32>` to `App` (rows), compute the layout with `layout::fit` using the monitor work area from `reposition` (`mi.rcWork`; take `max_cols = 8` in mirror mode, `MAX_COLS` otherwise), draw only items whose row is in `scroll..scroll+visible_rows` (adjust `item_rect`/`hit_test` to subtract `scroll * cell_h`), handle `WM_MOUSEWHEEL` (`delta/120` rows, then `clamp_scroll`, `InvalidateRect`). `WM_MOUSEWHEEL` goes to the window under the cursor via the focus window; because the node never takes focus, route it from the `WM_MOUSEMOVE`-tracked hover state **or** register a low-level wheel handler only while the cursor is over the node; simplest verified approach: `TrackMouseEvent` already runs, so forward wheel via `SetFocus`-free `WM_MOUSEWHEEL` -> test on real Windows and record which approach works in the commit body.

- [ ] **Step 4: Run** tests -> PASS. Manual with 200+ Desktop items (create with PowerShell in a throw-away folder via `--mirror` pointing at it is **not** supported; use the real Desktop on a test machine): Node never exceeds the work area, wheel scrolls, no flicker.

- [ ] **Step 5: Commit** (`Task C3`)

---

### Task C4: Guidance when Explorer's icons are still visible

**Files:**
- Modify: `src/node.rs`, `src/app.rs`

**Interfaces:**
- Consumes: `shell::desktop_icons_hidden() -> bool` (exists).
- Produces: `pub fn notice(mirror: bool, icons_hidden: bool) -> Option<&'static str>` in `node.rs`.

- [ ] **Step 1: Failing test**

```rust
#[test]
fn notice_only_when_mirroring_with_visible_desktop_icons() {
    assert!(notice(true, false).is_some());
    assert!(notice(true, true).is_none());
    assert!(notice(false, false).is_none());
}
```

- [ ] **Step 2: Run** -> FAIL.

- [ ] **Step 3: Implement.** `notice` returns `Some("Desktop icons are still shown. Right-click the desktop > View > Show desktop icons to hide them.")` in the first case. Draw it as a one-line subtext strip under the header (`SUBTEXT` colour, `DT_END_ELLIPSIS`), re-evaluated in `WM_SETTINGCHANGE` (Explorer broadcasts it when the setting flips) - the existing controller handler already calls `reposition`; extend it to call `InvalidateRect` on the node.

- [ ] **Step 4: Run** tests -> PASS. Manual: toggle *Show desktop icons* in Explorer; the strip appears/disappears without restart. If `WM_SETTINGCHANGE` does not fire for this setting, poll `shell::desktop_icons_hidden()` on the existing 2 s health timer and record that in the commit body.

- [ ] **Step 5: Commit** (`Task C4`)

---

### Task C5 (spike, report only): can DeskTuk toggle Explorer's icons itself?

**Output is a short section appended to `docs/PHASE-0-1-REPORT.md` section 4.2, no product code kept.**

- [ ] **Step 1:** On a test machine, call `SHGetSetSettings(&mut ss, SSF_HIDEICONS, 1)` with `fHideIcons` set, from a scratch binary (`examples/` or a `#[test] #[ignore]`), and observe whether the desktop icons disappear immediately.
- [ ] **Step 2:** If not immediate, try the documented refresh (`SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, null, null)`) and a `WM_COMMAND`-free approach only. Do **not** send undocumented messages to `SysListView32` / `Progman` (rejected in the report).
- [ ] **Step 3:** Record: works / needs restart of Explorer / does not work, Windows build, and whether the user-visible setting stays in sync with `HideIcons` in the registry.
- [ ] **Step 4:** Commit the report section; delete the scratch code.

---

## Self-review

* Coverage of report 4.2: mirror source (C1), live (C2), scalable (C3), guidance (C4), toggle feasibility (C5).
* Names: `mirror_apply` consumes `watch::Change` from Plan A; `notice` consumes existing `shell::desktop_icons_hidden`.
* The "Unsorted Node" from the report needs multiple Nodes and is deliberately deferred; in this plan the single Node *is* "everything on the Desktop".
* Review Focus: 0/1/2000 items (C3 tests), duplicate names (C1 uses full paths), live create (C2), icons visible (C4), saved list preserved (C1 test), scroll clamp (C3 test).
