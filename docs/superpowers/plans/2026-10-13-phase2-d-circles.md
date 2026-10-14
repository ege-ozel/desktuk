# Phase 2 / Plan D - Circles

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Read `AGENTS.md` first.

**Goal:** Replace the single panel with round "circles" on the desktop that open into a ring of their Desktop files on hover, plus a built-in Recent circle, while idle RAM/CPU stay at or below v1.

**Architecture:** Pure, unit-tested modules (`circles::model`, `geom`, `view`, `suggest`, `desktop::recent`) hold all logic. A thin Win32 layer draws each circle as a per-pixel-alpha layered window (painted once, surface freed), opens one ring window owned by the hovered circle, and wakes the existing message loop on Desktop folder changes via `MsgWaitForMultipleObjectsEx`. The Phase 1 panel stays available (`--ui panel`, and always for `--selftest`).

**Tech Stack:** Rust 2021, `windows-sys 0.61` (features already enabled), Plan A's `items`, `watch` and `shell_menu` modules. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-10-13-circles-design.md` (read it first; it records the owner's decisions and the resource gate).

**Branch:** `phase2/d-circles`, created from `main` **after PR #1 (Plan A) is merged** - this plan uses `items::spawn_loader`, `items::Meta`, `watch::{Change, apply_change, decode}` and `shell_menu::{show, Choice}` from Plan A.

**About the code in this plan:** every code block was compiled with `cargo clippy --release --locked --all-targets -- -D warnings` and its tests were run on Windows 11 (MSVC) before the plan was written. Copy it as given. If something still does not compile on your machine, fix the smallest thing and say so in the commit body (AGENTS.md).

## Global Constraints

* Never create, move, rename, delete or change attributes of user files. Items are references (paths) only. Desktop listing is read-only.
* RefCell rule: never hold a borrow of `app().board`, `app().items` or circles state across a menu (`TrackPopupMenu`, `shell_menu::show`), `ShellExecuteExW`/`items::open`, `DestroyWindow`, `MessageBoxW` or the rename box. Circles code uses `try_borrow*` and returns early on failure.
* `panic = "abort"`: no `unwrap()`/`expect()` on Win32 or shell results outside `#[cfg(test)]`.
* No `SHGFI_ICON` / `SHGFI_OVERLAYINDEX`. Icons only for the visible ring page (at most 20), released when the ring closes.
* No timer may run while nothing animates (idle CPU must be 0). No new threads besides Plan A's short-lived loader.
* Do not run `scripts/windows-integration-test.ps1` locally (it moves the mouse and presses Win+D); CI runs it.
* Pre-commit, from the repo root: `cargo fmt`, `cargo clippy --release --locked -- -D warnings`, `cargo test --release --locked`.
* Commit message last line: `Plan: docs/superpowers/plans/2026-10-13-phase2-d-circles.md Task D<n>`. Mark the task's steps `- [x]` in this file in the same commit. Push after every task.

## Review Focus

* **The owner's existing Phase 1 config** (no `ui=` line, `item=` lines): DeskTuk now starts in the circles UI, the `item=` lines survive every save, and `--ui panel` brings the panel back. (Config tests in D1 pin the file side; check the start-up by hand in D5.)
* **A Desktop with hundreds of files**: Recent shows 20 at a time, the wheel and the centre click page through them, and never more than 20 icons are extracted (geom paging tests in D2; manual check in D5).
* **Menus and the rename box while the ring is open**: the ring must not close under an open menu and nothing may crash when the pointer leaves meanwhile (`with_menu` guard in D5; manual check).
* **A circle deleted or added while a ring or rename box is open**: indices shift; `rebuild()` closes the ring and cancels the rename box first (D5; manual check: rename, then delete another circle).
* **Desktop files created, renamed or deleted while DeskTuk runs**: Recent updates within about 0.3 s; stored references follow shell renames; a deleted file stays referenced and is drawn dimmed (model `follow` test in D1; manual check in D5).

## File structure

| File | Responsibility |
|------|----------------|
| `src/config.rs` (modify, D1) | pure `parse_config` / `render_config`, `Ui`, `--ui`, board lines delegated to the model |
| `src/circles/mod.rs` (create D1, grow D2-D4, final D5) | runtime state and commands (assign, rename, delete, menus, Desktop refresh) |
| `src/circles/model.rs` (D1) | `Board`, `Circle`, `Group`, `Place`; names; config lines |
| `src/circles/geom.rs` (D2) | metrics, slot positions, hit testing, paging, disc mask, animation curve |
| `src/circles/desktop.rs` (D3) | Desktop listing, `recent()`, folder change `Watcher` |
| `src/circles/suggest.rs` (D3) | extension-based rule suggestions |
| `src/circles/view.rs` (D3) | `Target`, `SlotRef`: what a ring shows; drop targets |
| `src/circles/surface.rs` (D4) | 32-bit DIB + `UpdateLayeredWindow` |
| `src/circles/draw.rs` (D4) | GDI drawing of a circle and a ring; ignored preview test |
| `src/circles/circle_win.rs` (D5) | closed circle windows: hover, click, move, drop, menu |
| `src/circles/ring.rs` (D5) | the open ring window: animation, paging, open, menus, drag between circles |
| `src/circles/rename.rs` (D5) | inline rename popup |
| `src/app.rs` (modify D1, D5) | UI selection, board state, save, quit, message loop, controller messages |
| `src/main.rs` (modify D1) | `mod circles;` |
| `scripts/bench-memory.ps1` (create D6) | v1 vs v2 memory/CPU benchmark |
| `scripts/windows-integration-test.ps1` (modify D6) | perf runs for both UIs |
| `README.md` (modify D6) | circles usage |

---

### Task D1: Config split and the circle board model

**Files:**
- Modify: `src/config.rs` (whole file below), `src/main.rs`, `src/app.rs` (one literal)
- Create: `src/circles/mod.rs`, `src/circles/model.rs`

**Interfaces:**
- Produces (`circles::model`): `pub struct Board { pub circles: Vec<Circle>, pub recent_pos: Option<(i32, i32)> }` (`Default`, `Clone`, `PartialEq`), `pub struct Circle { pub name: String, pub pos: (i32, i32), pub items: Vec<PathBuf>, pub groups: Vec<Group> }`, `pub struct Group { pub name: String, pub items: Vec<PathBuf> }`, `pub enum Place { Circle(usize), Group(usize, usize) }` (`Copy`), `pub const MAX_NAME: usize = 40`, `pub fn clean_name(&str) -> String`, and on `Board`: `find_circle`, `unique_circle_name`, `add_circle(&str, (i32, i32)) -> usize`, `rename_circle(usize, &str)`, `remove_circle(usize)`, `add_group(usize, &str) -> Option<usize>`, `rename_group(usize, usize, &str)`, `remove_group(usize, usize)`, `place_of(&Path) -> Option<Place>`, `is_assigned(&Path) -> bool`, `unassign(&Path) -> bool`, `assign(&Path, Place) -> bool`, `follow(&watch::Change) -> bool`, `parse_line(&str, &str) -> bool`, `render() -> String`.
- Produces (`config`): `pub enum Ui { Panel, Circles }` with `parse`/`name`; `Config { host, pos, items, ui: Option<Ui>, board: Board }`; `Args.ui: Option<Ui>`; `pub fn parse_config(&str) -> Config`; `pub fn render_config(&Config) -> String`.
- Consumes: `watch::{Change, apply_change}` (Plan A).

- [x] **Step 1: Declare the module and write the failing model tests.**

In `src/main.rs` add `mod circles;` directly after `mod app;`.

Create `src/circles/mod.rs`:

```rust
//! Phase 2 circles UI. Task D5 replaces this file with the full runtime.
#![allow(dead_code)] // Removed in Task D5, once every item is used.

pub mod model;
```

Create `src/circles/model.rs` containing only this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    fn parse(text: &str) -> Board {
        let mut b = Board::default();
        for line in text.lines() {
            if let Some((k, v)) = line.split_once('=') {
                b.parse_line(k.trim(), v.trim());
            }
        }
        b
    }

    #[test]
    fn names_are_cleaned_and_made_unique() {
        assert_eq!(clean_name("  Mu|sic \n"), "Mu/sic");
        assert_eq!(clean_name("   "), "Untitled");
        assert_eq!(clean_name(&"x".repeat(100)).chars().count(), MAX_NAME);
        let mut b = Board::default();
        assert_eq!(b.add_circle("Music", (0, 0)), 0);
        b.add_circle("music", (0, 0));
        b.add_circle("MUSIC", (0, 0));
        let names: Vec<&str> = b.circles.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Music", "music 2", "MUSIC 3"]);
        b.rename_circle(0, "Music");
        assert_eq!(b.circles[0].name, "Music", "renaming to its own name keeps it");
        b.rename_circle(2, "music 2");
        assert_eq!(b.circles[2].name, "music 2 2");
    }

    #[test]
    fn an_item_lives_in_exactly_one_place() {
        let mut b = Board::default();
        let m = b.add_circle("Music", (0, 0));
        let k = b.add_circle("Books", (0, 0));
        let g = b.add_group(m, "Titled").unwrap();
        let f = p(r"C:\Users\x\Desktop\song.wav");
        assert!(b.assign(&f, Place::Circle(k)));
        assert!(b.assign(&p(r"c:\users\x\desktop\SONG.wav"), Place::Group(m, g)));
        assert_eq!(b.place_of(&f), Some(Place::Group(m, g)));
        assert!(b.circles[k].items.is_empty(), "moved, not copied");
        assert!(!b.assign(&f, Place::Group(m, 7)), "invalid target refused");
        assert_eq!(b.place_of(&f), Some(Place::Group(m, g)), "...and nothing changed");
        assert!(b.unassign(&f));
        assert!(!b.is_assigned(&f));
        assert!(!b.unassign(&f));
    }

    #[test]
    fn removing_a_group_keeps_its_items_in_the_circle() {
        let mut b = Board::default();
        let m = b.add_circle("Music", (0, 0));
        let g = b.add_group(m, "Titled").unwrap();
        b.assign(&p(r"C:\a.wav"), Place::Group(m, g));
        b.remove_group(m, g);
        assert_eq!(b.place_of(&p(r"C:\a.wav")), Some(Place::Circle(m)));
        b.remove_circle(m);
        assert!(!b.is_assigned(&p(r"C:\a.wav")), "removing a circle unassigns, never deletes files");
    }

    #[test]
    fn group_names_are_unique_per_circle_only() {
        let mut b = Board::default();
        let a = b.add_circle("A", (0, 0));
        let c = b.add_circle("C", (0, 0));
        assert_eq!(b.add_group(a, "Old"), Some(0));
        assert_eq!(b.add_group(a, "old"), Some(1));
        assert_eq!(b.circles[a].groups[1].name, "old 2");
        b.add_group(c, "Old");
        assert_eq!(b.circles[c].groups[0].name, "Old");
        assert_eq!(b.add_group(9, "x"), None);
    }

    #[test]
    fn follows_renames_of_files_and_folders() {
        let mut b = Board::default();
        let m = b.add_circle("Music", (0, 0));
        b.assign(&p(r"C:\D\old.wav"), Place::Circle(m));
        assert!(b.follow(&Change::Renamed { from: p(r"c:\d\OLD.wav"), to: p(r"C:\D\new.wav") }));
        assert_eq!(b.place_of(&p(r"C:\D\new.wav")), Some(Place::Circle(m)));
        assert!(!b.follow(&Change::Removed(p(r"C:\D\new.wav"))), "a deleted file stays referenced");
        assert!(b.is_assigned(&p(r"C:\D\new.wav")));
    }

    #[test]
    fn config_lines_round_trip() {
        let mut b = Board::default();
        let m = b.add_circle("Music", (120, -40));
        let g = b.add_group(m, "Titled Project").unwrap();
        b.add_group(m, "Empty");
        b.assign(&p(r"C:\Users\x\Desktop\notes.txt"), Place::Circle(m));
        b.assign(&p(r"C:\Users\x\Desktop\a b\song.wav"), Place::Group(m, g));
        b.add_circle("Books", (5, 6));
        b.recent_pos = Some((1600, 80));
        let text = b.render();
        assert!(text.contains("circle=Music|120|-40\n"));
        assert!(text.contains("citem=Music||C:\\Users\\x\\Desktop\\notes.txt\n"));
        assert_eq!(parse(&text), b);
    }

    #[test]
    fn parsing_tolerates_hand_edited_files() {
        let b = parse("citem=Music|Live|C:\\x.wav\ncircle=Music|1|2\nbogus=1\ncitem=Music||\ncircle=|3|4\nrecent=garbage\n");
        assert_eq!(b.circles.len(), 2);
        assert_eq!(b.circles[0].name, "Music");
        assert_eq!(b.circles[0].pos, (1, 2), "a later circle= line sets the position");
        assert_eq!(b.circles[0].groups[0].name, "Live");
        assert_eq!(b.place_of(&p(r"C:\x.wav")), Some(Place::Group(0, 0)));
        assert_eq!(b.circles[1].name, "Untitled");
        assert_eq!(b.recent_pos, None);
        let mut b2 = Board::default();
        assert!(!b2.parse_line("item", r"C:\x"), "legacy keys are left to the caller");
    }
}
```

- [x] **Step 2: Run to verify failure.**

Run: `cargo test --release --locked circles::model`
Expected: FAIL to compile (`Board`, `Place`, `clean_name`, `MAX_NAME` not found).

- [x] **Step 3: Implement the model.** Insert this **above** the test module in `src/circles/model.rs`:

```rust
//! The circle board: pure data and operations, no Win32.
//!
//! A board is a list of user circles. A circle holds item paths directly and in groups
//! (at most one level of groups: circle > group > item). Items are *references*: nothing
//! here touches the file system. Every path lives in at most one place.

use std::path::{Path, PathBuf};

use crate::watch::{self, Change};

/// Longest circle/group name, in characters.
pub const MAX_NAME: usize = 40;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub name: String,
    pub items: Vec<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Circle {
    pub name: String,
    /// Top-left of the circle window in screen pixels.
    pub pos: (i32, i32),
    pub items: Vec<PathBuf>,
    pub groups: Vec<Group>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Board {
    pub circles: Vec<Circle>,
    /// Top-left of the built-in Recent circle; `None` = default position.
    pub recent_pos: Option<(i32, i32)>,
}

/// Where an item lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Circle(usize),
    Group(usize, usize),
}

fn same_path(a: &Path, b: &Path) -> bool {
    a.as_os_str().eq_ignore_ascii_case(b.as_os_str())
}

fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// A name that is safe to store: no `|` (the config separator), no control characters,
/// trimmed, at most `MAX_NAME` characters, never empty.
pub fn clean_name(s: &str) -> String {
    let t: String = s.chars().map(|c| if c == '|' { '/' } else { c }).filter(|c| !c.is_control()).collect();
    let t: String = t.trim().chars().take(MAX_NAME).collect();
    let t = t.trim().to_string();
    if t.is_empty() {
        "Untitled".to_string()
    } else {
        t
    }
}

/// `wanted` made unique against `taken` (case-insensitive): "Music", "Music 2", "Music 3", ...
fn unique_among(wanted: &str, taken: &[&str]) -> String {
    let base = clean_name(wanted);
    if !taken.iter().any(|t| same_name(t, &base)) {
        return base;
    }
    for n in 2.. {
        let suffix = format!(" {n}");
        let keep = MAX_NAME.saturating_sub(suffix.chars().count());
        let candidate = format!("{}{suffix}", base.chars().take(keep).collect::<String>().trim_end());
        if !taken.iter().any(|t| same_name(t, &candidate)) {
            return candidate;
        }
    }
    unreachable!()
}

fn remove_path(v: &mut Vec<PathBuf>, path: &Path) -> bool {
    let before = v.len();
    v.retain(|p| !same_path(p, path));
    v.len() != before
}

impl Board {
    pub fn find_circle(&self, name: &str) -> Option<usize> {
        self.circles.iter().position(|c| same_name(&c.name, name))
    }

    /// `wanted` made unique among circle names, ignoring circle `except` (the one being renamed).
    pub fn unique_circle_name(&self, wanted: &str, except: Option<usize>) -> String {
        let taken: Vec<&str> = self.circles.iter().enumerate().filter(|(i, _)| Some(*i) != except).map(|(_, c)| c.name.as_str()).collect();
        unique_among(wanted, &taken)
    }

    /// Appends a circle and returns its index. The name is cleaned and made unique.
    pub fn add_circle(&mut self, name: &str, pos: (i32, i32)) -> usize {
        let name = self.unique_circle_name(name, None);
        self.circles.push(Circle { name, pos, items: Vec::new(), groups: Vec::new() });
        self.circles.len() - 1
    }

    pub fn rename_circle(&mut self, ci: usize, name: &str) {
        if ci < self.circles.len() {
            self.circles[ci].name = self.unique_circle_name(name, Some(ci));
        }
    }

    /// Removes a circle. Its items become unassigned (they show up in Recent again).
    pub fn remove_circle(&mut self, ci: usize) {
        if ci < self.circles.len() {
            self.circles.remove(ci);
        }
    }

    /// Appends a group to circle `ci` and returns its index (`None` if `ci` is invalid).
    pub fn add_group(&mut self, ci: usize, name: &str) -> Option<usize> {
        let c = self.circles.get_mut(ci)?;
        let taken: Vec<&str> = c.groups.iter().map(|g| g.name.as_str()).collect();
        let name = unique_among(name, &taken);
        c.groups.push(Group { name, items: Vec::new() });
        Some(c.groups.len() - 1)
    }

    pub fn rename_group(&mut self, ci: usize, gi: usize, name: &str) {
        let Some(c) = self.circles.get_mut(ci) else { return };
        if gi >= c.groups.len() {
            return;
        }
        let taken: Vec<&str> = c.groups.iter().enumerate().filter(|(i, _)| *i != gi).map(|(_, g)| g.name.as_str()).collect();
        c.groups[gi].name = unique_among(name, &taken);
    }

    /// Removes a group; its items move up to the circle itself.
    pub fn remove_group(&mut self, ci: usize, gi: usize) {
        let Some(c) = self.circles.get_mut(ci) else { return };
        if gi < c.groups.len() {
            let g = c.groups.remove(gi);
            c.items.extend(g.items);
        }
    }

    pub fn place_of(&self, path: &Path) -> Option<Place> {
        for (ci, c) in self.circles.iter().enumerate() {
            if c.items.iter().any(|p| same_path(p, path)) {
                return Some(Place::Circle(ci));
            }
            for (gi, g) in c.groups.iter().enumerate() {
                if g.items.iter().any(|p| same_path(p, path)) {
                    return Some(Place::Group(ci, gi));
                }
            }
        }
        None
    }

    pub fn is_assigned(&self, path: &Path) -> bool {
        self.place_of(path).is_some()
    }

    /// Removes `path` from wherever it is. Returns whether it was assigned.
    pub fn unassign(&mut self, path: &Path) -> bool {
        let mut any = false;
        for c in &mut self.circles {
            any |= remove_path(&mut c.items, path);
            for g in &mut c.groups {
                any |= remove_path(&mut g.items, path);
            }
        }
        any
    }

    /// Puts `path` at `to`, removing it from its previous place first. Returns `false`
    /// (and changes nothing) when `to` does not exist.
    pub fn assign(&mut self, path: &Path, to: Place) -> bool {
        let valid = match to {
            Place::Circle(ci) => ci < self.circles.len(),
            Place::Group(ci, gi) => self.circles.get(ci).map(|c| gi < c.groups.len()).unwrap_or(false),
        };
        if !valid {
            return false;
        }
        self.unassign(path);
        match to {
            Place::Circle(ci) => self.circles[ci].items.push(path.to_path_buf()),
            Place::Group(ci, gi) => self.circles[ci].groups[gi].items.push(path.to_path_buf()),
        }
        true
    }

    /// Follows a rename/move reported by the shell. Returns whether any stored path changed.
    pub fn follow(&mut self, change: &Change) -> bool {
        if !matches!(change, Change::Renamed { .. }) {
            return false;
        }
        let mut changed = false;
        for c in &mut self.circles {
            changed |= !watch::apply_change(&mut c.items, change).is_empty();
            for g in &mut c.groups {
                changed |= !watch::apply_change(&mut g.items, change).is_empty();
            }
        }
        changed
    }

    /// Applies one `key=value` line of the config file. Returns `false` for keys that
    /// are not board keys (the caller handles those).
    ///
    /// ```text
    /// circle=Music|120|340
    /// group=Music|Titled Project
    /// citem=Music||C:\Users\me\Desktop\notes.txt
    /// citem=Music|Titled Project|C:\Users\me\Desktop\song.wav
    /// recent=1600|80
    /// ```
    pub fn parse_line(&mut self, key: &str, value: &str) -> bool {
        match key {
            "circle" => {
                let mut f = value.splitn(3, '|');
                let name = clean_name(f.next().unwrap_or(""));
                let x = f.next().and_then(|v| v.trim().parse().ok()).unwrap_or(0);
                let y = f.next().and_then(|v| v.trim().parse().ok()).unwrap_or(0);
                match self.find_circle(&name) {
                    Some(ci) => self.circles[ci].pos = (x, y),
                    None => self.circles.push(Circle { name, pos: (x, y), items: Vec::new(), groups: Vec::new() }),
                }
            }
            "group" => {
                let Some((c, g)) = value.split_once('|') else { return true };
                let ci = self.circle_or_create(c);
                let g = clean_name(g);
                if !self.circles[ci].groups.iter().any(|x| same_name(&x.name, &g)) {
                    self.circles[ci].groups.push(Group { name: g, items: Vec::new() });
                }
            }
            "citem" => {
                let mut f = value.splitn(3, '|');
                let (Some(c), Some(g), Some(p)) = (f.next(), f.next(), f.next()) else { return true };
                let p = p.trim();
                if p.is_empty() {
                    return true;
                }
                let ci = self.circle_or_create(c);
                let to = if g.trim().is_empty() {
                    Place::Circle(ci)
                } else {
                    let g = clean_name(g);
                    let gi = match self.circles[ci].groups.iter().position(|x| same_name(&x.name, &g)) {
                        Some(gi) => gi,
                        None => {
                            self.circles[ci].groups.push(Group { name: g, items: Vec::new() });
                            self.circles[ci].groups.len() - 1
                        }
                    };
                    Place::Group(ci, gi)
                };
                self.assign(Path::new(p), to);
            }
            "recent" => {
                let parsed = value.split_once('|').and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)));
                self.recent_pos = parsed;
            }
            _ => return false,
        }
        true
    }

    fn circle_or_create(&mut self, name: &str) -> usize {
        let name = clean_name(name);
        match self.find_circle(&name) {
            Some(ci) => ci,
            None => {
                self.circles.push(Circle { name, pos: (0, 0), items: Vec::new(), groups: Vec::new() });
                self.circles.len() - 1
            }
        }
    }

    /// The board as config lines (each ends with `\n`), in an order `parse_line` reads back.
    pub fn render(&self) -> String {
        let mut s = String::new();
        if let Some((x, y)) = self.recent_pos {
            s += &format!("recent={x}|{y}\n");
        }
        for c in &self.circles {
            s += &format!("circle={}|{}|{}\n", c.name, c.pos.0, c.pos.1);
            for g in &c.groups {
                s += &format!("group={}|{}\n", c.name, g.name);
            }
            for p in &c.items {
                s += &format!("citem={}||{}\n", c.name, p.display());
            }
            for g in &c.groups {
                for p in &g.items {
                    s += &format!("citem={}|{}|{}\n", c.name, g.name, p.display());
                }
            }
        }
        s
    }
}
```

- [x] **Step 4: Run the model tests.**

Run: `cargo test --release --locked circles::model`
Expected: PASS (7 tests).

- [x] **Step 5: Write the failing config tests.** Append inside `mod tests` of `src/config.rs`:

```rust
    #[test]
    fn config_round_trips_panel_and_circle_lines_together() {
        let text = "host=window\nui=circles\nx=10\ny=20\nitem=C:\\a.txt\ncircle=Music|1|2\ncitem=Music||C:\\b.wav\n";
        let c = parse_config(text);
        assert_eq!(c.ui, Some(Ui::Circles));
        assert_eq!(c.items, vec![PathBuf::from(r"C:\a.txt")]);
        assert_eq!(c.board.circles.len(), 1);
        assert_eq!(c.pos, Some((10, 20)));
        assert_eq!(parse_config(&render_config(&c)), c);
    }

    #[test]
    fn a_phase_one_file_has_no_ui_and_no_circles() {
        let c = parse_config("host=owned\nitem=C:\\a.txt\n");
        assert_eq!(c.ui, None);
        assert!(c.board.circles.is_empty());
        assert!(!render_config(&c).contains("ui="), "rendering does not invent a ui line");
        assert_eq!(Ui::parse("CIRCLES"), Some(Ui::Circles));
        assert_eq!(Ui::parse("rings"), None);
    }
```

Run: `cargo test --release --locked config::tests`
Expected: FAIL to compile (`parse_config`, `render_config`, `Ui` not found).

- [x] **Step 6: Implement the config split.** Replace `src/config.rs` with:

```rust
//! Command line and the persisted node configuration (%APPDATA%\DeskTuk\desktuk.cfg).
//!
//! The file is a plain `key=value` text file so users can inspect and edit it:
//!
//! ```text
//! host=window
//! x=120
//! y=120
//! item=C:\Users\me\Desktop\Notes.txt
//! item=C:\Users\me\Desktop\Visual Studio Code.lnk
//! ```

use std::path::PathBuf;

use crate::circles::model::Board;
use crate::host::HostKind;

/// Which user interface runs: the Phase 1 panel or the Phase 2 circles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ui {
    Panel,
    Circles,
}

impl Ui {
    pub fn parse(s: &str) -> Option<Ui> {
        match s.to_ascii_lowercase().as_str() {
            "panel" => Some(Ui::Panel),
            "circles" => Some(Ui::Circles),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Ui::Panel => "panel",
            Ui::Circles => "circles",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub host: HostKind,
    pub pos: Option<(i32, i32)>,
    /// Panel items (`item=` lines). Kept untouched while the circles UI runs.
    pub items: Vec<PathBuf>,
    /// None when the file has no ui= line (a Phase 1 file).
    pub ui: Option<Ui>,
    /// Circles (`circle=`, `group=`, `citem=`, `recent=` lines). Kept untouched while the panel runs.
    pub board: Board,
}

#[derive(Debug, Default)]
pub struct Args {
    pub host: Option<HostKind>,
    pub ui: Option<Ui>,
    pub pos: Option<(i32, i32)>,
    pub items: Vec<PathBuf>,
    pub log: Option<PathBuf>,
    pub selftest: bool,
    pub selftest_explorer: bool,
    pub shots: Option<PathBuf>,
    pub exit_after_ms: Option<u32>,
    pub no_save: bool,
    pub allow_multiple: bool,
    pub diag_only: bool,
    pub help: bool,
}

pub const USAGE: &str = "DeskTuk.exe [options] [item paths...]

  --host <window|owned|child>  Desktop hosting strategy (default: window).
                               owned/child are EXPERIMENTAL (undocumented Explorer windows).
  --ui <circles|panel>         User interface (default: circles; the self-test always uses panel).
  --pos <x,y>                  Node position in screen pixels.
  --log <file>                 Write a diagnostic log.
  --exit-after <ms>            Quit after the given time (measurements).
  --selftest                   Run the automated integration self-test, then exit.
  --selftest-explorer-restart  Self-test also kills and restarts explorer.exe (CI only!).
  --shots <dir>                Self-test saves screenshots (BMP) into <dir>.
  --diag                       Log desktop/shell diagnostics and exit.
  --no-save                    Do not write the configuration file.
  --allow-multiple             Skip the single-instance check.

Item paths given on the command line replace the configured items for this run.";

pub fn parse_args() -> Result<Args, String> {
    let mut a = Args::default();
    let mut it = std::env::args_os().skip(1);
    while let Some(arg) = it.next() {
        let s = arg.to_string_lossy().into_owned();
        let mut value = |name: &str| it.next().map(|v| v.to_string_lossy().into_owned()).ok_or(format!("{name} needs a value"));
        match s.as_str() {
            "--host" => {
                let v = value("--host")?;
                a.host = Some(HostKind::parse(&v).ok_or(format!("unknown host '{v}'"))?);
            }
            "--ui" => {
                let v = value("--ui")?;
                a.ui = Some(Ui::parse(&v).ok_or(format!("unknown ui '{v}'"))?);
            }
            "--pos" => {
                let v = value("--pos")?;
                a.pos = Some(parse_pos(&v).ok_or(format!("bad --pos '{v}'"))?);
            }
            "--log" => a.log = Some(PathBuf::from(value("--log")?)),
            "--shots" => a.shots = Some(PathBuf::from(value("--shots")?)),
            "--exit-after" => {
                let v = value("--exit-after")?;
                a.exit_after_ms = Some(v.parse().map_err(|_| format!("bad --exit-after '{v}'"))?);
            }
            "--selftest" => a.selftest = true,
            "--selftest-explorer-restart" => {
                a.selftest = true;
                a.selftest_explorer = true;
            }
            "--diag" => a.diag_only = true,
            "--no-save" => a.no_save = true,
            "--allow-multiple" => a.allow_multiple = true,
            "-h" | "--help" | "/?" => a.help = true,
            _ if s.starts_with("--") => return Err(format!("unknown option '{s}'")),
            _ => a.items.push(PathBuf::from(arg)),
        }
    }
    if a.selftest {
        a.no_save = true;
        a.allow_multiple = true;
    }
    Ok(a)
}

fn parse_pos(s: &str) -> Option<(i32, i32)> {
    let (x, y) = s.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

pub fn config_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("DeskTuk").join("desktuk.cfg"))
}

pub fn load() -> Option<Config> {
    let text = std::fs::read_to_string(config_path()?).ok()?;
    Some(parse_config(&text))
}

/// Parses the configuration text. Unknown keys and malformed lines are ignored.
pub fn parse_config(text: &str) -> Config {
    let mut c = Config { host: HostKind::Window, pos: None, items: Vec::new(), ui: None, board: Board::default() };
    let (mut x, mut y) = (None, None);
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let (k, v) = (k.trim(), v.trim());
        if c.board.parse_line(k, v) {
            continue;
        }
        match k {
            "host" => c.host = HostKind::parse(v).unwrap_or(HostKind::Window),
            "ui" => c.ui = Ui::parse(v),
            "x" => x = v.parse().ok(),
            "y" => y = v.parse().ok(),
            "item" if !v.is_empty() => c.items.push(PathBuf::from(v)),
            _ => {}
        }
    }
    if let (Some(x), Some(y)) = (x, y) {
        c.pos = Some((x, y));
    }
    c
}

pub fn render_config(c: &Config) -> String {
    let mut s = String::from("# DeskTuk configuration. Items are references; the files themselves are never moved or changed.\n");
    s += &format!("host={}\n", c.host.name());
    if let Some(ui) = c.ui {
        s += &format!("ui={}\n", ui.name());
    }
    if let Some((x, y)) = c.pos {
        s += &format!("x={x}\ny={y}\n");
    }
    for p in &c.items {
        s += &format!("item={}\n", p.display());
    }
    s += &c.board.render();
    s
}

pub fn save(c: &Config) -> std::io::Result<()> {
    let Some(path) = config_path() else { return Ok(()) };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Write-then-rename so a crash never leaves a truncated file.
    let tmp = path.with_extension("cfg.tmp");
    std::fs::write(&tmp, render_config(c))?;
    std::fs::rename(&tmp, &path)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_positions() {
        assert_eq!(parse_pos("10,20"), Some((10, 20)));
        assert_eq!(parse_pos(" -5 , 7 "), Some((-5, 7)));
        assert_eq!(parse_pos("10"), None);
        assert_eq!(parse_pos("a,b"), None);
    }

    #[test]
    fn host_names_round_trip() {
        for k in [HostKind::Window, HostKind::Owned, HostKind::Child] {
            assert_eq!(HostKind::parse(k.name()), Some(k));
        }
        assert_eq!(HostKind::parse("auto"), Some(HostKind::Window));
        assert_eq!(HostKind::parse("CHILD"), Some(HostKind::Child));
        assert_eq!(HostKind::parse("workerw"), None);
        assert!(!HostKind::Window.experimental());
        assert!(HostKind::Child.experimental());
    }

    #[test]
    fn config_round_trips_panel_and_circle_lines_together() {
        let text = "host=window\nui=circles\nx=10\ny=20\nitem=C:\\a.txt\ncircle=Music|1|2\ncitem=Music||C:\\b.wav\n";
        let c = parse_config(text);
        assert_eq!(c.ui, Some(Ui::Circles));
        assert_eq!(c.items, vec![PathBuf::from(r"C:\a.txt")]);
        assert_eq!(c.board.circles.len(), 1);
        assert_eq!(c.pos, Some((10, 20)));
        assert_eq!(parse_config(&render_config(&c)), c);
    }

    #[test]
    fn a_phase_one_file_has_no_ui_and_no_circles() {
        let c = parse_config("host=owned\nitem=C:\\a.txt\n");
        assert_eq!(c.ui, None);
        assert!(c.board.circles.is_empty());
        assert!(!render_config(&c).contains("ui="), "rendering does not invent a ui line");
        assert_eq!(Ui::parse("CIRCLES"), Some(Ui::Circles));
        assert_eq!(Ui::parse("rings"), None);
    }
}
```

`app.rs` builds a `Config` in `App::save_config`; until Task D5 wires the circles UI, give it the two new fields. In `src/app.rs`, `save_config`, change the literal to:

```rust
        // Task D5 replaces `ui` and `board` with the running UI and the real board.
        let c = Config {
            host: self.host().kind(),
            pos: Some(self.pos.get()),
            items: self.items.borrow().iter().map(|i| i.path.clone()).collect(),
            ui: None,
            board: Default::default(),
        };
```

(Nothing writes circle lines before D5, so this intermediate state cannot lose data.)

- [x] **Step 7: Run everything.**

Run: `cargo fmt`, `cargo clippy --release --locked -- -D warnings`, `cargo test --release --locked`
Expected: clippy clean; 19 tests PASS (Plan A's 10 + 7 model + 2 new config tests).

- [x] **Step 8: Commit** (`Add the circle board model and split config parsing`, last line `... Task D1`).

---

### Task D2: Ring geometry, paging, disc mask and animation curve

**Files:**
- Create: `src/circles/geom.rs`
- Modify: `src/circles/mod.rs` (add `pub mod geom;`)

**Interfaces:**
- Produces: `PER_PAGE = 20`, `INNER_SLOTS = 8`, `OUTER_SLOTS = 12`, `OPEN_MS = 160.0`, `CLOSE_MS = 110.0`; `pub struct Metrics { circle_d, ring_r, slot, icon, inner_r, outer_r }` (all `i32`); `metrics(dpi: u32) -> Metrics`; `slot_center(&Metrics, usize) -> (i32, i32)`; `pub enum RingHit { Center, Slot(usize), Ring, Outside }`; `ring_hit(&Metrics, n: usize, dx: i32, dy: i32) -> RingHit`; `clamp_start(total, start) -> usize`; `page_range(total, start) -> Range<usize>`; `scroll(total, start, pages: i32) -> usize`; `wheel_pages(acc: i32) -> (i32, i32)`; `page_label(total, start) -> String`; `coverage(f32, f32, f32) -> u8`; `mask_disc(&mut [u32], w, h, r: f32)`; `reveal(&[u32], &mut [u32], w, h, r: f32)`; `ease_out_cubic(f64) -> f64`; `anim_radius(from: f32, to: f32, elapsed_ms: f64, dur_ms: f64) -> (f32, bool)`.
- Consumes: `win::scale`.

Why these numbers: slots are squares of 54 px (96 DPI) on radii 90 and 150. `outer_r - inner_r >= slot`, and adjacent outer slots are 55 px apart on both axes at 30 degrees, so no two slot squares overlap at any DPI (the test checks 96/120/144/192).

- [x] **Step 1: Write the failing tests.** Add `pub mod geom;` to `src/circles/mod.rs` and create `src/circles/geom.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_fit_the_ring_and_never_overlap() {
        for dpi in [96, 120, 144, 192] {
            let m = metrics(dpi);
            let half = m.slot / 2;
            let centers: Vec<(i32, i32)> = (0..PER_PAGE).map(|k| slot_center(&m, k)).collect();
            for (k, &(x, y)) in centers.iter().enumerate() {
                // Every slot corner stays inside the ring window square.
                assert!(x.abs() + half <= m.ring_r && y.abs() + half <= m.ring_r, "dpi {dpi} slot {k}");
                // ...and outside the centre disc.
                let d = ((x * x + y * y) as f64).sqrt();
                assert!(d - half as f64 * std::f64::consts::SQRT_2 >= (m.circle_d / 2) as f64 - 1.0, "dpi {dpi} slot {k} hits centre");
                for (j, &(x2, y2)) in centers.iter().enumerate().skip(k + 1) {
                    assert!((x - x2).abs() >= m.slot || (y - y2).abs() >= m.slot, "dpi {dpi} slots {k} and {j} overlap");
                }
            }
        }
    }

    #[test]
    fn slot_zero_is_at_twelve_o_clock() {
        let m = metrics(96);
        assert_eq!(slot_center(&m, 0), (0, -m.inner_r));
    }

    #[test]
    fn ring_hit_finds_centre_slots_ring_and_outside() {
        let m = metrics(96);
        assert_eq!(ring_hit(&m, 20, 0, 0), RingHit::Center);
        let (x, y) = slot_center(&m, 9);
        assert_eq!(ring_hit(&m, 20, x, y), RingHit::Slot(9));
        assert_eq!(ring_hit(&m, 5, x, y), RingHit::Ring, "slot 9 is empty when only 5 are shown");
        assert_eq!(ring_hit(&m, 20, m.ring_r + 1, 0), RingHit::Outside);
        assert_eq!(ring_hit(&m, 20, m.ring_r - 1, m.ring_r - 1), RingHit::Outside, "square corner is outside the disc");
    }

    #[test]
    fn paging_walks_300_items_in_steps_of_20() {
        assert_eq!(page_range(300, 0), 0..20);
        assert_eq!(scroll(300, 0, 1), 20);
        assert_eq!(page_range(300, 20), 20..40);
        assert_eq!(scroll(300, 20, -1), 0);
        assert_eq!(scroll(300, 0, -3), 0, "never before the first page");
        assert_eq!(scroll(300, 280, 5), 280, "never past the last page");
        assert_eq!(page_range(301, 300), 300..301);
        assert_eq!(page_label(300, 20), "21-40 / 300");
        assert_eq!(page_label(20, 0), "");
        assert_eq!(page_range(0, 40), 0..0);
        assert_eq!(clamp_start(300, 37), 20);
        assert_eq!(clamp_start(25, 400), 20, "item count shrank while scrolled: clamped");
    }

    #[test]
    fn wheel_delta_becomes_page_steps() {
        assert_eq!(wheel_pages(-120), (1, 0), "wheel towards the user = next page");
        assert_eq!(wheel_pages(240), (-2, 0));
        assert_eq!(wheel_pages(-50), (0, -50), "partial touchpad deltas accumulate");
        assert_eq!(wheel_pages(-170), (1, -50));
    }

    #[test]
    fn disc_mask_is_opaque_inside_clear_outside_and_premultiplied() {
        let (w, h) = (20usize, 20usize);
        let mut px = vec![0x00ff_8000u32; w * h];
        mask_disc(&mut px, w, h, 8.0);
        assert_eq!(px[10 * w + 10], 0xffff_8000, "centre opaque, colour unchanged");
        assert_eq!(px[0], 0, "corner transparent");
        let edge = px[10 * w + 2]; // pixel centre 7.5 from the disc centre -> partly covered
        let a = edge >> 24;
        assert!(a > 0 && a < 255, "anti-aliased edge, alpha {a}");
        assert!((edge >> 16 & 0xff) <= a, "premultiplied: colour <= alpha");
    }

    #[test]
    fn reveal_cuts_a_smaller_disc_without_touching_the_source() {
        let (w, h) = (20usize, 20usize);
        let mut full = vec![0x0012_3456u32; w * h];
        mask_disc(&mut full, w, h, 10.0);
        let copy = full.clone();
        let mut out = vec![0u32; w * h];
        reveal(&full, &mut out, w, h, 4.0);
        assert_eq!(out[10 * w + 10], full[10 * w + 10]);
        assert_eq!(out[10 * w + 2], 0, "outside the small disc");
        assert_eq!(full, copy);
    }

    #[test]
    fn animation_eases_and_finishes() {
        assert_eq!(anim_radius(10.0, 100.0, 0.0, OPEN_MS), (10.0, false));
        let (mid, done) = anim_radius(10.0, 100.0, OPEN_MS / 2.0, OPEN_MS);
        assert!(!done && mid > 55.0, "ease-out is past half way at half time: {mid}");
        assert_eq!(anim_radius(10.0, 100.0, OPEN_MS + 1.0, OPEN_MS), (100.0, true));
        assert_eq!(anim_radius(100.0, 10.0, CLOSE_MS, CLOSE_MS), (10.0, true));
    }
}
```

- [x] **Step 2: Run to verify failure.**

Run: `cargo test --release --locked circles::geom`
Expected: FAIL to compile (`metrics`, `slot_center`, ... not found).

- [x] **Step 3: Implement.** Insert above the test module:

```rust
//! Pure geometry for circles and rings: slot positions, hit testing, paging, the
//! anti-aliased disc mask and the open/close animation curve. No Win32 here.

use crate::win;

/// Slots shown per ring page: 8 on the inner ring + 12 on the outer ring.
pub const PER_PAGE: usize = 20;
pub const INNER_SLOTS: usize = 8;
pub const OUTER_SLOTS: usize = PER_PAGE - INNER_SLOTS;

/// Ring open/close animation lengths.
pub const OPEN_MS: f64 = 160.0;
pub const CLOSE_MS: f64 = 110.0;

/// Pixel sizes for one DPI. All values are 96-DPI design values scaled by `win::scale`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Metrics {
    /// Diameter of a closed circle window.
    pub circle_d: i32,
    /// Radius of an open ring; the ring window is `2 * ring_r` square.
    pub ring_r: i32,
    /// Side of one (square) slot.
    pub slot: i32,
    /// Icon size inside a slot.
    pub icon: i32,
    /// Distance from the ring centre to inner / outer slot centres.
    pub inner_r: i32,
    pub outer_r: i32,
}

pub fn metrics(dpi: u32) -> Metrics {
    let s = |v| win::scale(v, dpi);
    Metrics { circle_d: s(88), ring_r: s(186), slot: s(54), icon: s(32), inner_r: s(90), outer_r: s(150) }
}

/// Centre of slot `k` relative to the ring centre. Slot 0 is at 12 o'clock on the inner
/// ring; the outer ring starts half a step clockwise of 12 o'clock.
pub fn slot_center(m: &Metrics, k: usize) -> (i32, i32) {
    let (r, deg) = if k < INNER_SLOTS {
        (m.inner_r, -90.0 + k as f64 * (360.0 / INNER_SLOTS as f64))
    } else {
        let j = (k - INNER_SLOTS) % OUTER_SLOTS;
        (m.outer_r, -90.0 + j as f64 * (360.0 / OUTER_SLOTS as f64))
    };
    let a = deg.to_radians();
    ((r as f64 * a.cos()).round() as i32, (r as f64 * a.sin()).round() as i32)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RingHit {
    /// Inside the centre disc (the circle's own title area).
    Center,
    Slot(usize),
    /// Inside the ring but not on a slot.
    Ring,
    Outside,
}

/// What is under `(dx, dy)` (relative to the ring centre) on a ring showing `n` slots.
pub fn ring_hit(m: &Metrics, n: usize, dx: i32, dy: i32) -> RingHit {
    let d2 = dx as i64 * dx as i64 + dy as i64 * dy as i64;
    if d2 > m.ring_r as i64 * m.ring_r as i64 {
        return RingHit::Outside;
    }
    let half = m.slot / 2;
    for k in 0..n.min(PER_PAGE) {
        let (cx, cy) = slot_center(m, k);
        if (dx - cx).abs() <= half && (dy - cy).abs() <= half {
            return RingHit::Slot(k);
        }
    }
    let c = m.circle_d / 2;
    if d2 <= c as i64 * c as i64 {
        RingHit::Center
    } else {
        RingHit::Ring
    }
}

/// Normalises a page start: a multiple of `PER_PAGE`, never past the last page.
pub fn clamp_start(total: usize, start: usize) -> usize {
    if total == 0 {
        return 0;
    }
    let last = (total - 1) / PER_PAGE * PER_PAGE;
    (start / PER_PAGE * PER_PAGE).min(last)
}

/// Indices shown on the page beginning at `start`.
pub fn page_range(total: usize, start: usize) -> std::ops::Range<usize> {
    let s = clamp_start(total, start);
    s..(s + PER_PAGE).min(total)
}

/// New page start after `pages` wheel steps (positive = next page, negative = previous).
pub fn scroll(total: usize, start: usize, pages: i32) -> usize {
    let s = clamp_start(total, start) as i64 + pages as i64 * PER_PAGE as i64;
    clamp_start(total, s.max(0) as usize)
}

/// Converts accumulated wheel delta into whole page steps. Returns `(pages, remainder)`.
/// `WM_MOUSEWHEEL` reports +120 per notch *away* from the user (= previous page).
pub fn wheel_pages(acc: i32) -> (i32, i32) {
    let notches = acc / 120;
    (-notches, acc - notches * 120)
}

/// "21-40 / 300"; empty when everything fits on one page.
pub fn page_label(total: usize, start: usize) -> String {
    if total <= PER_PAGE {
        return String::new();
    }
    let r = page_range(total, start);
    format!("{}-{} / {}", r.start + 1, r.end, total)
}

/// Coverage (0..=255) of the pixel whose centre is `(dx, dy)` from the disc centre.
pub fn coverage(dx: f32, dy: f32, r: f32) -> u8 {
    let d = (dx * dx + dy * dy).sqrt();
    let c = (r - d + 0.5).clamp(0.0, 1.0);
    (c * 255.0).round() as u8
}

/// Turns an opaque BGRA image (`0x00RRGGBB` or any alpha) into a premultiplied
/// image of a disc of radius `r` centred in the `w`x`h` buffer: alpha = coverage,
/// colour scaled by alpha. Pixels outside the disc become fully transparent.
pub fn mask_disc(px: &mut [u32], w: usize, h: usize, r: f32) {
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    for y in 0..h {
        let dy = y as f32 + 0.5 - cy;
        for x in 0..w {
            let a = coverage(x as f32 + 0.5 - cx, dy, r) as u32;
            let i = y * w + x;
            px[i] = if a == 0 { 0 } else { premultiply(px[i] | 0xff00_0000, a) };
        }
    }
}

/// Scales all four channels of a premultiplied pixel by `a / 255`.
fn premultiply(p: u32, a: u32) -> u32 {
    let ch = |shift: u32| (((p >> shift) & 0xff) * a + 127) / 255;
    (ch(24) << 24) | (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

/// Writes `full` (a premultiplied image, see `mask_disc`) into `out`, cut to a disc of
/// radius `r`. Used for the reveal animation; `full` itself is never modified.
pub fn reveal(full: &[u32], out: &mut [u32], w: usize, h: usize, r: f32) {
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    for y in 0..h {
        let dy = y as f32 + 0.5 - cy;
        for x in 0..w {
            let i = y * w + x;
            let a = coverage(x as f32 + 0.5 - cx, dy, r) as u32;
            out[i] = match a {
                0 => 0,
                255 => full[i],
                _ => premultiply(full[i], a),
            };
        }
    }
}

pub fn ease_out_cubic(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Radius of a ring that animates from `from` to `to` over `dur_ms`, `elapsed_ms` in.
/// Returns `(radius, finished)`.
pub fn anim_radius(from: f32, to: f32, elapsed_ms: f64, dur_ms: f64) -> (f32, bool) {
    if dur_ms <= 0.0 || elapsed_ms >= dur_ms {
        return (to, true);
    }
    let k = ease_out_cubic(elapsed_ms / dur_ms) as f32;
    (from + (to - from) * k, false)
}
```

- [x] **Step 4: Run.**

Run: `cargo test --release --locked circles::geom`
Expected: PASS (8 tests).

- [x] **Step 5: Pre-commit checks and commit** (`Add ring geometry, paging and the disc mask`, last line `... Task D2`).

---

### Task D3: Desktop listing, Recent, suggestions and ring contents

**Files:**
- Create: `src/circles/desktop.rs`, `src/circles/suggest.rs`, `src/circles/view.rs`
- Modify: `src/circles/mod.rs` (add `pub mod desktop;`, `pub mod suggest;`, `pub mod view;`)

**Interfaces:**
- Produces (`desktop`): `pub struct Entry { pub path: PathBuf, pub stamp: u64 }`; `skip_name(&str) -> bool`; `recent(&[Entry], &Board) -> Vec<PathBuf>` (unassigned, newest first); `desktop_dirs() -> Vec<PathBuf>`; `list(&[PathBuf]) -> Vec<Entry>`; `pub struct Watcher` with `new(&[PathBuf])`, `handles(&self) -> &[HANDLE]`, `rearm(&self, usize)` (closes its handles on drop).
- Produces (`suggest`): `MAX_SUGGESTIONS = 3`, `MIN_EXAMPLES = 2`; `pub struct Suggestion { pub ext: String, pub circle: usize, pub paths: Vec<PathBuf> }`; `ext_of(&Path) -> Option<String>`; `suggest(&Board, unassigned: &[PathBuf], dismissed: &[(String, usize)]) -> Vec<Suggestion>`.
- Produces (`view`): `pub enum Target { Recent, Circle(usize), Group(usize, usize) }` (`Copy`, `Debug`); `pub enum SlotRef { Back, Group(usize), Suggestion(usize), Item(PathBuf) }`; `slot_refs(&Board, Target, recent: &[PathBuf], &[Suggestion]) -> Vec<SlotRef>`; `title(&Board, Target) -> String`; `circle_count(&Board, usize) -> usize`; `drop_place(Target, Option<&SlotRef>) -> Option<Place>` (`None` = back to Recent).
- Consumes: `model::{Board, Place}` (D1), `items::known_folder` (Phase 1).

- [x] **Step 1: Write the failing tests.** Add the three `pub mod` lines and create each file with only its test module:

`src/circles/desktop.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::circles::model::Place;
    use std::path::Path;

    fn e(path: &str, stamp: u64) -> Entry {
        Entry { path: PathBuf::from(path), stamp }
    }

    #[test]
    fn recent_is_unassigned_newest_first() {
        let mut b = Board::default();
        let m = b.add_circle("Music", (0, 0));
        b.assign(Path::new(r"C:\D\song.wav"), Place::Circle(m));
        let entries = [e(r"C:\D\old.txt", 10), e(r"C:\D\song.wav", 99), e(r"C:\D\new.pdf", 50), e(r"C:\D\B.txt", 10)];
        let r = recent(&entries, &b);
        let names: Vec<String> = r.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["new.pdf", "B.txt", "old.txt"]);
    }

    #[test]
    fn explorer_metadata_files_are_skipped() {
        assert!(skip_name("desktop.ini"));
        assert!(skip_name("Thumbs.DB"));
        assert!(!skip_name("notes.txt"));
    }

    #[test]
    fn listing_reads_a_real_folder_and_skips_hidden_files() {
        let dir = std::env::temp_dir().join(format!("desktuk-list-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "x").unwrap();
        std::fs::write(dir.join("desktop.ini"), "x").unwrap();
        let entries = list(std::slice::from_ref(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path.file_name().unwrap(), "a.txt");
        assert!(entries[0].stamp > 0);
    }
}
```

`src/circles/suggest.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::circles::model::Place;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    fn board() -> Board {
        let mut b = Board::default();
        let m = b.add_circle("Music", (0, 0));
        let k = b.add_circle("Books", (0, 0));
        let g = b.add_group(m, "Live").unwrap();
        b.assign(&p(r"C:\D\a.wav"), Place::Circle(m));
        b.assign(&p(r"C:\D\b.WAV"), Place::Group(m, g));
        b.assign(&p(r"C:\D\c.pdf"), Place::Circle(k));
        b.assign(&p(r"C:\D\d.pdf"), Place::Circle(k));
        b.assign(&p(r"C:\D\e.pdf"), Place::Circle(m));
        b.assign(&p(r"C:\D\x.lnk"), Place::Circle(m));
        b.assign(&p(r"C:\D\y.lnk"), Place::Circle(m));
        b
    }

    #[test]
    fn suggests_by_extension_majority() {
        let un = [p(r"C:\D\new.wav"), p(r"C:\D\n2.wav"), p(r"C:\D\book.pdf"), p(r"C:\D\app.lnk"), p(r"C:\D\README")];
        let s = suggest(&board(), &un, &[]);
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].ext.as_str(), s[0].circle, s[0].paths.len()), ("wav", 0, 2), "groups count for their circle");
        assert_eq!((s[1].ext.as_str(), s[1].circle), ("pdf", 1));
    }

    #[test]
    fn no_suggestion_without_enough_examples_a_clear_winner_or_after_dismissal() {
        let mut b = board();
        b.assign(&p(r"C:\D\f.pdf"), Place::Circle(0)); // pdf: Music 2, Books 2 -> tie
        let un = [p(r"C:\D\book.pdf"), p(r"C:\D\new.wav")];
        let s = suggest(&b, &un, &[]);
        assert_eq!(s.iter().map(|s| s.ext.as_str()).collect::<Vec<_>>(), ["wav"]);
        assert!(suggest(&b, &un, &[("wav".into(), 0)]).is_empty());
        assert!(suggest(&Board::default(), &un, &[]).is_empty());
    }

    #[test]
    fn generic_extensions_are_ignored() {
        assert_eq!(ext_of(&p(r"C:\a.LNK")), None);
        assert_eq!(ext_of(&p(r"C:\a")), None);
        assert_eq!(ext_of(&p(r"C:\a.Wav")), Some("wav".into()));
    }
}
```

`src/circles/view.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    fn board() -> Board {
        let mut b = Board::default();
        let m = b.add_circle("Music", (0, 0));
        let g = b.add_group(m, "Live").unwrap();
        b.assign(&p(r"C:\D\a.wav"), Place::Circle(m));
        b.assign(&p(r"C:\D\b.wav"), Place::Group(m, g));
        b
    }

    #[test]
    fn circle_view_lists_groups_then_items() {
        let b = board();
        assert_eq!(slot_refs(&b, Target::Circle(0), &[], &[]), vec![SlotRef::Group(0), SlotRef::Item(p(r"C:\D\a.wav"))]);
        assert_eq!(slot_refs(&b, Target::Group(0, 0), &[], &[]), vec![SlotRef::Back, SlotRef::Item(p(r"C:\D\b.wav"))]);
        assert!(slot_refs(&b, Target::Circle(5), &[], &[]).is_empty());
        assert!(slot_refs(&b, Target::Group(0, 5), &[], &[]).is_empty());
        assert_eq!(circle_count(&b, 0), 2);
        assert_eq!(title(&b, Target::Group(0, 0)), "Music / Live");
        assert_eq!(title(&b, Target::Recent), "Recent");
    }

    #[test]
    fn recent_view_lists_suggestions_first() {
        let b = board();
        let s = vec![Suggestion { ext: "wav".into(), circle: 0, paths: vec![p(r"C:\D\c.wav")] }];
        let refs = slot_refs(&b, Target::Recent, &[p(r"C:\D\c.wav")], &s);
        assert_eq!(refs, vec![SlotRef::Suggestion(0), SlotRef::Item(p(r"C:\D\c.wav"))]);
    }

    #[test]
    fn drops_land_in_the_group_under_the_cursor_or_the_view_itself() {
        assert_eq!(drop_place(Target::Circle(1), Some(&SlotRef::Group(2))), Some(Place::Group(1, 2)));
        assert_eq!(drop_place(Target::Circle(1), Some(&SlotRef::Item(p(r"C:\x")))), Some(Place::Circle(1)));
        assert_eq!(drop_place(Target::Circle(1), None), Some(Place::Circle(1)));
        assert_eq!(drop_place(Target::Group(1, 2), None), Some(Place::Group(1, 2)));
        assert_eq!(drop_place(Target::Group(1, 2), Some(&SlotRef::Back)), Some(Place::Circle(1)));
        assert_eq!(drop_place(Target::Recent, None), None, "dropping on Recent unassigns");
    }
}
```

- [x] **Step 2: Run to verify failure.**

Run: `cargo test --release --locked circles::`
Expected: FAIL to compile (`Entry`, `recent`, `suggest`, `slot_refs`, ... not found).

- [x] **Step 3: Implement.** Insert above each test module:

`src/circles/desktop.rs`:

```rust
//! The Desktop folders: listing their entries, computing "Recent" (every entry that is
//! not in a circle, newest first) and waking the message loop when they change.
//!
//! Read-only: nothing here creates, moves, renames or deletes anything.

use std::os::windows::fs::MetadataExt;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    FindCloseChangeNotification, FindFirstChangeNotificationW, FindNextChangeNotification, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_SYSTEM,
    FILE_NOTIFY_CHANGE_DIR_NAME, FILE_NOTIFY_CHANGE_FILE_NAME,
};
use windows_sys::Win32::UI::Shell::{FOLDERID_Desktop, FOLDERID_PublicDesktop};

use super::model::Board;
use crate::{dlog, items, win};

/// One Desktop entry. `stamp` = newest of creation/modification time (ms since 1970),
/// so a file that was just copied or downloaded to the Desktop sorts first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub stamp: u64,
}

/// Names Explorer never shows on the Desktop.
pub fn skip_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("desktop.ini") || name.eq_ignore_ascii_case("thumbs.db")
}

/// Desktop entries that are in no circle, newest first (ties: by name, case-insensitive).
pub fn recent(entries: &[Entry], board: &Board) -> Vec<PathBuf> {
    let mut v: Vec<&Entry> = entries.iter().filter(|e| !board.is_assigned(&e.path)).collect();
    v.sort_by(|a, b| {
        b.stamp.cmp(&a.stamp).then_with(|| {
            let n = |e: &Entry| e.path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
            n(a).cmp(&n(b))
        })
    });
    v.into_iter().map(|e| e.path.clone()).collect()
}

/// The user's and the public Desktop folder (whichever exist).
pub fn desktop_dirs() -> Vec<PathBuf> {
    [&FOLDERID_Desktop, &FOLDERID_PublicDesktop].into_iter().filter_map(items::known_folder).collect()
}

/// Lists `dirs` (direct children only). Hidden and system entries are skipped, like
/// Explorer does by default.
pub fn list(dirs: &[PathBuf]) -> Vec<Entry> {
    let mut out = Vec::new();
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(d) else { continue };
        for e in rd.flatten() {
            let name = e.file_name();
            if skip_name(&name.to_string_lossy()) {
                continue;
            }
            let Ok(md) = e.metadata() else { continue };
            if md.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0 {
                continue;
            }
            let ms = |t: std::io::Result<std::time::SystemTime>| {
                t.ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_millis() as u64).unwrap_or(0)
            };
            let stamp = ms(md.created()).max(ms(md.modified()));
            out.push(Entry { path: e.path(), stamp });
        }
    }
    out
}

/// Kernel change notifications for the Desktop folders. The handles are waited on by the
/// message loop (`MsgWaitForMultipleObjectsEx`), so watching costs no thread and no CPU.
pub struct Watcher {
    handles: Vec<HANDLE>,
}

impl Watcher {
    pub fn new(dirs: &[PathBuf]) -> Watcher {
        let mut handles = Vec::new();
        for d in dirs {
            let w = win::wide_path(d);
            let h = unsafe { FindFirstChangeNotificationW(w.as_ptr(), 0, FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_DIR_NAME) };
            if h.is_null() || h == INVALID_HANDLE_VALUE {
                dlog!("desktop watch failed for {}", d.display());
            } else {
                handles.push(h);
            }
        }
        Watcher { handles }
    }

    pub fn handles(&self) -> &[HANDLE] {
        &self.handles
    }

    /// Re-arms handle `i` after it was signalled.
    pub fn rearm(&self, i: usize) {
        if let Some(&h) = self.handles.get(i) {
            unsafe { FindNextChangeNotification(h) };
        }
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        for &h in &self.handles {
            unsafe { FindCloseChangeNotification(h) };
        }
    }
}
```

`src/circles/suggest.rs`:

```rust
//! Rule suggestions: "these 12 .wav files on the Desktop probably belong in Music".
//!
//! Pure. A suggestion is only ever *offered*; nothing is assigned until the owner clicks it.

use std::path::{Path, PathBuf};

use super::model::Board;

/// Most suggestions shown at once.
pub const MAX_SUGGESTIONS: usize = 3;
/// A circle must already hold at least this many files of an extension.
pub const MIN_EXAMPLES: usize = 2;
/// Extensions that say nothing about what a file is about.
const GENERIC: [&str; 3] = ["lnk", "url", "tmp"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    /// Lower-case extension without the dot.
    pub ext: String,
    pub circle: usize,
    pub paths: Vec<PathBuf>,
}

pub fn ext_of(p: &Path) -> Option<String> {
    let e = p.extension()?.to_string_lossy().to_lowercase();
    if e.is_empty() || GENERIC.contains(&e.as_str()) {
        None
    } else {
        Some(e)
    }
}

/// Suggestions for `unassigned` paths. An extension points at circle `c` when `c` holds at
/// least `MIN_EXAMPLES` files with it and strictly more than any other circle. Pairs in
/// `dismissed` (extension, circle) are not offered again.
pub fn suggest(board: &Board, unassigned: &[PathBuf], dismissed: &[(String, usize)]) -> Vec<Suggestion> {
    use std::collections::BTreeMap;
    // ext -> per-circle counts
    let mut counts: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (ci, c) in board.circles.iter().enumerate() {
        let all = c.items.iter().chain(c.groups.iter().flat_map(|g| g.items.iter()));
        for p in all {
            if let Some(e) = ext_of(p) {
                counts.entry(e).or_insert_with(|| vec![0; board.circles.len()])[ci] += 1;
            }
        }
    }
    let mut out: Vec<Suggestion> = Vec::new();
    for (ext, per) in counts {
        let Some((best, &n)) = per.iter().enumerate().max_by_key(|(_, &n)| n) else { continue };
        let unique_best = per.iter().filter(|&&m| m == n).count() == 1;
        if n < MIN_EXAMPLES || !unique_best || dismissed.iter().any(|(e, c)| *e == ext && *c == best) {
            continue;
        }
        let paths: Vec<PathBuf> = unassigned.iter().filter(|p| ext_of(p).as_deref() == Some(ext.as_str())).cloned().collect();
        if !paths.is_empty() {
            out.push(Suggestion { ext, circle: best, paths });
        }
    }
    out.sort_by(|a, b| b.paths.len().cmp(&a.paths.len()).then_with(|| a.ext.cmp(&b.ext)));
    out.truncate(MAX_SUGGESTIONS);
    out
}
```

`src/circles/view.rs`:

```rust
//! What an open ring shows: pure mapping from the board to slots. No Win32.

use std::path::PathBuf;

use super::model::{Board, Place};
use super::suggest::Suggestion;

/// What a ring is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// The built-in Recent circle: Desktop files that are in no circle.
    Recent,
    Circle(usize),
    /// A group inside a circle (the third and last level).
    Group(usize, usize),
}

/// One slot of a ring, before paging.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlotRef {
    /// "Back to the circle" (first slot of a group view).
    Back,
    Group(usize),
    /// Index into the suggestion list (Recent only).
    Suggestion(usize),
    Item(PathBuf),
}

/// All slots of `target`, in display order. An invalid target yields no slots.
pub fn slot_refs(board: &Board, target: Target, recent: &[PathBuf], suggestions: &[Suggestion]) -> Vec<SlotRef> {
    match target {
        Target::Recent => (0..suggestions.len()).map(SlotRef::Suggestion).chain(recent.iter().cloned().map(SlotRef::Item)).collect(),
        Target::Circle(ci) => match board.circles.get(ci) {
            Some(c) => (0..c.groups.len()).map(SlotRef::Group).chain(c.items.iter().cloned().map(SlotRef::Item)).collect(),
            None => Vec::new(),
        },
        Target::Group(ci, gi) => match board.circles.get(ci).and_then(|c| c.groups.get(gi)) {
            Some(g) => std::iter::once(SlotRef::Back).chain(g.items.iter().cloned().map(SlotRef::Item)).collect(),
            None => Vec::new(),
        },
    }
}

pub fn title(board: &Board, target: Target) -> String {
    match target {
        Target::Recent => "Recent".to_string(),
        Target::Circle(ci) => board.circles.get(ci).map(|c| c.name.clone()).unwrap_or_default(),
        Target::Group(ci, gi) => match board.circles.get(ci).and_then(|c| Some((c, c.groups.get(gi)?))) {
            Some((c, g)) => format!("{} / {}", c.name, g.name),
            None => String::new(),
        },
    }
}

/// Number shown on a closed circle: its items including those in groups.
pub fn circle_count(board: &Board, ci: usize) -> usize {
    board.circles.get(ci).map(|c| c.items.len() + c.groups.iter().map(|g| g.items.len()).sum::<usize>()).unwrap_or(0)
}

/// Where a file dropped on `target` ends up, given the slot it was dropped on (if any).
/// `None` means "unassign" (back to Recent).
pub fn drop_place(target: Target, on: Option<&SlotRef>) -> Option<Place> {
    match (target, on) {
        (Target::Circle(ci), Some(SlotRef::Group(gi))) => Some(Place::Group(ci, *gi)),
        (Target::Group(ci, _), Some(SlotRef::Back)) => Some(Place::Circle(ci)),
        (Target::Circle(ci), _) => Some(Place::Circle(ci)),
        (Target::Group(ci, gi), _) => Some(Place::Group(ci, gi)),
        (Target::Recent, _) => None,
    }
}
```

- [x] **Step 4: Run.**

Run: `cargo test --release --locked circles::`
Expected: PASS (model 7, geom 8, desktop 3, suggest 3, view 3). `listing_reads_a_real_folder_and_skips_hidden_files` writes only under `%TEMP%`.

- [x] **Step 5: Pre-commit checks and commit** (`Add Desktop listing, Recent, suggestions and ring contents`, last line `... Task D3`).

---

### Task D4: Layered surface and drawing

**Files:**
- Create: `src/circles/surface.rs`, `src/circles/draw.rs`
- Modify: `src/circles/mod.rs` (add `pub mod draw;`, `pub mod surface;`)

**Interfaces:**
- Produces (`surface`): `pub struct Surface { pub w: i32, pub h: i32, .. }` with `new(w, h) -> Option<Surface>`, `dc(&self) -> HDC`, `pixels(&mut self) -> &mut [u32]`, `present(&self, hwnd: HWND, at: POINT) -> bool`; frees its GDI objects on drop.
- Produces (`draw`): colour constants; `circle(&Surface, dpi, name: &str, count: usize, recent: bool)`; `pub enum SlotLook { Item { icon: HICON, dim: bool }, Group, Back, Suggestion { count: usize } }`; `pub struct SlotView<'a> { pub label: &'a str, pub look: SlotLook }`; `pub struct RingView<'a> { title, subtitle, slots, hover: Option<usize>, recent: bool }`; `ring(&Surface, dpi, &RingView)`.
- Consumes: `geom::{metrics, slot_center, PER_PAGE, mask_disc}` (D2), `node::ACCENT` (Phase 1).

Drawing rule: everything is drawn on an opaque background and `geom::mask_disc` runs **last** (GDI text writes alpha 0; the mask sets every alpha byte).

- [x] **Step 1: Write the preview test first.** Add the two `pub mod` lines and create `src/circles/draw.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::circles::geom;

    /// Writes `px` (premultiplied BGRA) composited over a desktop-blue background as a 24-bit BMP.
    fn save_bmp(path: &std::path::Path, px: &[u32], w: usize, h: usize) {
        let row = (w * 3).div_ceil(4) * 4;
        let mut f: Vec<u8> = Vec::with_capacity(54 + row * h);
        let size = (54 + row * h) as u32;
        f.extend_from_slice(b"BM");
        f.extend_from_slice(&size.to_le_bytes());
        f.extend_from_slice(&[0, 0, 0, 0, 54, 0, 0, 0, 40, 0, 0, 0]);
        f.extend_from_slice(&(w as i32).to_le_bytes());
        f.extend_from_slice(&(-(h as i32)).to_le_bytes());
        f.extend_from_slice(&[1, 0, 24, 0]);
        f.extend_from_slice(&[0u8; 24]);
        let bg = [0x80u32, 0x60, 0x40]; // B, G, R
        for y in 0..h {
            for x in 0..w {
                let p = px[y * w + x];
                let a = p >> 24;
                for (i, shift) in [0u32, 8, 16].into_iter().enumerate() {
                    let c = (p >> shift) & 0xff;
                    f.push((c + bg[i] * (255 - a) / 255).min(255) as u8);
                }
            }
            f.extend(std::iter::repeat_n(0u8, row - w * 3));
        }
        std::fs::write(path, f).unwrap();
    }

    /// Visual check, not run by default: `cargo test --release preview -- --ignored`
    /// then open `%TEMP%\desktuk-preview-*.bmp`.
    #[test]
    #[ignore]
    fn preview() {
        let dpi = 144;
        let m = geom::metrics(dpi);
        let d = m.circle_d;
        let mut s = Surface::new(d, d).unwrap();
        circle(&s, dpi, "Kitaplar ve Dergiler", 128, false);
        geom::mask_disc(s.pixels(), d as usize, d as usize, d as f32 / 2.0);
        save_bmp(&std::env::temp_dir().join("desktuk-preview-circle.bmp"), s.pixels(), d as usize, d as usize);

        let size = 2 * m.ring_r;
        let mut r = Surface::new(size, size).unwrap();
        let labels: Vec<String> = (0..20).map(|i| format!("Song {i:02}.wav")).collect();
        let mut slots: Vec<SlotView> =
            labels.iter().map(|l| SlotView { label: l, look: SlotLook::Item { icon: std::ptr::null_mut(), dim: false } }).collect();
        slots[0] = SlotView { label: "Titled Project", look: SlotLook::Group };
        slots[1] = SlotView { label: ".wav \u{2192} Music", look: SlotLook::Suggestion { count: 12 } };
        slots[2] = SlotView { label: "Back", look: SlotLook::Back };
        ring(&r, dpi, &RingView { title: "Music", subtitle: "21-40 / 300", slots: &slots, hover: Some(5), recent: false });
        geom::mask_disc(r.pixels(), size as usize, size as usize, m.ring_r as f32);
        save_bmp(&std::env::temp_dir().join("desktuk-preview-ring.bmp"), r.pixels(), size as usize, size as usize);
    }
}
```

Run: `cargo test --release --locked circles::draw -- --ignored`
Expected: FAIL to compile (`Surface`, `circle`, `ring`, `SlotView`, ... not found).

- [x] **Step 2: Implement the surface.** Create `src/circles/surface.rs`:

```rust
//! A 32-bit top-down DIB section for painting per-pixel-alpha (layered) windows.
//!
//! GDI draws into `dc()`; `geom::mask_disc` then turns the pixels into a premultiplied
//! disc and `present` hands them to the window with `UpdateLayeredWindow`. The system
//! keeps its own copy of what was presented, so a surface can be dropped right after.

use windows_sys::Win32::Foundation::{HWND, POINT, SIZE};
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GdiFlush, SelectObject, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{UpdateLayeredWindow, ULW_ALPHA};

pub struct Surface {
    dc: HDC,
    bmp: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u32,
    pub w: i32,
    pub h: i32,
}

impl Surface {
    /// `None` if GDI is out of resources.
    pub fn new(w: i32, h: i32) -> Option<Surface> {
        let (w, h) = (w.max(1), h.max(1));
        unsafe {
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            if dc.is_null() {
                return None;
            }
            let mut bi: BITMAPINFO = std::mem::zeroed();
            bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bi.bmiHeader.biWidth = w;
            bi.bmiHeader.biHeight = -h; // top-down rows
            bi.bmiHeader.biPlanes = 1;
            bi.bmiHeader.biBitCount = 32;
            bi.bmiHeader.biCompression = BI_RGB;
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let bmp = CreateDIBSection(dc, &bi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
            if bmp.is_null() || bits.is_null() {
                if !bmp.is_null() {
                    DeleteObject(bmp);
                }
                DeleteDC(dc);
                return None;
            }
            let old = SelectObject(dc, bmp);
            Some(Surface { dc, bmp, old, bits: bits.cast(), w, h })
        }
    }

    pub fn dc(&self) -> HDC {
        self.dc
    }

    /// The pixels as `0xAARRGGBB`, row-major, `w * h` long. Flushes pending GDI drawing first.
    pub fn pixels(&mut self) -> &mut [u32] {
        unsafe {
            GdiFlush();
            std::slice::from_raw_parts_mut(self.bits, (self.w * self.h) as usize)
        }
    }

    /// Shows the (premultiplied) pixels as the content of layered window `hwnd`, whose
    /// top-left corner is moved to `at` (screen pixels) and whose size becomes `w`x`h`.
    pub fn present(&self, hwnd: HWND, at: POINT) -> bool {
        let size = SIZE { cx: self.w, cy: self.h };
        let src = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
        unsafe { UpdateLayeredWindow(hwnd, std::ptr::null_mut(), &at, &size, self.dc, &src, 0, &blend, ULW_ALPHA) != 0 }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old);
            DeleteObject(self.bmp);
            DeleteDC(self.dc);
        }
    }
}
```

- [x] **Step 3: Implement the drawing.** Insert above the test module of `src/circles/draw.rs`:

```rust
//! GDI drawing of closed circles and open rings into a `Surface`.
//!
//! Everything is drawn on an opaque background; the caller then applies
//! `geom::mask_disc`, which sets the alpha channel (GDI text leaves alpha bytes at 0,
//! so the mask must always run last).

use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Gdi::{
    CreateFontIndirectW, CreateSolidBrush, DeleteObject, DrawTextW, Ellipse, FillRect, GetStockObject, SelectObject, SetBkMode,
    SetTextColor, DT_CENTER, DT_EDITCONTROL, DT_END_ELLIPSIS, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK, FW_SEMIBOLD, HDC,
    HFONT, LOGFONTW, NULL_PEN, TRANSPARENT,
};
use windows_sys::Win32::UI::HiDpi::SystemParametersInfoForDpi;
use windows_sys::Win32::UI::WindowsAndMessaging::{DrawIconEx, DI_NORMAL, HICON, SPI_GETICONTITLELOGFONT};

use super::geom::{self, Metrics};
use super::surface::Surface;
use crate::win;

pub const CIRCLE_BG: u32 = win::rgb(30, 31, 36);
pub const RECENT_BG: u32 = win::rgb(32, 48, 68);
pub const CENTER_BG: u32 = win::rgb(40, 42, 50);
pub const ACCENT: u32 = crate::node::ACCENT;
pub const HOVER: u32 = win::rgb(56, 60, 72);
pub const GROUP: u32 = win::rgb(70, 90, 130);
pub const TEXT: u32 = win::rgb(240, 240, 240);
pub const SUBTEXT: u32 = win::rgb(170, 175, 185);

/// The icon-title font scaled to `pct` percent, deleted on drop.
struct Font(HFONT);

impl Font {
    fn new(dpi: u32, bold: bool, pct: i32) -> Font {
        unsafe {
            let mut lf: LOGFONTW = std::mem::zeroed();
            SystemParametersInfoForDpi(
                SPI_GETICONTITLELOGFONT,
                std::mem::size_of::<LOGFONTW>() as u32,
                (&mut lf as *mut LOGFONTW).cast(),
                0,
                dpi,
            );
            lf.lfHeight = lf.lfHeight * pct / 100;
            if bold {
                lf.lfWeight = FW_SEMIBOLD as i32;
            }
            Font(CreateFontIndirectW(&lf))
        }
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        unsafe { DeleteObject(self.0) };
    }
}

fn fill_rect(dc: HDC, r: &RECT, color: u32) {
    unsafe {
        let b = CreateSolidBrush(color);
        FillRect(dc, r, b);
        DeleteObject(b);
    }
}

fn fill_ellipse(dc: HDC, r: &RECT, color: u32) {
    unsafe {
        let b = CreateSolidBrush(color);
        let old_b = SelectObject(dc, b);
        let old_p = SelectObject(dc, GetStockObject(NULL_PEN));
        // With a null pen the filled area is one pixel smaller; compensate.
        Ellipse(dc, r.left, r.top, r.right + 1, r.bottom + 1);
        SelectObject(dc, old_p);
        SelectObject(dc, old_b);
        DeleteObject(b);
    }
}

fn text(dc: HDC, s: &str, r: RECT, color: u32, font: &Font, flags: u32) {
    let mut w = win::wide(s);
    let mut r = r;
    unsafe {
        let old = SelectObject(dc, font.0);
        SetTextColor(dc, color);
        DrawTextW(dc, w.as_mut_ptr(), -1, &mut r, flags | DT_NOPREFIX);
        SelectObject(dc, old);
    }
}

/// Square around `(cx, cy)` with side `side`.
fn square(cx: i32, cy: i32, side: i32) -> RECT {
    win::rect(cx - side / 2, cy - side / 2, cx - side / 2 + side, cy - side / 2 + side)
}

/// A closed circle: accent rim, name (up to two lines) and the item count.
pub fn circle(s: &Surface, dpi: u32, name: &str, count: usize, recent: bool) {
    let dc = s.dc();
    let d = s.w;
    let sc = |v| win::scale(v, dpi);
    fill_rect(dc, &win::rect(0, 0, d, d), ACCENT);
    let rim = sc(3).max(2);
    fill_ellipse(dc, &win::rect(rim, rim, d - rim, d - rim), if recent { RECENT_BG } else { CIRCLE_BG });
    unsafe { SetBkMode(dc, TRANSPARENT as i32) };
    let bold = Font::new(dpi, true, 100);
    let small = Font::new(dpi, false, 90);
    let pad = sc(12);
    text(
        dc,
        name,
        win::rect(pad, d * 24 / 100, d - pad, d * 62 / 100),
        TEXT,
        &bold,
        DT_CENTER | DT_WORDBREAK | DT_EDITCONTROL | DT_END_ELLIPSIS,
    );
    text(
        dc,
        &count.to_string(),
        win::rect(pad, d * 62 / 100, d - pad, d * 80 / 100),
        SUBTEXT,
        &small,
        DT_CENTER | DT_SINGLELINE | DT_VCENTER,
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotLook {
    /// A file: its shell icon. `dim` for files that no longer exist.
    Item { icon: HICON, dim: bool },
    /// A group: a filled disc with the group's first letter.
    Group,
    /// "Back to the circle" inside a group view.
    Back,
    /// A rule suggestion: accent disc with "+N".
    Suggestion { count: usize },
}

pub struct SlotView<'a> {
    pub label: &'a str,
    pub look: SlotLook,
}

pub struct RingView<'a> {
    pub title: &'a str,
    /// Page indicator or hint, under the title.
    pub subtitle: &'a str,
    pub slots: &'a [SlotView<'a>],
    pub hover: Option<usize>,
    pub recent: bool,
}

/// An open ring: rim, centre disc with title/subtitle, and up to `PER_PAGE` slots.
pub fn ring(s: &Surface, dpi: u32, v: &RingView) {
    let m: Metrics = geom::metrics(dpi);
    let dc = s.dc();
    let size = s.w;
    let c = size / 2;
    let sc = |x| win::scale(x, dpi);
    let bg = if v.recent { RECENT_BG } else { CIRCLE_BG };
    fill_rect(dc, &win::rect(0, 0, size, size), ACCENT);
    let rim = sc(3).max(2);
    fill_ellipse(dc, &win::rect(rim, rim, size - rim, size - rim), bg);
    fill_ellipse(dc, &square(c, c, m.circle_d), CENTER_BG);
    unsafe { SetBkMode(dc, TRANSPARENT as i32) };
    let bold = Font::new(dpi, true, 100);
    let small = Font::new(dpi, false, 85);
    let inner = m.circle_d / 2 - sc(8);
    text(
        dc,
        v.title,
        win::rect(c - inner, c - inner, c + inner, c + sc(6)),
        TEXT,
        &bold,
        DT_CENTER | DT_WORDBREAK | DT_EDITCONTROL | DT_END_ELLIPSIS,
    );
    text(dc, v.subtitle, win::rect(c - inner, c + sc(8), c + inner, c + inner), SUBTEXT, &small, DT_CENTER | DT_WORDBREAK);

    for (k, slot) in v.slots.iter().enumerate().take(geom::PER_PAGE) {
        let (dx, dy) = geom::slot_center(&m, k);
        let b = square(c + dx, c + dy, m.slot);
        if v.hover == Some(k) {
            fill_rect(dc, &b, HOVER);
        }
        let ix = b.left + (m.slot - m.icon) / 2;
        let iy = b.top + sc(3);
        let glyph = |color: u32, letter: &str| {
            fill_ellipse(dc, &win::rect(ix, iy, ix + m.icon, iy + m.icon), color);
            text(dc, letter, win::rect(ix, iy, ix + m.icon, iy + m.icon), TEXT, &bold, DT_CENTER | DT_SINGLELINE | DT_VCENTER);
        };
        let mut label_color = TEXT;
        match slot.look {
            SlotLook::Item { icon, dim } => {
                if !icon.is_null() {
                    unsafe { DrawIconEx(dc, ix, iy, icon, m.icon, m.icon, 0, std::ptr::null_mut(), DI_NORMAL) };
                }
                if dim {
                    label_color = SUBTEXT;
                }
            }
            SlotLook::Group => {
                let first: String = slot.label.chars().take(1).collect::<String>().to_uppercase();
                glyph(GROUP, &first);
            }
            SlotLook::Back => glyph(CENTER_BG, "\u{2039}"),
            SlotLook::Suggestion { count } => glyph(ACCENT, &format!("+{count}")),
        }
        let lr = win::rect(b.left, iy + m.icon + sc(2), b.right, b.bottom);
        text(dc, slot.label, lr, label_color, &small, DT_CENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
    }
}
```

- [x] **Step 4: Render the preview and look at it.**

Run: `cargo test --release --locked circles::draw -- --ignored`
Expected: PASS; `%TEMP%\desktuk-preview-circle.bmp` shows a dark disc with a blue rim, "Kitaplar ve Dergiler" on two lines and "128"; `%TEMP%\desktuk-preview-ring.bmp` shows a disc with "Music" / "21-40 / 300" in the centre, 20 slots on two rings (a lettered group disc "T", a blue "+12" suggestion, a "<" Back disc, 17 "Song NN.wav" labels) and a highlighted square on slot 5. Corners are the background colour (transparent). Mention in the commit body that you looked at both.

- [x] **Step 5: Pre-commit checks** (`cargo clippy --release --locked --all-targets -- -D warnings` too, so the test code is linted) **and commit** (`Add the layered surface and circle/ring drawing`, last line `... Task D4`).

---

### Task D5: The circles UI - windows, ring, rename, start-up

**Files:**
- Create: `src/circles/circle_win.rs`, `src/circles/ring.rs`, `src/circles/rename.rs`
- Replace: `src/circles/mod.rs` (final version, no `allow(dead_code)`)
- Modify: `src/app.rs` (diff below)

**Interfaces:**
- Produces (`circle_win`): `pub enum Which { Recent, User(usize) }` with `target() -> Target`; `pub struct CircleWin { pub hwnd, pub host: Rc<dyn DesktopHost>, pub which }`; `register_class()`; `create(Which, (i32, i32)) -> Option<CircleWin>`; `render(HWND, Which)`; `clamp_to_work((i32, i32), d: i32) -> (i32, i32)`; `dropped_paths(HDROP) -> Vec<PathBuf>`.
- Produces (`ring`): `register_class()`, `open(anchor: HWND, Target)`, `close_now()`, `is_open_for(HWND) -> bool`, `refresh()`, `apply_metas(Vec<(PathBuf, Meta)>)`.
- Produces (`rename`): `pub enum RenameTarget { Circle(usize), Group(usize, usize) }`, `register_class()`, `begin(RenameTarget, anchor: RECT, initial: &str)`, `cancel()`.
- Produces (`circles`): `start()`, `shutdown()`, `rebuild()`, `refresh_all()`, `which_of(HWND)`, `host_of(HWND)`, `foreground_changed(HWND)`, `default_recent_pos()`, `recent_paths()`, `slots_for(Target)`, `wait_handles() -> Vec<HANDLE>`, `on_desktop_signal(usize)`, `refresh_desktop()`, `on_shell_change(&Change)`, `assign(&[PathBuf], Option<Place>)`, `moved(Which, (i32, i32))`, `accept_suggestion`, `dismiss_suggestion`, `popup(&[(usize, &str)], POINT) -> usize` (id 0 = separator), `circle_menu(Which, POINT)`, `begin_rename(RenameTarget)`, `commit_rename(RenameTarget, &str)`, `delete_group(usize, usize)`.
- Produces (`app`): `pub const TIMER_DESKTOP: usize = 8`; `pub fn schedule_trim()`; `App.ui: Ui`; `App.board: RefCell<Board>`.
- Consumes: everything from D1-D4; Plan A `items::{Item, Meta, spawn_loader, open}`, `watch::decode`, `shell_menu::{show, Choice}`; `host::{create, DesktopHost, HostKind}`.

How the pieces fit (read before copying):
* `app::run` picks the UI (`--selftest` -> panel; else `--ui`, else the config's `ui=`, else circles). Circles mode creates no node and calls `circles::start()`.
* `circles::start` lists the Desktop, creates a `Watcher`, registers three window classes and `rebuild()`s: one window for Recent, one per circle. Windows are `WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE` popups; each gets its own `WindowHost` (its "raised during Show desktop" state is per window).
* `circle_win::render` paints into a temporary `Surface` and presents it; the system keeps the pixels, so nothing stays allocated.
* `ring::open` creates the ring window **owned by** the circle (owned windows stay above their owner), loads placeholders for the page and asks Plan A's loader thread for metadata (`WM_APP_ITEMS_LOADED` -> `ring::apply_metas`). A 15 ms timer runs only while animating.
* The message loop (`pump_messages`) waits on the Watcher handles with `MsgWaitForMultipleObjectsEx`; a signal re-arms the handle and (re)starts the 250 ms `TIMER_DESKTOP`, which re-lists the Desktop.
* When a ring closes it frees its surface and icons, calls `CoFreeUnusedLibrariesEx` and schedules the existing working-set trim.

- [x] **Step 1: Create `src/circles/circle_win.rs`:**

```rust
//! Closed circles: small round layered tool windows that sit on the desktop.
//!
//! Hover (or click) opens the ring, dragging moves the circle, right-click opens the
//! circle menu, and files dropped from Explorer are added to the circle.

use std::cell::Cell;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::rc::Rc;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromRect, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::{WM_MOUSEHOVER, WM_MOUSELEAVE};
use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetCapture, ReleaseCapture, SetCapture, TrackMouseEvent, TME_HOVER, TME_LEAVE, TRACKMOUSEEVENT,
};
use windows_sys::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use super::surface::Surface;
use super::view::{self, Target};
use super::{draw, geom, ring};
use crate::app::app;
use crate::host::{self, DesktopHost, HostKind};
use crate::{dlog, win};

const CLASS: &str = "DeskTuk.Circle";
/// How long the pointer must rest on a circle before its ring opens.
const HOVER_MS: u32 = 250;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Recent,
    /// Index into `Board::circles`.
    User(usize),
}

impl Which {
    pub fn target(self) -> Target {
        match self {
            Which::Recent => Target::Recent,
            Which::User(ci) => Target::Circle(ci),
        }
    }
}

pub struct CircleWin {
    pub hwnd: HWND,
    /// Each circle has its own host: `WindowHost` keeps per-window "raised" state.
    pub host: Rc<dyn DesktopHost>,
    pub which: Which,
}

#[derive(Clone, Copy)]
struct Press {
    hwnd: HWND,
    cursor: POINT,
    origin: (i32, i32),
    moving: bool,
}

thread_local! {
    static PRESS: Cell<Option<Press>> = const { Cell::new(None) };
    /// The circle window that currently has `TrackMouseEvent` hover/leave tracking.
    static TRACKING: Cell<HWND> = const { Cell::new(std::ptr::null_mut()) };
}

pub fn register_class() {
    let name = win::wide(CLASS);
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: Some(circle_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: unsafe { GetModuleHandleW(std::ptr::null()) },
        hIcon: std::ptr::null_mut(),
        hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_HAND) },
        hbrBackground: std::ptr::null_mut(),
        lpszMenuName: std::ptr::null(),
        lpszClassName: name.as_ptr(),
        hIconSm: std::ptr::null_mut(),
    };
    unsafe { RegisterClassExW(&wc) };
}

/// Creates the (still invisible) window for `which`; `render` sizes, paints and shows it.
pub fn create(which: Which, pos: (i32, i32)) -> Option<CircleWin> {
    let class = win::wide(CLASS);
    let title = win::wide("DeskTuk Circle");
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            pos.0,
            pos.1,
            1,
            1,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        dlog!("circle window creation failed: error {}", unsafe { windows_sys::Win32::Foundation::GetLastError() });
        return None;
    }
    unsafe { DragAcceptFiles(hwnd, 1) };
    let host = host::create(HostKind::Window);
    host.attached(hwnd);
    Some(CircleWin { hwnd, host, which })
}

/// Paints `which` into `hwnd` (name + item count), moves it to its saved position
/// (clamped to the nearest work area) and shows it. Uses a temporary surface: the
/// system keeps the presented pixels, so nothing stays allocated here.
pub fn render(hwnd: HWND, which: Which) {
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let m = geom::metrics(dpi);
    let recent_count = if which == Which::Recent { super::recent_paths().len() } else { 0 };
    let (name, count, pos) = {
        let Ok(b) = app().board.try_borrow() else { return };
        match which {
            Which::Recent => ("Recent".to_string(), recent_count, b.recent_pos.unwrap_or_else(super::default_recent_pos)),
            Which::User(ci) => match b.circles.get(ci) {
                Some(c) => (c.name.clone(), view::circle_count(&b, ci), c.pos),
                None => return,
            },
        }
    };
    let Some(mut s) = Surface::new(m.circle_d, m.circle_d) else { return };
    draw::circle(&s, dpi, &name, count, which == Which::Recent);
    let d = m.circle_d as usize;
    geom::mask_disc(s.pixels(), d, d, m.circle_d as f32 / 2.0);
    let (x, y) = clamp_to_work(pos, m.circle_d);
    s.present(hwnd, POINT { x, y });
    unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
}

/// Keeps a `d`x`d` window at `pos` fully inside the nearest monitor's work area.
pub fn clamp_to_work(pos: (i32, i32), d: i32) -> (i32, i32) {
    let r = win::rect(pos.0, pos.1, pos.0 + d, pos.1 + d);
    unsafe {
        let mon = MonitorFromRect(&r, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(mon, &mut mi) == 0 {
            return pos;
        }
        let w = mi.rcWork;
        (pos.0.min(w.right - d).max(w.left), pos.1.min(w.bottom - d).max(w.top))
    }
}

fn cursor_pos() -> POINT {
    let mut p = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut p) };
    p
}

/// Paths from a `WM_DROPFILES` handle (which is released).
pub fn dropped_paths(h: HDROP) -> Vec<PathBuf> {
    let mut out = Vec::new();
    unsafe {
        let n = DragQueryFileW(h, u32::MAX, std::ptr::null_mut(), 0);
        for i in 0..n {
            let len = DragQueryFileW(h, i, std::ptr::null_mut(), 0) as usize;
            let mut buf = vec![0u16; len + 1];
            let got = DragQueryFileW(h, i, buf.as_mut_ptr(), buf.len() as u32) as usize;
            if got > 0 {
                out.push(PathBuf::from(OsString::from_wide(&buf[..got])));
            }
        }
        DragFinish(h);
    }
    out
}

fn on_move(hwnd: HWND) {
    if let Some(mut p) = PRESS.get().filter(|p| p.hwnd == hwnd) {
        let c = cursor_pos();
        let (dx, dy) = (c.x - p.cursor.x, c.y - p.cursor.y);
        let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
        let beyond = unsafe { dx.abs() > GetSystemMetricsForDpi(SM_CXDRAG, dpi) || dy.abs() > GetSystemMetricsForDpi(SM_CYDRAG, dpi) };
        if !p.moving && beyond {
            p.moving = true;
            ring::close_now();
        }
        if p.moving {
            unsafe {
                SetWindowPos(hwnd, std::ptr::null_mut(), p.origin.0 + dx, p.origin.1 + dy, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE)
            };
        }
        PRESS.set(Some(p));
        return;
    }
    if TRACKING.get() != hwnd {
        let mut tme = TRACKMOUSEEVENT {
            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_HOVER | TME_LEAVE,
            hwndTrack: hwnd,
            dwHoverTime: HOVER_MS,
        };
        unsafe { TrackMouseEvent(&mut tme) };
        TRACKING.set(hwnd);
    }
}

/// Ends a left-button press. `released`: the button went up (a click opens the ring);
/// otherwise capture was lost and nothing is opened.
fn end_press(hwnd: HWND, released: bool) {
    let Some(p) = PRESS.take() else { return };
    if unsafe { GetCapture() } == hwnd {
        unsafe { ReleaseCapture() }; // sends WM_CAPTURECHANGED; PRESS is already empty
    }
    let Some(which) = super::which_of(hwnd) else { return };
    if p.moving {
        let r = win::window_rect(hwnd);
        super::moved(which, (r.left, r.top));
    } else if released {
        ring::open(hwnd, which.target());
    }
}

unsafe extern "system" fn circle_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        // Never take activation: a circle must not jump above application windows.
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        WM_WINDOWPOSCHANGING => {
            if let Some(h) = super::host_of(hwnd) {
                h.window_pos_changing(hwnd, &mut *(lparam as *mut WINDOWPOS));
            }
            0
        }
        WM_MOUSEMOVE => {
            on_move(hwnd);
            0
        }
        WM_MOUSEHOVER => {
            TRACKING.set(std::ptr::null_mut());
            if PRESS.get().is_none() && !ring::is_open_for(hwnd) {
                if let Some(w) = super::which_of(hwnd) {
                    ring::open(hwnd, w.target());
                }
            }
            0
        }
        WM_MOUSELEAVE => {
            if TRACKING.get() == hwnd {
                TRACKING.set(std::ptr::null_mut());
            }
            0
        }
        WM_LBUTTONDOWN => {
            let r = win::window_rect(hwnd);
            PRESS.set(Some(Press { hwnd, cursor: cursor_pos(), origin: (r.left, r.top), moving: false }));
            SetCapture(hwnd);
            0
        }
        WM_LBUTTONUP => {
            end_press(hwnd, true);
            0
        }
        WM_CAPTURECHANGED => {
            if lparam as HWND != hwnd {
                end_press(hwnd, false);
            }
            0
        }
        WM_RBUTTONUP => {
            if let Some(w) = super::which_of(hwnd) {
                super::circle_menu(w, cursor_pos());
            }
            0
        }
        WM_DROPFILES => {
            let paths = dropped_paths(wparam as HDROP);
            if let Some(w) = super::which_of(hwnd) {
                dlog!("dropped {} file(s) on {w:?}", paths.len());
                super::assign(&paths, view::drop_place(w.target(), None));
            }
            0
        }
        WM_DPICHANGED => {
            if let Some(w) = super::which_of(hwnd) {
                render(hwnd, w);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
```

- [x] **Step 2: Create `src/circles/ring.rs`:**

```rust
//! The open ring: at most one layered window, owned by the circle it opened from (so it
//! always stays just above that circle). It shows one page of slots, reveals itself with
//! a short animation, follows the wheel, and closes when the pointer leaves the disc.
//!
//! State lives in `RING`. Every access goes through `with`, which uses `try_borrow_mut`
//! and simply skips the message if the state is busy. Nothing that can pump messages,
//! open a menu or destroy a window runs inside `with` - those calls happen after it
//! returns. (`UpdateLayeredWindow` does run inside: it only sends `WM_WINDOWPOS*`, which
//! `ring_proc` passes to `DefWindowProcW` without touching `RING`.)

use std::cell::RefCell;
use std::path::PathBuf;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::WM_MOUSELEAVE;
use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetCapture, ReleaseCapture, SetCapture, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
};
use windows_sys::Win32::UI::Shell::{DragAcceptFiles, HDROP};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use super::circle_win::{self, Which};
use super::draw::{self, RingView, SlotLook, SlotView};
use super::geom::{self, RingHit};
use super::suggest::Suggestion;
use super::surface::Surface;
use super::view::{self, SlotRef, Target};
use crate::app::app;
use crate::items::{self, Item, Meta};
use crate::{diag, dlog, shell_menu, win};

const CLASS: &str = "DeskTuk.Ring";
const TIMER_ANIM: usize = 1;
/// Animation frame interval (only while animating; no timer runs when the ring is still).
const FRAME_MS: u32 = 15;
const IDM_REMOVE: usize = 0x8001;
const IDM_OPEN: usize = 1;
const IDM_REMOVE_PLAIN: usize = 2;
const IDM_ACCEPT: usize = 3;
const IDM_DISMISS: usize = 4;
const IDM_RENAME_GROUP: usize = 5;
const IDM_DELETE_GROUP: usize = 6;

struct PageSlot {
    r: SlotRef,
    /// Shell metadata/icon for `SlotRef::Item` slots.
    item: Option<Item>,
}

#[derive(Clone, Copy)]
struct Anim {
    t0: f64,
    from: f32,
    to: f32,
    closing: bool,
}

/// Left button pressed on an item slot; becomes a drag once it moves far enough.
struct Press {
    path: PathBuf,
    cursor: POINT,
    active: bool,
}

struct Ring {
    hwnd: HWND,
    anchor: HWND,
    target: Target,
    refs: Vec<SlotRef>,
    suggestions: Vec<Suggestion>,
    start: usize,
    page: Vec<PageSlot>,
    hover: Option<usize>,
    wheel: i32,
    dpi: u32,
    /// Screen position of the ring centre (= the circle centre).
    center: POINT,
    radius: f32,
    anim: Option<Anim>,
    /// Fully drawn and masked frame, kept only while animating.
    full: Option<Vec<u32>>,
    surface: Surface,
    menu_open: bool,
    tracking: bool,
    press: Option<Press>,
}

thread_local! {
    static RING: RefCell<Option<Ring>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Ring) -> R) -> Option<R> {
    RING.with(|cell| {
        let mut g = cell.try_borrow_mut().ok()?;
        g.as_mut().map(f)
    })
}

pub fn register_class() {
    let name = win::wide(CLASS);
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: Some(ring_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: unsafe { GetModuleHandleW(std::ptr::null()) },
        hIcon: std::ptr::null_mut(),
        hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) },
        hbrBackground: std::ptr::null_mut(),
        lpszMenuName: std::ptr::null(),
        lpszClassName: name.as_ptr(),
        hIconSm: std::ptr::null_mut(),
    };
    unsafe { RegisterClassExW(&wc) };
}

fn which_of(t: Target) -> Which {
    match t {
        Target::Recent => Which::Recent,
        Target::Circle(ci) | Target::Group(ci, _) => Which::User(ci),
    }
}

fn top_left(r: &Ring) -> POINT {
    let m = geom::metrics(r.dpi);
    POINT { x: r.center.x - m.ring_r, y: r.center.y - m.ring_r }
}

fn closing(r: &Ring) -> bool {
    r.anim.map(|a| a.closing).unwrap_or(false)
}

/// Whether a ring (not already closing) is open for the circle window `anchor`.
pub fn is_open_for(anchor: HWND) -> bool {
    with(|r| r.anchor == anchor && !closing(r)).unwrap_or(false)
}

/// Opens the ring for `target` over the circle window `anchor`, closing any other ring.
pub fn open(anchor: HWND, target: Target) {
    close_now();
    let dpi = unsafe { GetDpiForWindow(anchor) }.max(96);
    let m = geom::metrics(dpi);
    let wr = win::window_rect(anchor);
    let size = 2 * m.ring_r;
    // Centred on the circle, but shifted inwards near a screen edge so every slot stays reachable.
    let (cx, cy) = ((wr.left + wr.right) / 2, (wr.top + wr.bottom) / 2);
    let (left, top) = circle_win::clamp_to_work((cx - m.ring_r, cy - m.ring_r), size);
    let center = POINT { x: left + m.ring_r, y: top + m.ring_r };
    let Some(surface) = Surface::new(size, size) else { return };
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            win::wide(CLASS).as_ptr(),
            win::wide("DeskTuk Ring").as_ptr(),
            WS_POPUP,
            center.x - m.ring_r,
            center.y - m.ring_r,
            size,
            size,
            anchor, // owner: the ring always stays just above its circle
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        return;
    }
    unsafe { DragAcceptFiles(hwnd, 1) };
    let (refs, suggestions) = super::slots_for(target);
    let r0 = m.circle_d as f32 / 2.0;
    let mut ring = Ring {
        hwnd,
        anchor,
        target,
        refs,
        suggestions,
        start: 0,
        page: Vec::new(),
        hover: None,
        wheel: 0,
        dpi,
        center,
        radius: r0,
        anim: Some(Anim { t0: diag::since_start_ms(), from: r0, to: m.ring_r as f32, closing: false }),
        full: None,
        surface,
        menu_open: false,
        tracking: false,
        press: None,
    };
    let to_load = load_page(&mut ring);
    let slots = ring.refs.len();
    let stored = RING.with(|cell| match cell.try_borrow_mut() {
        Ok(mut g) => {
            *g = Some(ring);
            true
        }
        Err(_) => false,
    });
    if !stored {
        unsafe { DestroyWindow(hwnd) };
        return;
    }
    with(redraw);
    items::spawn_loader(app().controller.get(), to_load);
    unsafe {
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        SetTimer(hwnd, TIMER_ANIM, FRAME_MS, None);
    }
    dlog!("ring opened target={target:?} slots={slots}");
}

/// Destroys the ring immediately (no animation). Safe to call when none is open.
pub fn close_now() {
    let taken = RING.with(|cell| cell.try_borrow_mut().ok().and_then(|mut g| g.take()));
    if let Some(r) = taken {
        let h = r.hwnd;
        drop(r); // releases icons and the surface
        unsafe {
            KillTimer(h, TIMER_ANIM);
            DestroyWindow(h);
            // Let COM unload shell extensions the icons pulled in, then trim the working set.
            windows_sys::Win32::System::Com::CoFreeUnusedLibrariesEx(0, 0);
        }
        crate::app::schedule_trim();
    }
}

fn begin_close() {
    with(|r| {
        if closing(r) {
            return;
        }
        let m = geom::metrics(r.dpi);
        if r.full.is_none() {
            r.full = Some(r.surface.pixels().to_vec());
        }
        r.anim = Some(Anim { t0: diag::since_start_ms(), from: r.radius, to: m.circle_d as f32 / 2.0, closing: true });
        unsafe { SetTimer(r.hwnd, TIMER_ANIM, FRAME_MS, None) };
    });
}

/// Re-reads the slots (after the board or the Desktop changed) and repaints.
pub fn refresh() {
    let to_load = with(|r| {
        let (refs, suggestions) = super::slots_for(r.target);
        r.suggestions = suggestions;
        let mut to_load = Vec::new();
        if refs != r.refs {
            r.refs = refs;
            to_load = load_page(r);
        }
        if !closing(r) {
            redraw(r);
        }
        to_load
    });
    if let Some(paths) = to_load {
        items::spawn_loader(app().controller.get(), paths);
    }
}

/// Shell metadata from the loader thread for items on the current page.
pub fn apply_metas(metas: Vec<(PathBuf, Meta)>) {
    with(|r| {
        let mut any = false;
        for (p, m) in metas {
            if let Some(it) = r.page.iter_mut().filter_map(|s| s.item.as_mut()).find(|i| i.path == p) {
                it.apply(m);
                any = true;
            }
        }
        if any && !closing(r) {
            redraw(r);
        }
    });
}

/// Builds `page` for the current `start`; returns the item paths whose metadata must load.
fn load_page(r: &mut Ring) -> Vec<PathBuf> {
    let range = geom::page_range(r.refs.len(), r.start);
    r.start = range.start;
    r.hover = None;
    r.page = r.refs[range]
        .iter()
        .map(|sr| PageSlot {
            r: sr.clone(),
            item: match sr {
                SlotRef::Item(p) => Some(Item::placeholder(p.clone())),
                _ => None,
            },
        })
        .collect();
    r.page.iter().filter_map(|s| s.item.as_ref().map(|i| i.path.clone())).collect()
}

fn set_target(r: &mut Ring, t: Target) -> Vec<PathBuf> {
    r.target = t;
    let (refs, suggestions) = super::slots_for(t);
    r.refs = refs;
    r.suggestions = suggestions;
    r.start = 0;
    let to_load = load_page(r);
    redraw(r);
    to_load
}

fn subtitle(r: &Ring) -> String {
    let label = geom::page_label(r.refs.len(), r.start);
    if !label.is_empty() {
        return label;
    }
    if r.refs.is_empty() {
        let no_circles = app().board.try_borrow().map(|b| b.circles.is_empty()).unwrap_or(false);
        return if r.target == Target::Recent && no_circles { "Right-click: New circle".into() } else { "Drop files here".into() };
    }
    String::new()
}

fn render(r: &mut Ring) {
    let m = geom::metrics(r.dpi);
    let mut labels: Vec<String> = Vec::with_capacity(r.page.len());
    let mut looks: Vec<SlotLook> = Vec::with_capacity(r.page.len());
    {
        let board = app().board.try_borrow().ok();
        let circle_name = |ci: usize| board.as_ref().and_then(|b| b.circles.get(ci)).map(|c| c.name.clone()).unwrap_or_default();
        for s in r.page.iter_mut() {
            match (&s.r, s.item.as_mut()) {
                (SlotRef::Item(_), Some(it)) => {
                    labels.push(it.name.clone());
                    looks.push(SlotLook::Item { icon: it.icon(m.icon), dim: it.missing });
                }
                (SlotRef::Group(gi), _) => {
                    let ci = match r.target {
                        Target::Circle(ci) => ci,
                        _ => usize::MAX,
                    };
                    let name = board.as_ref().and_then(|b| b.circles.get(ci)).and_then(|c| c.groups.get(*gi)).map(|g| g.name.clone());
                    labels.push(name.unwrap_or_default());
                    looks.push(SlotLook::Group);
                }
                (SlotRef::Back, _) => {
                    labels.push("Back".into());
                    looks.push(SlotLook::Back);
                }
                (SlotRef::Suggestion(i), _) => match r.suggestions.get(*i) {
                    Some(sg) => {
                        labels.push(format!(".{} \u{2192} {}", sg.ext, circle_name(sg.circle)));
                        looks.push(SlotLook::Suggestion { count: sg.paths.len() });
                    }
                    None => {
                        labels.push(String::new());
                        looks.push(SlotLook::Suggestion { count: 0 });
                    }
                },
                (SlotRef::Item(_), None) => {
                    labels.push(String::new());
                    looks.push(SlotLook::Item { icon: std::ptr::null_mut(), dim: true });
                }
            }
        }
    }
    let title = app().board.try_borrow().map(|b| view::title(&b, r.target)).unwrap_or_default();
    let sub = subtitle(r);
    let views: Vec<SlotView> = labels.iter().zip(looks).map(|(l, look)| SlotView { label: l, look }).collect();
    draw::ring(
        &r.surface,
        r.dpi,
        &RingView { title: &title, subtitle: &sub, slots: &views, hover: r.hover, recent: r.target == Target::Recent },
    );
    let size = r.surface.w as usize;
    let px = r.surface.pixels();
    geom::mask_disc(px, size, size, m.ring_r as f32);
    if r.anim.is_some() {
        let full = px.to_vec();
        geom::reveal(&full, px, size, size, r.radius);
        r.full = Some(full);
    } else {
        r.full = None;
    }
}

fn redraw(r: &mut Ring) {
    render(r);
    r.surface.present(r.hwnd, top_left(r));
}

fn tick() {
    let now = diag::since_start_ms();
    let closed = with(|r| {
        let Some(a) = r.anim else {
            unsafe { KillTimer(r.hwnd, TIMER_ANIM) };
            return false;
        };
        let dur = if a.closing { geom::CLOSE_MS } else { geom::OPEN_MS };
        let (rad, done) = geom::anim_radius(a.from, a.to, now - a.t0, dur);
        r.radius = rad;
        if done {
            r.anim = None;
            unsafe { KillTimer(r.hwnd, TIMER_ANIM) };
            if a.closing {
                return true;
            }
        }
        let size = r.surface.w as usize;
        if let Some(full) = r.full.take() {
            let px = r.surface.pixels();
            if r.anim.is_some() {
                geom::reveal(&full, px, size, size, rad);
                r.full = Some(full);
            } else {
                px.copy_from_slice(&full); // finished opening: keep only the surface
            }
        }
        r.surface.present(r.hwnd, top_left(r));
        false
    })
    .unwrap_or(false);
    if closed {
        close_now();
    }
}

/// What is under client point `(x, y)` of the ring window.
fn hit(r: &Ring, x: i32, y: i32) -> RingHit {
    let m = geom::metrics(r.dpi);
    geom::ring_hit(&m, r.page.len(), x - m.ring_r, y - m.ring_r)
}

fn hit_screen(r: &Ring, p: POINT) -> RingHit {
    let tl = top_left(r);
    hit(r, p.x - tl.x, p.y - tl.y)
}

fn cursor_pos() -> POINT {
    let mut p = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut p) };
    p
}

/// Starts closing if the pointer is no longer over the ring (after a menu or a drag).
fn close_if_outside() {
    let outside = with(|r| hit_screen(r, cursor_pos()) == RingHit::Outside).unwrap_or(false);
    if outside {
        begin_close();
    }
}

fn on_mouse_move(lparam: LPARAM) {
    let (x, y) = (win::x_lparam(lparam), win::y_lparam(lparam));
    with(|r| {
        if closing(r) {
            return;
        }
        if !r.tracking {
            let mut tme = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: r.hwnd,
                dwHoverTime: 0,
            };
            unsafe { TrackMouseEvent(&mut tme) };
            r.tracking = true;
        }
        let dpi = r.dpi;
        if let Some(p) = r.press.as_mut() {
            let c = cursor_pos();
            let (dx, dy) = (c.x - p.cursor.x, c.y - p.cursor.y);
            if !p.active
                && unsafe { dx.abs() > GetSystemMetricsForDpi(SM_CXDRAG, dpi) || dy.abs() > GetSystemMetricsForDpi(SM_CYDRAG, dpi) }
            {
                p.active = true;
            }
            if p.active {
                unsafe { SetCursor(LoadCursorW(std::ptr::null_mut(), IDC_SIZEALL)) };
            }
            return;
        }
        let hover = match hit(r, x, y) {
            RingHit::Slot(k) => Some(k),
            _ => None,
        };
        if hover != r.hover {
            r.hover = hover;
            redraw(r);
        }
    });
}

/// Something the ring must do after its state borrow has ended.
enum Action {
    None,
    Open(PathBuf),
    Load(Vec<PathBuf>),
    Accept(Suggestion),
    Drop { path: PathBuf, at: POINT },
}

fn on_left_down(lparam: LPARAM) {
    let (x, y) = (win::x_lparam(lparam), win::y_lparam(lparam));
    let capture = with(|r| {
        if closing(r) {
            return None;
        }
        if let RingHit::Slot(k) = hit(r, x, y) {
            if let SlotRef::Item(p) = &r.page[k].r {
                r.press = Some(Press { path: p.clone(), cursor: cursor_pos(), active: false });
                return Some(r.hwnd);
            }
        }
        None
    })
    .flatten();
    if let Some(h) = capture {
        unsafe { SetCapture(h) };
    }
}

fn on_left_up(lparam: LPARAM) {
    let (x, y) = (win::x_lparam(lparam), win::y_lparam(lparam));
    let (press, hwnd) = with(|r| (r.press.take(), r.hwnd)).unwrap_or((None, std::ptr::null_mut()));
    if !hwnd.is_null() && unsafe { GetCapture() } == hwnd {
        unsafe { ReleaseCapture() };
    }
    let action = match press {
        Some(p) if p.active => Action::Drop { path: p.path, at: cursor_pos() },
        _ => with(|r| {
            if closing(r) {
                return Action::None;
            }
            match hit(r, x, y) {
                RingHit::Slot(k) => match r.page[k].r.clone() {
                    SlotRef::Item(p) => Action::Open(p),
                    SlotRef::Group(gi) => match r.target {
                        Target::Circle(ci) => Action::Load(set_target(r, Target::Group(ci, gi))),
                        _ => Action::None,
                    },
                    SlotRef::Back => match r.target {
                        Target::Group(ci, _) => Action::Load(set_target(r, Target::Circle(ci))),
                        _ => Action::None,
                    },
                    SlotRef::Suggestion(i) => r.suggestions.get(i).cloned().map(Action::Accept).unwrap_or(Action::None),
                },
                // Clicking the centre turns the page (wraps to the first page at the end);
                // useful when "scroll inactive windows" is switched off.
                RingHit::Center if r.refs.len() > geom::PER_PAGE => {
                    let next = geom::scroll(r.refs.len(), r.start, 1);
                    r.start = if next == r.start { 0 } else { next };
                    let to_load = load_page(r);
                    redraw(r);
                    Action::Load(to_load)
                }
                _ => Action::None,
            }
        })
        .unwrap_or(Action::None),
    };
    run(action);
}

fn run(action: Action) {
    match action {
        Action::None => {}
        Action::Open(p) => {
            close_now();
            let _ = items::open(&p, app().controller.get());
        }
        Action::Load(paths) => items::spawn_loader(app().controller.get(), paths),
        Action::Accept(s) => super::accept_suggestion(&s),
        Action::Drop { path, at } => {
            finish_drag(path, at);
            close_if_outside();
        }
    }
}

/// An item dragged out of a slot was released at `at` (screen). Dropping it on a group
/// slot, the Back slot, another circle or the Recent circle moves it there.
fn finish_drag(path: PathBuf, at: POINT) {
    let under = unsafe { WindowFromPoint(at) };
    let own = with(|r| (r.hwnd == under).then(|| (r.target, hit_screen(r, at), r.page.iter().map(|s| s.r.clone()).collect::<Vec<_>>())));
    let place = match own.flatten() {
        Some((target, RingHit::Slot(k), refs)) => Some(view::drop_place(target, refs.get(k))),
        Some(_) => None, // released on the ring itself but not on a slot: nothing to do
        None => super::which_of(under).map(|w| view::drop_place(w.target(), None)),
    };
    if let Some(place) = place {
        super::assign(&[path], place);
    }
}

/// Runs `f` (a menu) with the ring marked busy so leaving it does not close it.
fn with_menu(f: impl FnOnce()) {
    with(|r| r.menu_open = true);
    f();
    with(|r| r.menu_open = false);
    close_if_outside();
}

fn on_right_up(lparam: LPARAM) {
    let (x, y) = (win::x_lparam(lparam), win::y_lparam(lparam));
    let info = with(|r| {
        if closing(r) {
            return None;
        }
        let slot = match hit(r, x, y) {
            RingHit::Slot(k) => Some(r.page[k].r.clone()),
            _ => None,
        };
        Some((r.target, slot, r.suggestions.clone()))
    })
    .flatten();
    let Some((target, slot, suggestions)) = info else { return };
    let at = cursor_pos();
    with_menu(|| match slot {
        Some(SlotRef::Item(path)) => item_menu(target, &path, at),
        Some(SlotRef::Group(gi)) => {
            if let Target::Circle(ci) = target {
                match super::popup(&[(IDM_RENAME_GROUP, "&Rename group\u{2026}"), (IDM_DELETE_GROUP, "&Delete group")], at) {
                    IDM_RENAME_GROUP => super::begin_rename(super::rename::RenameTarget::Group(ci, gi)),
                    IDM_DELETE_GROUP => super::delete_group(ci, gi),
                    _ => {}
                }
            }
        }
        Some(SlotRef::Suggestion(i)) => {
            if let Some(s) = suggestions.get(i) {
                let accept = format!("&Add {} file(s) to this circle", s.paths.len());
                match super::popup(&[(IDM_ACCEPT, accept.as_str()), (IDM_DISMISS, "&Not now")], at) {
                    IDM_ACCEPT => super::accept_suggestion(s),
                    IDM_DISMISS => super::dismiss_suggestion(s),
                    _ => {}
                }
            }
        }
        Some(SlotRef::Back) => {}
        None => super::circle_menu(which_of(target), at),
    });
}

fn item_menu(target: Target, path: &std::path::Path, at: POINT) {
    let removable = target != Target::Recent;
    let extras: &[(usize, &str)] = if removable { &[(IDM_REMOVE, "Remove from circle")] } else { &[] };
    match shell_menu::show(app().controller.get(), path, at, extras) {
        shell_menu::Choice::Extra(IDM_REMOVE) => super::assign(&[path.to_path_buf()], None),
        shell_menu::Choice::Failed => {
            let mut entries = vec![(IDM_OPEN, "&Open")];
            if removable {
                entries.push((IDM_REMOVE_PLAIN, "&Remove from circle"));
            }
            match super::popup(&entries, at) {
                IDM_OPEN => {
                    let _ = items::open(path, app().controller.get());
                }
                IDM_REMOVE_PLAIN => super::assign(&[path.to_path_buf()], None),
                _ => {}
            }
        }
        _ => {}
    }
}

fn on_wheel(wparam: WPARAM) {
    let delta = ((wparam >> 16) & 0xffff) as u16 as i16 as i32;
    let to_load = with(|r| {
        if closing(r) {
            return None;
        }
        r.wheel += delta;
        let (pages, rest) = geom::wheel_pages(r.wheel);
        r.wheel = rest;
        if pages == 0 {
            return None;
        }
        let s = geom::scroll(r.refs.len(), r.start, pages);
        if s == r.start {
            return None;
        }
        r.start = s;
        let to_load = load_page(r);
        redraw(r);
        Some(to_load)
    })
    .flatten();
    if let Some(paths) = to_load {
        items::spawn_loader(app().controller.get(), paths);
    }
}

fn on_drop_files(wparam: WPARAM) {
    let paths = circle_win::dropped_paths(wparam as HDROP);
    let place = with(|r| {
        let slot = match hit_screen(r, cursor_pos()) {
            RingHit::Slot(k) => Some(r.page[k].r.clone()),
            _ => None,
        };
        view::drop_place(r.target, slot.as_ref())
    });
    if let Some(place) = place {
        super::assign(&paths, place);
    }
}

unsafe extern "system" fn ring_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        WM_MOUSEMOVE => {
            on_mouse_move(lparam);
            0
        }
        WM_MOUSELEAVE => {
            let close = with(|r| {
                r.tracking = false;
                !r.menu_open && r.press.is_none()
            })
            .unwrap_or(false);
            if close {
                begin_close();
            }
            0
        }
        WM_LBUTTONDOWN => {
            on_left_down(lparam);
            0
        }
        WM_LBUTTONUP => {
            on_left_up(lparam);
            0
        }
        WM_CAPTURECHANGED => {
            // Capture taken away mid-drag (e.g. Alt+Tab): forget the drag.
            if lparam as HWND != hwnd {
                with(|r| r.press = None);
            }
            0
        }
        WM_RBUTTONUP => {
            on_right_up(lparam);
            0
        }
        WM_MOUSEWHEEL => {
            on_wheel(wparam);
            0
        }
        WM_DROPFILES => {
            on_drop_files(wparam);
            0
        }
        WM_TIMER if wparam == TIMER_ANIM => {
            tick();
            0
        }
        WM_DESTROY => {
            // Destroyed from outside (its owner circle went away): drop the state.
            RING.with(|cell| {
                if let Ok(mut g) = cell.try_borrow_mut() {
                    if g.as_ref().map(|r| r.hwnd == hwnd).unwrap_or(false) {
                        *g = None;
                    }
                }
            });
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
```

- [x] **Step 3: Create `src/circles/rename.rs`:**

```rust
//! Inline rename box: a small ordinary (non-layered) popup with an EDIT control, shown
//! over a circle. Enter or clicking elsewhere commits, Esc cancels.
//!
//! Layered windows painted with `UpdateLayeredWindow` cannot show child controls, which
//! is why this is a separate window.

use std::cell::Cell;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{CreateFontIndirectW, DeleteObject, GetSysColorBrush, COLOR_WINDOW, LOGFONTW};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::EM_SETSEL;
use windows_sys::Win32::UI::HiDpi::{GetDpiForSystem, SystemParametersInfoForDpi};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{SetFocus, VK_ESCAPE, VK_RETURN};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use super::model::MAX_NAME;
use crate::win;

const CLASS: &str = "DeskTuk.Rename";
/// Posted to the popup; `wparam` 1 = commit, 0 = cancel.
const WM_APP_RENAME_DONE: u32 = WM_APP + 50;
const EM_LIMITTEXT: u32 = 0x00C5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenameTarget {
    Circle(usize),
    Group(usize, usize),
}

thread_local! {
    /// (popup, edit, target) while a rename box is open.
    static ACTIVE: Cell<Option<(HWND, HWND, RenameTarget)>> = const { Cell::new(None) };
}

pub fn register_class() {
    let name = win::wide(CLASS);
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: Some(popup_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: unsafe { GetModuleHandleW(std::ptr::null()) },
        hIcon: std::ptr::null_mut(),
        hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) },
        hbrBackground: unsafe { GetSysColorBrush(COLOR_WINDOW) },
        lpszMenuName: std::ptr::null(),
        lpszClassName: name.as_ptr(),
        hIconSm: std::ptr::null_mut(),
    };
    unsafe { RegisterClassExW(&wc) };
}

/// Opens a rename box centred on `anchor` (screen rect) with `initial` selected.
pub fn begin(target: RenameTarget, anchor: RECT, initial: &str) {
    cancel();
    let dpi = unsafe { GetDpiForSystem() }.max(96);
    let (w, h, pad) = (win::scale(220, dpi), win::scale(30, dpi), win::scale(4, dpi));
    let cx = (anchor.left + anchor.right) / 2;
    let cy = (anchor.top + anchor.bottom) / 2;
    let hinst = unsafe { GetModuleHandleW(std::ptr::null()) };
    unsafe {
        let popup = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            win::wide(CLASS).as_ptr(),
            win::wide("Rename").as_ptr(),
            WS_POPUP | WS_BORDER,
            cx - w / 2,
            cy - h / 2,
            w,
            h,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinst,
            std::ptr::null(),
        );
        if popup.is_null() {
            return;
        }
        let mut cr = win::rect(0, 0, 0, 0);
        GetClientRect(popup, &mut cr);
        let edit = CreateWindowExW(
            0,
            win::wide("EDIT").as_ptr(),
            win::wide(initial).as_ptr(),
            WS_CHILD | WS_VISIBLE | ES_AUTOHSCROLL as u32,
            pad,
            pad,
            cr.right - 2 * pad,
            cr.bottom - 2 * pad,
            popup,
            1 as _,
            hinst,
            std::ptr::null(),
        );
        if edit.is_null() {
            DestroyWindow(popup);
            return;
        }
        let mut lf: LOGFONTW = std::mem::zeroed();
        SystemParametersInfoForDpi(
            SPI_GETICONTITLELOGFONT,
            std::mem::size_of::<LOGFONTW>() as u32,
            (&mut lf as *mut LOGFONTW).cast(),
            0,
            dpi,
        );
        let font = CreateFontIndirectW(&lf);
        SetWindowLongPtrW(popup, GWLP_USERDATA, font as isize); // deleted in WM_DESTROY
        SendMessageW(edit, WM_SETFONT, font as usize, 0);
        SendMessageW(edit, EM_LIMITTEXT, MAX_NAME, 0);
        SetWindowSubclass(edit, Some(edit_proc), 1, popup as usize);
        ACTIVE.set(Some((popup, edit, target)));
        ShowWindow(popup, SW_SHOW);
        SetForegroundWindow(popup);
        SetFocus(edit);
        SendMessageW(edit, EM_SETSEL, 0, -1);
    }
}

/// Closes an open rename box without committing.
pub fn cancel() {
    if let Some((popup, _, _)) = ACTIVE.take() {
        unsafe { DestroyWindow(popup) };
    }
}

fn window_text(h: HWND) -> String {
    unsafe {
        let n = GetWindowTextLengthW(h).max(0) as usize;
        let mut buf = vec![0u16; n + 1];
        let got = GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
        String::from_utf16_lossy(&buf[..got])
    }
}

fn finish(popup: HWND, commit: bool) {
    let Some((p, edit, target)) = ACTIVE.get() else { return };
    if p != popup {
        return;
    }
    ACTIVE.set(None);
    let text = window_text(edit);
    unsafe { DestroyWindow(popup) };
    if commit {
        super::commit_rename(target, &text);
    }
}

unsafe extern "system" fn edit_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM, _id: usize, popup: usize) -> LRESULT {
    match msg {
        WM_KEYDOWN if wparam == VK_RETURN as usize => {
            PostMessageW(popup as HWND, WM_APP_RENAME_DONE, 1, 0);
            0
        }
        WM_KEYDOWN if wparam == VK_ESCAPE as usize => {
            PostMessageW(popup as HWND, WM_APP_RENAME_DONE, 0, 0);
            0
        }
        // Swallow the characters of Enter/Esc so the edit control does not beep.
        WM_CHAR if wparam == 0x0d || wparam == 0x1b => 0,
        WM_KILLFOCUS => {
            PostMessageW(popup as HWND, WM_APP_RENAME_DONE, 1, 0);
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        WM_NCDESTROY => {
            RemoveWindowSubclass(hwnd, Some(edit_proc), 1);
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn popup_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_APP_RENAME_DONE => {
            finish(hwnd, wparam == 1);
            0
        }
        WM_DESTROY => {
            let font = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if font != 0 {
                DeleteObject(font as _);
            }
            if ACTIVE.get().map(|(p, _, _)| p == hwnd).unwrap_or(false) {
                ACTIVE.set(None);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
```

- [x] **Step 4: Replace `src/circles/mod.rs`:**

```rust
//! Phase 2 circles UI: round circles on the desktop that open into a ring of their files
//! on hover. Spec: `docs/superpowers/specs/2026-10-13-circles-design.md`.
//!
//! * `model`, `geom`, `view`, `suggest` and the pure half of `desktop` are plain logic
//!   with unit tests.
//! * `circle_win`, `ring`, `rename`, `surface`, `draw` are the Win32 side.
//! * This file owns the runtime state and the commands (assign, rename, delete, ...).
//!
//! RefCell rule (AGENTS.md): code here uses `try_borrow*` on shared state and copies what
//! it needs into locals before any call that can pump messages or destroy a window.

pub mod circle_win;
pub mod desktop;
pub mod draw;
pub mod geom;
pub mod model;
pub mod rename;
pub mod ring;
pub mod suggest;
pub mod surface;
pub mod view;

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use windows_sys::Win32::Foundation::{HANDLE, HWND, POINT};
use windows_sys::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::app::{app, TIMER_DESKTOP};
use crate::host::DesktopHost;
use crate::watch::Change;
use crate::{diag, dlog, win};
use circle_win::{CircleWin, Which};
use model::Place;
use suggest::Suggestion;
use view::{SlotRef, Target};

const IDM_NEW_CIRCLE: usize = 1;
const IDM_NEW_GROUP: usize = 2;
const IDM_RENAME: usize = 3;
const IDM_DELETE: usize = 4;
const IDM_EXIT: usize = 10;

/// A download or copy can fire several folder events in a row; re-list once they settle.
const DESKTOP_DEBOUNCE_MS: u32 = 250;

struct State {
    dirs: Vec<PathBuf>,
    entries: RefCell<Vec<desktop::Entry>>,
    /// `[0]` is Recent, then one window per `Board::circles` entry, in order.
    windows: RefCell<Vec<CircleWin>>,
    watcher: RefCell<Option<desktop::Watcher>>,
    /// Suggestions dismissed this session: (extension, circle index).
    dismissed: RefCell<Vec<(String, usize)>>,
}

thread_local! {
    static STATE: Cell<Option<&'static State>> = const { Cell::new(None) };
}

fn state() -> Option<&'static State> {
    STATE.with(|s| s.get())
}

/// Lists the Desktop, starts watching it and shows every circle.
pub fn start() {
    let t = diag::since_start_ms();
    let dirs = desktop::desktop_dirs();
    let entries = desktop::list(&dirs);
    let watcher = desktop::Watcher::new(&dirs);
    let n_entries = entries.len();
    let st: &'static State = Box::leak(Box::new(State {
        dirs,
        entries: RefCell::new(entries),
        windows: RefCell::new(Vec::new()),
        watcher: RefCell::new(Some(watcher)),
        dismissed: RefCell::new(Vec::new()),
    }));
    STATE.with(|s| s.set(Some(st)));
    circle_win::register_class();
    ring::register_class();
    rename::register_class();
    rebuild();
    let circles = app().board.try_borrow().map(|b| b.circles.len()).unwrap_or(0);
    dlog!("METRIC circles_started circles={circles} desktop_entries={n_entries} ms={:.1}", diag::since_start_ms() - t);
}

/// Destroys every circle window, the ring and the rename box; stops watching.
pub fn shutdown() {
    let Some(st) = state() else { return };
    ring::close_now();
    rename::cancel();
    let old: Vec<CircleWin> = st.windows.try_borrow_mut().map(|mut w| std::mem::take(&mut *w)).unwrap_or_default();
    for w in &old {
        unsafe { DestroyWindow(w.hwnd) };
    }
    if let Ok(mut w) = st.watcher.try_borrow_mut() {
        *w = None;
    }
}

/// Re-creates all circle windows from the board (after a circle was added or removed).
pub fn rebuild() {
    let Some(st) = state() else { return };
    ring::close_now();
    rename::cancel(); // circle indices are about to change
    let old: Vec<CircleWin> = st.windows.try_borrow_mut().map(|mut w| std::mem::take(&mut *w)).unwrap_or_default();
    for w in &old {
        unsafe { DestroyWindow(w.hwnd) };
    }
    drop(old);
    let (recent_pos, positions) = match app().board.try_borrow() {
        Ok(b) => (b.recent_pos, b.circles.iter().map(|c| c.pos).collect::<Vec<_>>()),
        Err(_) => return,
    };
    let mut new = Vec::new();
    if let Some(w) = circle_win::create(Which::Recent, recent_pos.unwrap_or_else(default_recent_pos)) {
        new.push(w);
    }
    for (ci, pos) in positions.into_iter().enumerate() {
        if let Some(w) = circle_win::create(Which::User(ci), pos) {
            new.push(w);
        }
    }
    let list: Vec<(HWND, Which)> = new.iter().map(|w| (w.hwnd, w.which)).collect();
    if let Ok(mut w) = st.windows.try_borrow_mut() {
        *w = new;
    }
    for (h, which) in list {
        circle_win::render(h, which);
    }
}

/// Repaints every circle (names, counts) and the open ring.
pub fn refresh_all() {
    let Some(st) = state() else { return };
    let list: Vec<(HWND, Which)> = match st.windows.try_borrow() {
        Ok(w) => w.iter().map(|w| (w.hwnd, w.which)).collect(),
        Err(_) => return,
    };
    for (h, which) in list {
        circle_win::render(h, which);
    }
    ring::refresh();
}

pub fn which_of(hwnd: HWND) -> Option<Which> {
    state()?.windows.try_borrow().ok()?.iter().find(|w| w.hwnd == hwnd).map(|w| w.which)
}

pub fn host_of(hwnd: HWND) -> Option<Rc<dyn DesktopHost>> {
    state()?.windows.try_borrow().ok()?.iter().find(|w| w.hwnd == hwnd).map(|w| w.host.clone())
}

fn window_of(which: Which) -> Option<HWND> {
    state()?.windows.try_borrow().ok()?.iter().find(|w| w.which == which).map(|w| w.hwnd)
}

/// The foreground window changed: each circle's host applies the "Show desktop" policy.
pub fn foreground_changed(fg: HWND) {
    let Some(st) = state() else { return };
    let list: Vec<(HWND, Rc<dyn DesktopHost>)> = match st.windows.try_borrow() {
        Ok(w) => w.iter().map(|w| (w.hwnd, w.host.clone())).collect(),
        Err(_) => return,
    };
    for (h, host) in list {
        host.foreground_changed(h, fg);
    }
}

/// Top-right of the primary work area, like the Phase 1 panel.
pub fn default_recent_pos() -> (i32, i32) {
    let mut work = win::rect(0, 0, 1024, 768);
    unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut work as *mut windows_sys::Win32::Foundation::RECT).cast(), 0) };
    let dpi = unsafe { GetDpiForSystem() }.max(96);
    let m = geom::metrics(dpi);
    (work.right - m.circle_d - win::scale(40, dpi), work.top + win::scale(40, dpi))
}

/// Desktop entries in no circle, newest first.
pub fn recent_paths() -> Vec<PathBuf> {
    let Some(st) = state() else { return Vec::new() };
    let (Ok(entries), Ok(board)) = (st.entries.try_borrow(), app().board.try_borrow()) else { return Vec::new() };
    desktop::recent(&entries, &board)
}

/// The slots of `target` (and, for Recent, the suggestions they refer to).
pub fn slots_for(target: Target) -> (Vec<SlotRef>, Vec<Suggestion>) {
    let recent = if target == Target::Recent { recent_paths() } else { Vec::new() };
    let Ok(board) = app().board.try_borrow() else { return (Vec::new(), Vec::new()) };
    let suggestions = match (target, state()) {
        (Target::Recent, Some(st)) => {
            let dismissed = st.dismissed.try_borrow().map(|d| d.clone()).unwrap_or_default();
            suggest::suggest(&board, &recent, &dismissed)
        }
        _ => Vec::new(),
    };
    (view::slot_refs(&board, target, &recent, &suggestions), suggestions)
}

/// Kernel handles the message loop waits on (empty when the circles UI is not running).
pub fn wait_handles() -> Vec<HANDLE> {
    state().and_then(|st| st.watcher.try_borrow().ok().and_then(|w| w.as_ref().map(|w| w.handles().to_vec()))).unwrap_or_default()
}

/// Desktop folder handle `i` was signalled: re-arm it and re-list after a short pause.
pub fn on_desktop_signal(i: usize) {
    let Some(st) = state() else { return };
    if let Ok(w) = st.watcher.try_borrow() {
        if let Some(w) = w.as_ref() {
            w.rearm(i);
        }
    }
    unsafe { SetTimer(app().controller.get(), TIMER_DESKTOP, DESKTOP_DEBOUNCE_MS, None) };
}

/// `TIMER_DESKTOP` fired: re-list the Desktop and repaint.
pub fn refresh_desktop() {
    let Some(st) = state() else { return };
    unsafe { KillTimer(app().controller.get(), TIMER_DESKTOP) };
    let entries = desktop::list(&st.dirs);
    dlog!("desktop changed: {} entries", entries.len());
    if let Ok(mut e) = st.entries.try_borrow_mut() {
        *e = entries;
    }
    refresh_all();
}

/// A shell rename: stored paths follow it.
pub fn on_shell_change(change: &Change) {
    let changed = app().board.try_borrow_mut().map(|mut b| b.follow(change)).unwrap_or(false);
    if changed {
        app().save_config();
        refresh_all();
    }
}

/// Moves `paths` to `to` (`None` = back to Recent), saves and repaints.
pub fn assign(paths: &[PathBuf], to: Option<Place>) {
    if paths.is_empty() {
        return;
    }
    {
        let Ok(mut b) = app().board.try_borrow_mut() else { return };
        for p in paths {
            match to {
                Some(place) => {
                    b.assign(p, place);
                }
                None => {
                    b.unassign(p);
                }
            }
        }
    }
    app().save_config();
    refresh_all();
}

/// A circle was dragged to `pos` (screen top-left).
pub fn moved(which: Which, pos: (i32, i32)) {
    {
        let Ok(mut b) = app().board.try_borrow_mut() else { return };
        match which {
            Which::Recent => b.recent_pos = Some(pos),
            Which::User(ci) => {
                if let Some(c) = b.circles.get_mut(ci) {
                    c.pos = pos;
                }
            }
        }
    }
    app().save_config();
}

pub fn accept_suggestion(s: &Suggestion) {
    dlog!("suggestion accepted: {} .{} file(s) -> circle {}", s.paths.len(), s.ext, s.circle);
    assign(&s.paths, Some(Place::Circle(s.circle)));
}

pub fn dismiss_suggestion(s: &Suggestion) {
    if let Some(st) = state() {
        if let Ok(mut d) = st.dismissed.try_borrow_mut() {
            d.push((s.ext.clone(), s.circle));
        }
    }
    ring::refresh();
}

/// A context menu owned by the hidden controller window (so no circle needs activation).
/// An id of 0 adds a separator. Returns the chosen id or 0.
pub fn popup(entries: &[(usize, &str)], at: POINT) -> usize {
    let ctl = app().controller.get();
    unsafe {
        let m = CreatePopupMenu();
        for &(id, label) in entries {
            if id == 0 {
                AppendMenuW(m, MF_SEPARATOR, 0, std::ptr::null());
            } else {
                AppendMenuW(m, MF_STRING, id, win::wide(label).as_ptr());
            }
        }
        SetForegroundWindow(ctl);
        let cmd = TrackPopupMenu(m, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY, at.x, at.y, 0, ctl, std::ptr::null()) as usize;
        PostMessageW(ctl, WM_NULL, 0, 0);
        DestroyMenu(m);
        cmd
    }
}

pub fn circle_menu(which: Which, at: POINT) {
    let mut entries = vec![(IDM_NEW_CIRCLE, "&New circle")];
    if let Which::User(_) = which {
        entries.extend([(IDM_NEW_GROUP, "New &group"), (IDM_RENAME, "&Rename\u{2026}"), (IDM_DELETE, "&Delete circle")]);
    }
    entries.extend([(0, ""), (IDM_EXIT, "E&xit DeskTuk")]);
    match (popup(&entries, at), which) {
        (IDM_NEW_CIRCLE, _) => new_circle(which),
        (IDM_NEW_GROUP, Which::User(ci)) => new_group(ci),
        (IDM_RENAME, Which::User(ci)) => begin_rename(rename::RenameTarget::Circle(ci)),
        (IDM_DELETE, Which::User(ci)) => delete_circle(ci),
        (IDM_EXIT, _) => crate::app::quit(),
        _ => {}
    }
}

/// Adds "New circle" to the left of `from` and starts renaming it.
fn new_circle(from: Which) {
    let anchor = window_of(from);
    let dpi = anchor.map(|h| unsafe { GetDpiForWindow(h) }).unwrap_or(96).max(96);
    let m = geom::metrics(dpi);
    let r = anchor.map(win::window_rect).unwrap_or_else(|| win::rect(400, 200, 400 + m.circle_d, 200 + m.circle_d));
    let pos = circle_win::clamp_to_work((r.left - m.circle_d - win::scale(24, dpi), r.top), m.circle_d);
    let ci = {
        let Ok(mut b) = app().board.try_borrow_mut() else { return };
        b.add_circle("New circle", pos)
    };
    app().save_config();
    rebuild();
    begin_rename(rename::RenameTarget::Circle(ci));
}

fn new_group(ci: usize) {
    let gi = {
        let Ok(mut b) = app().board.try_borrow_mut() else { return };
        b.add_group(ci, "New group")
    };
    let Some(gi) = gi else { return };
    app().save_config();
    refresh_all();
    begin_rename(rename::RenameTarget::Group(ci, gi));
}

pub fn begin_rename(target: rename::RenameTarget) {
    let ci = match target {
        rename::RenameTarget::Circle(ci) | rename::RenameTarget::Group(ci, _) => ci,
    };
    let initial = {
        let Ok(b) = app().board.try_borrow() else { return };
        match target {
            rename::RenameTarget::Circle(ci) => b.circles.get(ci).map(|c| c.name.clone()),
            rename::RenameTarget::Group(ci, gi) => b.circles.get(ci).and_then(|c| c.groups.get(gi)).map(|g| g.name.clone()),
        }
    };
    let (Some(initial), Some(h)) = (initial, window_of(Which::User(ci))) else { return };
    rename::begin(target, win::window_rect(h), &initial);
}

pub fn commit_rename(target: rename::RenameTarget, text: &str) {
    {
        let Ok(mut b) = app().board.try_borrow_mut() else { return };
        match target {
            rename::RenameTarget::Circle(ci) => b.rename_circle(ci, text),
            rename::RenameTarget::Group(ci, gi) => b.rename_group(ci, gi, text),
        }
    }
    app().save_config();
    refresh_all();
}

fn delete_circle(ci: usize) {
    let (name, count) = match app().board.try_borrow() {
        Ok(b) => match b.circles.get(ci) {
            Some(c) => (c.name.clone(), view::circle_count(&b, ci)),
            None => return,
        },
        Err(_) => return,
    };
    let text = format!("Delete the circle \"{name}\"?\n\nIts {count} item(s) go back to Recent. No files are changed or deleted.");
    let ok = unsafe {
        MessageBoxW(std::ptr::null_mut(), win::wide(&text).as_ptr(), win::wide("DeskTuk").as_ptr(), MB_YESNO | MB_ICONQUESTION) == IDYES
    };
    if !ok {
        return;
    }
    if let Ok(mut b) = app().board.try_borrow_mut() {
        b.remove_circle(ci);
    }
    if let Some(st) = state() {
        if let Ok(mut d) = st.dismissed.try_borrow_mut() {
            d.clear(); // circle indices shifted
        }
    }
    app().save_config();
    rebuild();
}

pub fn delete_group(ci: usize, gi: usize) {
    if let Ok(mut b) = app().board.try_borrow_mut() {
        b.remove_group(ci, gi);
    }
    app().save_config();
    refresh_all();
}
```

- [x] **Step 5: Wire `src/app.rs`.** Apply this diff (against `app.rs` as it is after Task D1):

```diff
diff --git a/src/app.rs b/src/app.rs
index 9b0f852..3f42d72 100644
--- a/src/app.rs
+++ b/src/app.rs
@@ -15,7 +15,8 @@ use windows_sys::Win32::UI::Controls::{InitCommonControlsEx, ICC_STANDARD_CLASSE
 use windows_sys::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
 use windows_sys::Win32::UI::WindowsAndMessaging::*;
 
-use crate::config::{self, Args, Config};
+use crate::circles::{self, model::Board};
+use crate::config::{self, Args, Config, Ui};
 use crate::host::{self, DesktopHost, HostKind};
 use crate::items::{self, Item};
 use crate::{diag, dlog, node, selftest, shell, watch, win};
@@ -33,6 +34,8 @@ pub const TIMER_SELFTEST: usize = 4;
 const TIMER_HEALTH: usize = 5;
 const TIMER_REPAINT: usize = 6;
 const TIMER_TRIM: usize = 7;
+/// Debounced re-listing of the Desktop folders (circles UI).
+pub const TIMER_DESKTOP: usize = 8;
 
 /// How often experimental hosts verify that their Explorer parent/owner still exists.
 const HEALTH_INTERVAL_MS: u32 = 2000;
@@ -51,6 +54,11 @@ pub struct Drag {
 /// interior mutability because window procedures re-enter freely.
 pub struct App {
     pub args: Args,
+    pub ui: Ui,
+    /// Circles; only changed while the circles UI runs, saved back unchanged otherwise.
+    pub board: RefCell<Board>,
+    /// The panel's `item=` list as loaded; saved back unchanged while the circles UI runs.
+    panel_items: Vec<PathBuf>,
     host: RefCell<Rc<dyn DesktopHost>>,
     pub controller: Cell<HWND>,
     pub node: Cell<HWND>,
@@ -90,23 +98,27 @@ impl App {
         if self.args.no_save {
             return;
         }
-        // Task D5 replaces `ui` and `board` with the running UI and the real board.
-        let c = Config {
-            host: self.host().kind(),
-            pos: Some(self.pos.get()),
-            items: self.items.borrow().iter().map(|i| i.path.clone()).collect(),
-            ui: None,
-            board: Default::default(),
+        let items = match self.ui {
+            Ui::Panel => self.items.borrow().iter().map(|i| i.path.clone()).collect(),
+            Ui::Circles => self.panel_items.clone(),
         };
+        let Ok(board) = self.board.try_borrow().map(|b| b.clone()) else { return };
+        let c = Config { host: self.host().kind(), pos: Some(self.pos.get()), items, ui: Some(self.ui), board };
         if let Err(e) = config::save(&c) {
             dlog!("config save failed: {e}");
         }
     }
 }
 
+/// Drops start-up/ring pages from the working set in a second (`TIMER_TRIM`).
+pub fn schedule_trim() {
+    unsafe { SetTimer(app().controller.get(), TIMER_TRIM, 1000, None) };
+}
+
 pub fn quit() {
     let a = app();
     a.quitting.set(true);
+    circles::shutdown();
     let node = a.node.get();
     if !node.is_null() {
         unsafe { DestroyWindow(node) };
@@ -186,6 +198,10 @@ pub fn run() -> i32 {
     // Configuration: command line > saved file > first-run defaults.
     let saved = if args.selftest { None } else { config::load() };
     let kind = args.host.or(saved.as_ref().map(|c| c.host)).unwrap_or(HostKind::Window);
+    // The self-test drives the panel; everything else defaults to the circles UI.
+    let ui = if args.selftest { Ui::Panel } else { args.ui.or(saved.as_ref().and_then(|c| c.ui)).unwrap_or(Ui::Circles) };
+    let board = saved.as_ref().map(|c| c.board.clone()).unwrap_or_default();
+    let panel_items = saved.as_ref().map(|c| c.items.clone()).unwrap_or_default();
     let paths: Vec<PathBuf> = if !args.items.is_empty() {
         args.items.clone()
     } else if let Some(c) = saved.as_ref().filter(|c| !c.items.is_empty()) {
@@ -194,13 +210,18 @@ pub fn run() -> i32 {
         items::default_items()
     };
     let t_items = diag::since_start_ms();
-    let loaded: Vec<Item> = paths.iter().cloned().map(Item::placeholder).collect();
-    let load_paths = paths;
+    let (loaded, load_paths): (Vec<Item>, Vec<PathBuf>) = match ui {
+        Ui::Panel => (paths.iter().cloned().map(Item::placeholder).collect(), paths),
+        Ui::Circles => (Vec::new(), Vec::new()),
+    };
     dlog!("METRIC items_placeholder count={} ms={:.1}", loaded.len(), diag::since_start_ms() - t_items);
     let pos = args.pos.or(saved.as_ref().and_then(|c| c.pos)).unwrap_or_else(|| default_pos(loaded.len()));
     let first_run = saved.is_none();
 
     let state: &'static App = Box::leak(Box::new(App {
+        ui,
+        board: RefCell::new(board),
+        panel_items,
         host: RefCell::new(host::create(kind)),
         controller: Cell::new(std::ptr::null_mut()),
         node: Cell::new(std::ptr::null_mut()),
@@ -228,14 +249,18 @@ pub fn run() -> i32 {
     a.controller.set(controller);
     items::spawn_loader(controller, load_paths);
     a.notify_id.set(watch::register(controller));
-    node::register_class();
-
-    if let Err(e) = node::create() {
-        dlog!("host '{}' unavailable: {e}; falling back to 'window'", kind.name());
-        *a.host.borrow_mut() = host::create(HostKind::Window);
+    dlog!("ui={}", ui.name());
+    if ui == Ui::Circles {
+        circles::start();
+    } else {
+        node::register_class();
         if let Err(e) = node::create() {
-            message_box(&format!("Could not create the DeskTuk node: {e}"), true);
-            return 1;
+            dlog!("host '{}' unavailable: {e}; falling back to 'window'", kind.name());
+            *a.host.borrow_mut() = host::create(HostKind::Window);
+            if let Err(e) = node::create() {
+                message_box(&format!("Could not create the DeskTuk node: {e}"), true);
+                return 1;
+            }
         }
     }
     if first_run {
@@ -259,7 +284,7 @@ pub fn run() -> i32 {
     unsafe {
         SetTimer(controller, TIMER_TRIM, 1000, None);
         SetTimer(controller, TIMER_IDLE_METRICS, 2000, None);
-        if a.host().kind() != HostKind::Window {
+        if ui == Ui::Panel && a.host().kind() != HostKind::Window {
             SetTimer(controller, TIMER_HEALTH, HEALTH_INTERVAL_MS, None);
         }
         if let Some(ms) = a.args.exit_after_ms {
@@ -270,13 +295,7 @@ pub fn run() -> i32 {
         selftest::start();
     }
 
-    let mut msg: MSG = unsafe { std::mem::zeroed() };
-    while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
-        unsafe {
-            TranslateMessage(&msg);
-            DispatchMessageW(&msg);
-        }
-    }
+    pump_messages();
     unsafe { UnhookWinEvent(a.hook.get()) };
     diag::log_resources("exit");
     let code = selftest::exit_code();
@@ -284,6 +303,51 @@ pub fn run() -> i32 {
     code
 }
 
+/// The message loop. Besides messages it waits on the Desktop folder change handles of the
+/// circles UI (none in panel mode), so watching the Desktop needs no thread and no polling.
+fn pump_messages() {
+    use windows_sys::Win32::Foundation::{WAIT_FAILED, WAIT_OBJECT_0};
+    use windows_sys::Win32::System::Threading::INFINITE;
+    let mut msg: MSG = unsafe { std::mem::zeroed() };
+    loop {
+        let handles = circles::wait_handles();
+        let n = handles.len() as u32;
+        let r = unsafe {
+            MsgWaitForMultipleObjectsEx(
+                n,
+                if n == 0 { std::ptr::null() } else { handles.as_ptr() },
+                INFINITE,
+                QS_ALLINPUT,
+                MWMO_INPUTAVAILABLE,
+            )
+        };
+        if r < WAIT_OBJECT_0 + n {
+            circles::on_desktop_signal((r - WAIT_OBJECT_0) as usize);
+            continue;
+        }
+        if r == WAIT_FAILED {
+            // Should not happen; fall back to a plain blocking wait so the loop never spins.
+            dlog!("MsgWaitForMultipleObjectsEx failed: error {}", unsafe { GetLastError() });
+            if unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } <= 0 {
+                return;
+            }
+            unsafe {
+                TranslateMessage(&msg);
+                DispatchMessageW(&msg);
+            }
+            continue;
+        }
+        while unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) } != 0 {
+            if msg.message == WM_QUIT {
+                return;
+            }
+            unsafe {
+                TranslateMessage(&msg);
+                DispatchMessageW(&msg);
+            }
+        }
+    }
+}
 unsafe extern "system" fn win_event(_h: HWINEVENTHOOK, event: u32, hwnd: HWND, id_object: i32, _child: i32, _thread: u32, _time: u32) {
     if event != EVENT_SYSTEM_FOREGROUND || id_object != OBJID_WINDOW {
         return;
@@ -291,7 +355,9 @@ unsafe extern "system" fn win_event(_h: HWINEVENTHOOK, event: u32, hwnd: HWND, i
     let a = app();
     let node = a.node.get();
     dlog!("foreground -> {hwnd:?} class={:?}", win::class_name(hwnd));
-    if !node.is_null() {
+    if a.ui == Ui::Circles {
+        circles::foreground_changed(hwnd);
+    } else if !node.is_null() {
         a.host().foreground_changed(node, hwnd);
     }
 }
@@ -445,6 +511,10 @@ unsafe extern "system" fn controller_proc(hwnd: HWND, msg: u32, wparam: WPARAM,
         }
         WM_APP_ITEMS_LOADED => {
             let metas = *Box::from_raw(lparam as *mut Vec<(PathBuf, items::Meta)>);
+            if a.ui == Ui::Circles {
+                circles::ring::apply_metas(metas);
+                return 0;
+            }
             {
                 let mut items = a.items.borrow_mut();
                 for (p, m) in metas {
@@ -482,6 +552,7 @@ unsafe extern "system" fn controller_proc(hwnd: HWND, msg: u32, wparam: WPARAM,
                     diag::log_resources("idle_2s");
                 }
                 TIMER_SELFTEST => selftest::on_timer(),
+                TIMER_DESKTOP => circles::refresh_desktop(),
                 TIMER_HEALTH => health_check(),
                 TIMER_REPAINT => {
                     KillTimer(hwnd, TIMER_REPAINT);
@@ -499,10 +570,15 @@ unsafe extern "system" fn controller_proc(hwnd: HWND, msg: u32, wparam: WPARAM,
             if !node.is_null() {
                 node::reposition(node);
             }
+            circles::refresh_all();
             DefWindowProcW(hwnd, msg, wparam, lparam)
         }
         WM_APP_SHELL_CHANGE => {
             if let Some(change) = watch::decode(wparam, lparam) {
+                if a.ui == Ui::Circles {
+                    circles::on_shell_change(&change);
+                    return 0;
+                }
                 // Copy paths out, compute, write back: no borrow is held across a window call.
                 let mut paths: Vec<PathBuf> = a.items.borrow().iter().map(|i| i.path.clone()).collect();
                 let before = paths.clone();
```

- [x] **Step 6: Build and check.**

Run: `cargo fmt`, `cargo clippy --release --locked --all-targets -- -D warnings`, `cargo test --release --locked`
Expected: clippy clean (no `dead_code` left now that `allow` is gone); 36 tests PASS, 1 ignored (the preview).

- [x] **Step 7: Smoke run without touching the real config.**

```powershell
$tmp = "$env:TEMP\desktuk-d5"; New-Item -ItemType Directory -Force "$tmp\DeskTuk" | Out-Null
Set-Content "$tmp\DeskTuk\desktuk.cfg" "host=window`nui=circles`ncircle=Music|300|300`ngroup=Music|Live`n" -Encoding utf8
$old = $env:APPDATA; $env:APPDATA = $tmp
$p = Start-Process .\target\release\DeskTuk.exe -ArgumentList '--no-save','--allow-multiple','--exit-after','5000','--log',"$tmp\log.txt" -PassThru
$env:APPDATA = $old; $p.WaitForExit(10000); "exit=$($p.ExitCode)"
Select-String "$tmp\log.txt" -Pattern 'ui=|METRIC circles_started|resources\[idle_2s\]'
```
Expected: `exit=0`; `ui=circles`; `METRIC circles_started circles=1 ...`; `resources[idle_2s] ... private_kb=` about 2 000 (v1 panel: about 4 600). Then confirm the panel still starts: the same command with `--ui panel` added -> the log has `ui=panel` and `METRIC startup first_paint...`.

- [x] **Step 8: Manual checks (needs a person at the desktop; record each result in the commit body; if you cannot do them, say "not verified" for each).**
  1. Hover Recent: the ring opens with a short reveal; moving the pointer off the disc closes it.
  2. Right-click Recent -> New circle: a circle appears to its left with a rename box; type "Music", Enter.
  3. Drag a Desktop file from Explorer onto "Music": its count goes up; Recent's count goes down.
  4. Hover Music, right-click the file -> real shell menu with "Remove from circle"; Remove: it is back in Recent.
  5. Music -> New group "Live"; drag a file slot onto the "Live" slot; click "Live": the file is there; Back returns.
  6. With more than 20 files in Recent: wheel and centre click page through "1-20 / N".
  7. Drag "Music" to another place, Exit, start again: same place.
  8. Win+D: circles stay visible; click an app: they go back under it.
  9. Delete "Music": confirmation; its files are back in Recent; no file on disk changed (compare `Get-ChildItem $env:USERPROFILE\Desktop` before/after).

Execution notes: all nine Step 8 checks are **not verified** (no person performed the desktop interactions). Each is recorded separately in the D5 commit body. The task additionally includes `scripts/circles-regression-test.ps1`: it reproduced and verifies fixes for explicit CLI UI selection not being saved to an existing config and a deleted visible reference not refreshing its metadata. Fixtures live only in a unique TEMP directory; the script sends no mouse or keyboard input.

- [x] **Step 9: Commit** (`Add the circles UI`, last line `... Task D5`).

---

### Task D6: Benchmark against v1, integration perf runs, README

**Files:**
- Create: `scripts/bench-memory.ps1`
- Modify: `scripts/windows-integration-test.ps1` (perf loop), `README.md`

**Interfaces:**
- Consumes: the `--ui` option and `ui=` key (D1), `ui=` log line and window class `DeskTuk.Circle` (D5).

- [x] **Step 1: Create `scripts/bench-memory.ps1`:**

```powershell
<#
.SYNOPSIS
  DeskTuk memory/CPU benchmark: compares builds (e.g. v1 = Phase 1, v2 = Phase 2) in the
  same scenarios. Read-only for the user's files: every run uses a throw-away APPDATA
  (config) and throw-away item files under %TEMP%; nothing is written to the Desktop.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\bench-memory.ps1 -Exe C:\tmp\DeskTuk-v1.exe,target\release\DeskTuk.exe

  Give each build a distinct file name (the name is the table's Build column).
  Builds that do not know the circles UI (v1) are only run in the panel scenarios: the UI is
  selected with a `ui=` config line, which v1 ignores, never with `--ui` (v1 would show an
  "unknown option" message box).
#>
param(
    [Parameter(Mandatory = $true)][string[]]$Exe,
    [int]$Runs = 3,
    [int]$IdleSeconds = 15
)
$ErrorActionPreference = 'Stop'
# `-File` passes `a.exe,b.exe` as one string.
$Exe = @($Exe | ForEach-Object { $_ -split ',' } | Where-Object { $_ })

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class BenchWin {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, System.Text.StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    public static IntPtr FindOwn(uint pid, string cls) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) => {
            uint p; GetWindowThreadProcessId(h, out p);
            if (p != pid) return true;
            var sb = new System.Text.StringBuilder(64); GetClassName(h, sb, 64);
            if (sb.ToString() == cls) { found = h; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
'@

$root = Join-Path $env:TEMP "desktuk-bench-$PID"
$files = Join-Path $root 'items'
New-Item -ItemType Directory -Force $files | Out-Null
1..300 | ForEach-Object { Set-Content -LiteralPath (Join-Path $files ('file-{0:D3}.txt' -f $_)) -Value 'x' }
$paths = Get-ChildItem $files | Sort-Object Name | ForEach-Object FullName

function Config-Text([string]$kind) {
    switch ($kind) {
        'panel-default' { "host=window`nui=panel`n" }
        'panel-50'      { "host=window`nui=panel`n" + (($paths | Select-Object -First 50 | ForEach-Object { "item=$_" }) -join "`n") + "`n" }
        'circles-empty' { "host=window`nui=circles`n" }
        'circles-300'   {
            $t = "host=window`nui=circles`n"
            for ($c = 0; $c -lt 5; $c++) {
                $t += "circle=Bench $c|$(200 + 110 * $c)|200`n"
                $t += (($paths | Select-Object -Skip (60 * $c) -First 60 | ForEach-Object { "citem=Bench $c||$_" }) -join "`n") + "`n"
            }
            $t
        }
    }
}

$scenarios = @(
    @{ Name = 'panel, default items';      Config = 'panel-default'; Circles = $false; Ring = $false },
    @{ Name = 'panel, 50 items';           Config = 'panel-50';      Circles = $false; Ring = $false },
    @{ Name = 'circles, Recent only';      Config = 'circles-empty'; Circles = $true;  Ring = $false },
    @{ Name = 'circles, 5 x 60 items';     Config = 'circles-300';   Circles = $true;  Ring = $false },
    @{ Name = 'circles, ring open (20)';   Config = 'circles-300';   Circles = $true;  Ring = $true }
)

function Median($v) { $s = @($v | Sort-Object); if ($s.Count -eq 0) { return '' }; $s[[int][math]::Floor(($s.Count - 1) / 2)] }

$rows = foreach ($sc in $scenarios) {
    foreach ($item in $Exe) {
        $build = (Resolve-Path $item).Path
        # A build without the circles UI has no 'DeskTuk.Circle' window class string.
        $hasCircles = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($build)).Contains('DeskTuk.Circle')
        if ($sc.Circles -and -not $hasCircles) { continue }
        $m = @{ commit = @(); pws = @(); mods = @(); handles = @(); startcpu = @(); idlecpu = @(); ui = @() }
        for ($r = 0; $r -lt $Runs; $r++) {
            $appdata = Join-Path $root "appdata-$r"
            New-Item -ItemType Directory -Force (Join-Path $appdata 'DeskTuk') | Out-Null
            Set-Content -LiteralPath (Join-Path $appdata 'DeskTuk\desktuk.cfg') -Value (Config-Text $sc.Config) -Encoding utf8
            $log = Join-Path $root "run.log"
            Remove-Item $log -ErrorAction SilentlyContinue
            $old = $env:APPDATA; $env:APPDATA = $appdata
            $p = Start-Process -FilePath $build -ArgumentList @('--no-save', '--allow-multiple', '--log', "`"$log`"") -PassThru
            $env:APPDATA = $old
            Start-Sleep -Seconds 4
            $ui = if (Test-Path $log) { (Select-String -Path $log -Pattern 'ui=(\w+)' | Select-Object -First 1 | ForEach-Object { $_.Matches[0].Groups[1].Value }) } else { $null }
            if (-not $ui) { $ui = 'panel' } # v1 does not log a ui line
            if ($sc.Ring -and $ui -eq 'circles') {
                $h = [BenchWin]::FindOwn([uint32]$p.Id, 'DeskTuk.Circle')
                if ($h -ne [IntPtr]::Zero) { [void][BenchWin]::PostMessage($h, 0x02A1, [IntPtr]::Zero, [IntPtr]::Zero) } # WM_MOUSEHOVER
                Start-Sleep -Milliseconds 1500
            }
            $p.Refresh()
            if ($p.HasExited) { continue }
            $t0 = $p.TotalProcessorTime.TotalMilliseconds
            $pw = (Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Filter "IDProcess=$($p.Id)").WorkingSetPrivate
            $m.commit += [math]::Round($p.PrivateMemorySize64 / 1KB); $m.pws += [math]::Round($pw / 1KB)
            $m.mods += $p.Modules.Count; $m.handles += $p.HandleCount; $m.startcpu += [math]::Round($t0)
            $m.ui += $ui
            if (-not $sc.Ring) {
                Start-Sleep -Seconds $IdleSeconds
                $p.Refresh(); $m.idlecpu += [math]::Round($p.TotalProcessorTime.TotalMilliseconds - $t0, 1)
            }
            Stop-Process -Id $p.Id -Force; Start-Sleep -Milliseconds 300
        }
        $ranUi = ($m.ui | Select-Object -Unique) -join ','
        if ($sc.Circles -and $ranUi -ne 'circles') { continue } # this build has no circles UI
        [pscustomobject]@{
            Scenario = $sc.Name; Build = Split-Path $build -Leaf; UI = $ranUi
            CommitKB = Median $m.commit; PrivateWsKB = Median $m.pws; Modules = Median $m.mods; Handles = Median $m.handles
            'StartCpuMs' = Median $m.startcpu; "IdleCpuMs/$($IdleSeconds)s" = Median $m.idlecpu
        }
    }
}
$rows | Format-Table -AutoSize | Out-String -Width 220
Remove-Item $root -Recurse -Force -ErrorAction SilentlyContinue
```

- [x] **Step 2: Measure perf for both UIs in CI.** Apply to `scripts/windows-integration-test.ps1`:

```diff
diff --git a/scripts/windows-integration-test.ps1 b/scripts/windows-integration-test.ps1
index 5b0a7f3..a99ca40 100644
--- a/scripts/windows-integration-test.ps1
+++ b/scripts/windows-integration-test.ps1
@@ -92,19 +92,22 @@ foreach ($h in $Hosts) {
 Ensure-Explorer
 
 # --- RAM and startup ---------------------------------------------------------------------
-Write-Host "`n== performance (host=window, $PerfRuns runs)"
+Write-Host "`n== performance (host=window, $PerfRuns runs per UI)"
+foreach ($ui in @('panel', 'circles')) {
 for ($i = 1; $i -le $PerfRuns; $i++) {
-    $log = Join-Path $Out "perf-$i.log"
-    $quoted = @('--no-save', '--allow-multiple', '--exit-after', '6000', '--log', $log, $lnk, $txt) |
+    $log = Join-Path $Out "perf-$ui-$i.log"
+    # circles: the runner has no config, so this measures the Recent circle only.
+    $quoted = @('--ui', $ui, '--no-save', '--allow-multiple', '--exit-after', '6000', '--log', $log, $lnk, $txt) |
         ForEach-Object { if ($_ -match '\s') { '"' + $_ + '"' } else { $_ } }
     $p = Start-Process -FilePath $Exe -ArgumentList $quoted -PassThru
     Start-Sleep -Seconds 4
     $p.Refresh()
-    Write-Host ("METRIC perf run={0} working_set_kb={1} private_kb={2} peak_working_set_kb={3} handles={4}" -f `
+    Write-Host ("METRIC perf ui=$ui run={0} working_set_kb={1} private_kb={2} peak_working_set_kb={3} handles={4}" -f `
         $i, [int]($p.WorkingSet64 / 1KB), [int]($p.PrivateMemorySize64 / 1KB), [int]($p.PeakWorkingSet64 / 1KB), $p.HandleCount)
     $null = $p.WaitForExit(15000)
     Select-String -Path $log -Pattern 'METRIC' | ForEach-Object { Write-Host $_.Line }
 }
+}
 
 # --- Files must be untouched ---------------------------------------------------------------
 $after = Get-Snapshot
```

- [x] **Step 3: README.** In `README.md`, add to the options block after the `--host` line:

```text
  --ui <circles|panel>         circles (default) or the Phase 1 panel
```

and add this section after the "Run" section:

```markdown
## Circles (Phase 2)

By default DeskTuk shows **circles** instead of the panel. *Recent* (blue) holds every Desktop
file that is in no circle, newest first. Right-click a circle for *New circle*, *New group*,
*Rename*, *Delete circle*. Hover a circle to open its ring of files (20 per page; the wheel or
a click on the centre turns the page). Drop files from Explorer onto a circle to add them, drag
a file out of a ring onto another circle or group to move it, right-click a file for the real
shell menu and *Remove from circle*. Files are never moved or changed; circles only reference
them. `--ui panel` (saved) brings back the Phase 1 panel. Design:
[`docs/superpowers/specs/2026-10-13-circles-design.md`](docs/superpowers/specs/2026-10-13-circles-design.md).
```

- [x] **Step 4: Run the benchmark against v1** (on Windows, nothing else heavy running):

```powershell
git worktree add --detach $env:TEMP\desktuk-v1 39ef8a9   # v1 = Phase 1
$env:CARGO_TARGET_DIR = "$env:TEMP\desktuk-v1-target"; Push-Location $env:TEMP\desktuk-v1; cargo build --release --locked; Pop-Location; Remove-Item Env:CARGO_TARGET_DIR
Copy-Item "$env:TEMP\desktuk-v1-target\release\DeskTuk.exe" "$env:TEMP\DeskTuk-v1.exe"
cargo build --release --locked; Copy-Item target\release\DeskTuk.exe "$env:TEMP\DeskTuk-v2.exe"
powershell -ExecutionPolicy Bypass -File scripts\bench-memory.ps1 -Exe "$env:TEMP\DeskTuk-v1.exe,$env:TEMP\DeskTuk-v2.exe"
git worktree remove --force $env:TEMP\desktuk-v1
```
Expected (spec gate): "circles, Recent only" and "circles, 5 x 60 items" commit and modules **at or below** v1's "panel, default items"; every IdleCpuMs = 0; "circles, ring open (20)" is reported (it is the cost of real shell icons; compare with v1 showing 20 Desktop files: about 7.8 MB). Paste the table into the commit body. **If the gate fails, do not open the PR**: commit, push, and write in the commit body what failed - the owner decides.

- [x] **Step 5: Pre-commit checks, commit** (`Add the v1/v2 benchmark and perf runs for both UIs`, last line `... Task D6`), **push, wait for CI to be green, open the PR** into `main` titled `Phase 2 / Plan D: circles` with the benchmark table and the manual-check results from D5.

Execution deviations and verification notes:

* The benchmark selects the synthetic `Bench 0` circle explicitly and checks that its ring exists; enumerating the first circle could open Recent with a different number of real Desktop files. Background launches use `Hidden`, scratch roots are unique, and recursive cleanup checks the resolved TEMP path.
* The final independent review ran before the D6 commit so the branch retains one commit per task. Its three runtime corrections are included in D6: temporary foreground ownership during drag capture with focus/style/z-order restoration; `FILE_NOTIFY_CHANGE_LAST_WRITE` for Recent ordering; explicit rename commit before clicks on non-activating circles/rings. `src/circles/drag_capture.rs` is the additional module.
* The native file-write notification and hidden rename-editor lifecycle tests failed before implementation and passed afterward. The suite now has 38 passing tests and one ignored visual preview. The config/UI and deleted-item fixture regressions pass and run in CI via `scripts/circles-regression-test.ps1`.
* The foreground capture probe (`-CheckDragCapture`) is opt-in: `SetForegroundWindow` returned 0 in this session, so real drag behavior is **not verified**. All nine D5 manual checks remain **not verified**. No local integration test was run.

---

D6 measured results (final source, three-run medians):

Benchmark gate: PASS. Three-run medians, 15-second idle CPU samples; v1 is 39ef8a9. Closed circles commit/modules are compared with v1 panel default; all idle CPU medians are 0. Ring uses 20 visible synthetic .txt references.

| Scenario | Build | UI | Commit KB | Private WS KB | Modules | Handles | Start CPU ms | Idle CPU ms / 15 s |
|---|---|---|---:|---:|---:|---:|---:|---:|
| panel, default items | DeskTuk-v1.exe | panel | 4632 | 120 | 46 | 266 | 94 | 0 |
| panel, default items | DeskTuk-v2.exe | panel | 4688 | 124 | 45 | 267 | 94 | 0 |
| panel, 50 items | DeskTuk-v1.exe | panel | 4652 | 124 | 44 | 246 | 125 | 0 |
| panel, 50 items | DeskTuk-v2.exe | panel | 4728 | 116 | 43 | 247 | 172 | 0 |
| circles, Recent only | DeskTuk-v2.exe | circles | 2032 | 140 | 30 | 149 | 31 | 0 |
| circles, 5 x 60 items | DeskTuk-v2.exe | circles | 2160 | 136 | 30 | 149 | 62 | 0 |
| circles, ring open (20) | DeskTuk-v2.exe | circles | 6240 | 3628 | 43 | 251 | 141 | - |

## Self-review

* Spec coverage: radial circles + ring (D2, D4, D5); three levels with Back (D3 `view`, D5 ring); paging 20 + wheel + centre click (D2, D5); Recent newest-first (D3, D5); drop to assign, drag between circles/groups, Remove (D3 `drop_place`, D5); suggestions accept/not now (D3, D5); renamable names with uniqueness (D1, D5 rename box); persistence keys (D1); `--ui`, default circles, selftest on panel (D1, D5); Desktop watching without threads (D3 Watcher, D5 loop); free + trim after a ring (D5); benchmark gate vs v1 (D6).
* Types: `Place`, `Target`, `SlotRef`, `Suggestion`, `Which`, `RenameTarget` are defined once (D1/D3/D5) and used with the same names everywhere; `assign(&[PathBuf], Option<Place>)` (`None` = back to Recent) matches `view::drop_place`.
* Review Focus items have owners: config compatibility (D1 tests + D5 Step 7), paging (D2 tests + D5 Step 8.6), menus over the ring (`with_menu`, D5 Step 8.4), index shifts (`rebuild` cancels rename/ring, D5 Step 8.9), live Desktop changes (D1 `follow` test, D5 Watcher + Step 8.3).


## Owner follow-ups (2026-10-13, after PR #3)

The owner requested desktop-sized, sharp ring icons and a context-menu toggle for
Windows desktop icons. Continue the existing Plan D branch/PR, one commit per task.
No dependencies are added; only features of the existing Windows bindings are enabled.

### Task D7: Match the desktop icon size and resolution

- [x] Query the current Explorer desktop icon size only when opening a ring; use 48
      DPI-scaled pixels when Explorer is unavailable. Read it again on the next opening.
- [x] Keep native-sized icons and the desktop label font. Enlarge the ring and reduce
      page capacity when needed to fit the monitor work area; never shrink the icons.
- [x] Extract size-specific shell icons on the loader thread, discard obsolete batches,
      and release image/COM resources on failure, refresh and close.
- [x] Update the design/README; run the AGENTS pre-commit checks and release build,
      commit and push. Actual visual comparison at the desktop: **not verified**.

### Task D8: Toggle Windows desktop icons from the app context menu

- [x] Add a checked Masaüstü öğelerini göster entry to circle/Recent, file and panel menus.
      Read Explorer's current visibility whenever the menu is opened.
- [x] Change only desktop icon visibility (view flag and Explorer preference) on explicit selection; show an error
      if Explorer is unavailable. Do not change it at startup or shutdown.
- [x] Update the design/README; run the AGENTS pre-commit checks and release build,
      commit and push. Human menu/show/hide check: **not verified**.


D7 evidence: cargo fmt, cargo clippy --release --locked -- -D warnings and
cargo test --release --locked passed (38 passed, one ignored). Release build passed
with --target-dir target/desktop-update because the owner has the original release
executable running; the default build could not replace that locked executable.
Visual parity, alternate desktop icon sizes/DPI, and large-icon paging at the desktop:
**not verified**. No desktop settings were changed and no integration script was run.


D8 evidence: cargo fmt, cargo clippy --release --locked -- -D warnings and
cargo test --release --locked passed (38 passed, one ignored). Release build passed
with --target-dir target/desktop-update; the owner's original running executable
was left open. No integration script was run locally and no desktop visibility
setting was changed during verification.

The desktop lookup now passes an explicit VT_I4 CSIDL_DESKTOP location to
FindWindowSW. The menu is available in file shell menus and their fallback menus,
as well as the circle/Recent/panel menus. Only FWF_NOICONS and Explorer's fHideIcons
preference change on selection; no config preference or startup/exit action is added.

Manual checks:
* Windows menu and DeskTuk checkmarks agree: **not verified**.
* Hide and show Windows desktop icons through the new option: **not verified**.
* DeskTuk circles stay visible and files stay in place: **not verified**.
* Windows keeps the choice across app/Explorer restarts: **not verified**.
* Desktop icon visual parity, alternate sizes/DPI and large-icon paging: **not verified**.

Final follow-up benchmark, three-run medians, 15 s idle, synthetic TEMP fixtures:

| Scenario | UI | Commit KB | Private WS KB | Modules | Handles | Start CPU ms | Idle CPU ms / 15 s |
|---|---|---:|---:|---:|---:|---:|---:|
| panel, default items | panel | 4708.00 | 120.00 | 45 | 267 | 94.00 | 0.00 |
| panel, 50 items | panel | 4592.00 | 124.00 | 43 | 247 | 172.00 | 0.00 |
| circles, Recent only | circles | 2020.00 | 140.00 | 30 | 149 | 31.00 | 0.00 |
| circles, 5 x 60 items | circles | 2136.00 | 144.00 | 30 | 149 | 62.00 | 0.00 |
| circles, ring open (20) | circles | 7220.00 | 3892.00 | 46 | 307 | 750.00 | - |

Gate: **PASS**. Compare the closed-circle rows with the v1 baseline measured in D6
(4632 KB commit, 46 modules, 0 ms idle CPU). This run reused that baseline; it did not
rebuild/re-measure v1. The open-ring figure is reported separately and is not the
closed-circle gate. Benchmark launches used separate TEMP configs with no-save.

API references: [icon size](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifolderview2-getviewmodeandiconsize),
[worker image extraction](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ishellitemimagefactory-getimage),
[view flag](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifolderview2-setcurrentfolderflags),
[Explorer preference](https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shgetsetsettings).


### Task D9: Tray recovery and Windows startup (owner, 2026-10-14)

The owner requested the postponed B3 features after login left the desktop empty:
Explorer icons were hidden and DeskTuk was not running. Implement B3 in this existing
circles branch/PR, adapting Show Node to both UIs and using WM_APP + 5 because + 4
already belongs to ring loading. No new dependencies. Keep B1/B2/C on hold.

- [x] Reproduce missing startup/tray and add failing startup command/registry tests.
- [x] Implement tray add/remove, mouse/keyboard callbacks, checked startup toggle,
      bounded startup retry and TaskbarCreated recovery; release resources before exit.
- [x] Restore circles/panel from the tray and from a second launch without RefCell
      borrows across Win32 calls; preserve desktop visibility, files and config.
- [x] Verify with MSVC fmt/clippy/tests, isolated runtime fixtures, idle measurements
      and release build. Record limits; do not kill Explorer or run local integration tests.
- [x] Commit/push the validated implementation to the existing PR. Deploy the checked
      release binary and enable startup for the owner after the login failure. Hosted
      CI is tracked separately through the PR head checks.

Deviations from B3: UTF-16 startup commands preserve Windows paths and reject the
documented 260-character limit; registry operations return errors instead of silently
failing. Tests use an isolated HKCU test key, never the real Run value. Startup remains
off during tests; deployment enables it for the owner after validation. Measurement
and self-test instances using --allow-multiple do not register competing tray icons.
The superpowers execution skills referenced by the original plans are unavailable in
this session; execution follows AGENTS task order and checks directly.


D9 verification (Windows x86_64-pc-windows-msvc, 2026-10-14):

* RED: the new startup tests failed with the implementation functions missing.
* GREEN: cargo fmt, cargo clippy --release --locked -- -D warnings,
  cargo test --release --locked (43 passed, one ignored preview), and release build.
  CARGO_TARGET_DIR=target/startup-update avoided replacing the owner's running binary.
* tray-regression-test: both circles and panel passed tray registration, mouse and
  keyboard recovery, on-screen clamping, normal (non-topmost) window band, second launch,
  synthetic TaskbarCreated re-registration after removing the fixture icon, graceful
  exit and icon cleanup. Config/file hashes, desktop visibility and real startup value
  were unchanged. The fixture uses TEMP files only and does not restart Explorer.
* Existing circles-regression-test passed saved CLI UI and deleted-reference refresh.
  Actual drag capture remains not verified; its optional probe was not run.
* Normal instance with the real tray (Recent only, no --allow-multiple): 2108 KB private
  commit, 30 modules, 0 ms CPU over 15 seconds. Single local sample; comparison uses the
  earlier D6 v1 baseline (4632 KB, 46 modules), not a fresh v1 measurement. Gate PASS.
* Deployed the checked binary to D:/CodeSpace/desktuk/DeskTuk.exe; SHA256
  8C1FC5DDAE83E89F45C19039F3D68A811699CB8AF0A75D5B5FD495C5CF3847FC.
  The installed process responded and logged circles/tray initialization. Enabled
  HKCU Run/DeskTuk with exactly "D:\CodeSpace\desktuk\DeskTuk.exe". The owner's
  configuration hash and desktop visibility preference were preserved.
* Actual reboot/login, physical tray-menu interaction and killing/restarting Explorer
  are not verified. No local windows-integration-test was run. Hosted CI results are
  available in the head checks of https://github.com/ege-ozel/desktuk/pull/3.

Final review correction: reveal uses HWND_NOTOPMOST then HWND_TOP in the host layer,
so recovering the UI does not leave it permanently topmost over other applications.
Native regression checks assert that the topmost style is absent after recovery.
The Windows SDK's NIN_KEYSELECT constant is absent from windows-sys and is defined
locally as NIN_SELECT | NINF_KEY; no dependency/feature addition was needed.

API references: [notification area](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shell_notifyiconw),
[startup registry key and command limit](https://learn.microsoft.com/en-us/windows/win32/setupapi/run-and-runonce-registry-keys).


### Task D10: Drag a circle while its ring is open (owner, 2026-10-14)

The owner clarified that dragging the circle itself fails while the ring is open.
The layered ring covers the circle, but previously its centre only handled paging.
Route centre presses to the circle's existing capture/move/release lifecycle. Keep
short-click paging; once movement crosses the Windows drag threshold, close the ring
and move/save its circle. Use event coordinates rather than sampling a later cursor
position. Keep file-slot dragging and the other existing behaviours.

- [x] Reproduce with an isolated native message fixture for Recent and a user circle.
- [x] Add shared circle press routing, preserve centre-click paging and leave/cancel
      handling, and keep all message-pumping calls outside ring/state borrows.
- [x] Verify MSVC fmt/clippy/tests, open-ring drag/paging/persistence fixtures and the
      existing regressions. Do not move the user's pointer or restart Explorer.
- [ ] Commit/push, update the existing PR, rebuild and deploy to the owner's stable
      D:/CodeSpace/desktuk/DeskTuk.exe. Preserve startup, layout and desktop settings.


D10 scope update: the owner also reports disappearing circles when opening the
Windows notification-area overflow. The installed executable matches D9's SHA256.
WindowHost currently lowers circles below Explorer for Shell_TrayWnd and the
overflow XAML window. Preserve the existing desktop ordering for taskbar roots and
their descendants; continue lowering for real application windows. Add a native
off-screen host regression; do not broadly ignore generic UWP/XAML app windows.


D10 evidence:

* Installed D9 binary SHA256 matched the recorded checked release. Native message
  fixture failed before the fix: the ring closed during a pending centre drag.
* The new native host test failed on Shell_TrayWnd before changing focus handling.
  After the fix taskbar/overflow roots and descendants preserve host ordering; a
  generic UWP application still lowers the node and clears topmost style.
* Windows x86_64-pc-windows-msvc: cargo fmt, release locked clippy with -D warnings,
  cargo test --release --locked (44 passed, one ignored preview), and release build
  all passed. CARGO_TARGET_DIR=target/open-circle-drag keeps the installed app open.
* Native routed-message fixtures passed open-centre drag for Recent and Music,
  exact pointer delta (including an edge-shifted ring), ring close on movement,
  saved/reloaded positions, retained short-click paging and unchanged reference
  file hashes. Existing circles regressions passed UI persistence and deleted-item
  refresh. Physical input/foreground capture is not established by routed messages.
* No user pointer movement, user file changes, real Explorer restart or local
  windows-integration-test run. New fixture is included in CI.
* Source/release are committed as D10; installation is combined with the owner's
  subsequent D11 tray-menu request so only one final app replacement is needed.
