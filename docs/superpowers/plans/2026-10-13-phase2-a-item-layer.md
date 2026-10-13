# Phase 2 / Plan A - Robust item layer

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax. Read `AGENTS.md` first.

**Goal:** Items load off the UI thread, follow renames/deletes/updates made in Explorer, and offer the real shell context menu.

**Architecture:** `items.rs` gets a pure `Meta` value and a loader thread that posts results to the hidden controller window (`WM_APP_ITEMS_LOADED`). A new pure module `watch.rs` turns shell change notifications (`SHChangeNotifyRegister`, new delivery) into `Change` values and applies them to item paths; the same loader re-queries affected items. Context menu is a separate on-demand module.

**Tech Stack:** Rust 2021, `windows-sys 0.61` (features already enabled: `Win32_UI_Shell`, `Win32_UI_Shell_Common`, `Win32_System_Com`). Task A3 adds the `windows` crate - see there.

**Spec:** `docs/superpowers/specs/2026-10-13-phase-2-design.md`

## Global Constraints

* Never touch user files. Items are references only.
* Never hold `app().items.borrow()` / `borrow_mut()` across a message-pumping call (see spec). Copy data out first.
* No `SHGFI_ICON` / `SHGFI_OVERLAYINDEX` anywhere (loads icon-overlay handler DLLs, +3.5 MB).
* `panic = "abort"`: no `unwrap()` on Win32/shell results; a panic is a crash.
* Pre-commit: `cargo fmt`, `cargo clippy --release --locked -- -D warnings`, `cargo test --release --locked`.

## Review Focus

* Path with Unicode / spaces / trailing dots; case differences (`C:\Users\X` vs `c:\users\x`) must match the same item.
* A loader result that arrives after the item was removed or the node destroyed: must be ignored, not crash.
* Notification flood (thousands of events for unrelated files): handler must return quickly when nothing matches.
* Item deleted then re-created with the same name: becomes non-missing again.
* Renaming a *folder* that contains items: item paths are rewritten by prefix.

## File structure

| File | Responsibility |
|------|----------------|
| `src/items.rs` (modify) | `Meta`, `query_meta`, `Item::placeholder/apply`, `spawn_loader` |
| `src/watch.rs` (create) | `Change`, pure `apply_change`, Win32 registration + message decode |
| `src/shell_menu.rs` (create, A3) | on-demand `IContextMenu` for one path |
| `src/app.rs` (modify) | new messages, startup uses placeholders + loader, controller handlers |
| `src/node.rs` (modify, A3) | context menu uses `shell_menu` |
| `src/main.rs` (modify) | `mod watch; mod shell_menu;` |

---

### Task A1: Load item metadata off the UI thread

**Files:**
- Modify: `src/items.rs`, `src/app.rs`

**Interfaces:**
- Produces (`items.rs`):
  `pub struct Meta { pub name: String, pub missing: bool, pub sys_index: i32, pub overlay: u32 }`
  `pub fn query_meta(path: &Path) -> Meta`
  `Item::placeholder(path: PathBuf) -> Item`, `Item::apply(&mut self, m: Meta)`
  `pub fn spawn_loader(controller: HWND, paths: Vec<PathBuf>)`
- Produces (`app.rs`): `pub const WM_APP_ITEMS_LOADED: u32 = WM_APP + 2;` whose `lparam` is `Box<Vec<(PathBuf, Meta)>>` raw pointer.

- [ ] **Step 1: Write the failing tests** (append to `src/items.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_uses_file_name_and_generic_icon() {
        let it = Item::placeholder(PathBuf::from(r"C:\Users\x\Desktop\Notes.txt"));
        assert_eq!(it.name, "Notes.txt");
        assert!(!it.missing);
        assert_eq!(it.sys_index, 0);
    }

    #[test]
    fn apply_replaces_shell_metadata() {
        let mut it = Item::placeholder(PathBuf::from(r"C:\a\b.lnk"));
        it.apply(Meta { name: "b".into(), missing: true, sys_index: 293, overlay: 2 });
        assert_eq!((it.name.as_str(), it.missing, it.sys_index, it.overlay), ("b", true, 293, 2));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release --locked items::tests`
Expected: FAIL to compile (`Meta`, `placeholder`, `apply` not defined).

- [ ] **Step 3: Implement.** In `src/items.rs`:

```rust
/// Shell metadata for one path. Plain data so it can be produced on a worker thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Meta {
    pub name: String,
    pub missing: bool,
    pub sys_index: i32,
    pub overlay: u32,
}

/// Blocking shell query. Safe to call from any thread that has initialised COM.
pub fn query_meta(path: &Path) -> Meta {
    // body = the current body of `Item::load` up to (not including) `Item { .. }`,
    // returning `Meta { name, missing, sys_index, overlay }`.
}
```

Move the existing logic of `Item::load` (the `SHGetFileInfoW` call, `(missing)` suffix, `.lnk` overlay) into `query_meta` unchanged. Then:

```rust
impl Item {
    pub fn placeholder(path: PathBuf) -> Item {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
        Item { path, name, missing: false, sys_index: 0, overlay: 0, icon: std::ptr::null_mut(), icon_px: 0 }
    }

    pub fn apply(&mut self, m: Meta) {
        if self.sys_index != m.sys_index || self.overlay != m.overlay {
            self.release_icon(); // forces re-extraction on next paint
        }
        self.name = m.name;
        self.missing = m.missing;
        self.sys_index = m.sys_index;
        self.overlay = m.overlay;
    }

    pub fn load(path: PathBuf) -> Item {
        let m = query_meta(&path);
        let mut it = Item::placeholder(path);
        it.apply(m);
        it
    }
}

/// Queries `paths` on a worker thread and posts the result to `controller`.
pub fn spawn_loader(controller: HWND, paths: Vec<PathBuf>) {
    if paths.is_empty() {
        return;
    }
    let ctl = controller as usize; // HWND is not Send
    std::thread::spawn(move || unsafe {
        windows_sys::Win32::System::Com::CoInitializeEx(std::ptr::null(), windows_sys::Win32::System::Com::COINIT_APARTMENTTHREADED as u32);
        let metas: Vec<(PathBuf, Meta)> = paths.into_iter().map(|p| { let m = query_meta(&p); (p, m) }).collect();
        let raw = Box::into_raw(Box::new(metas));
        let ok = windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
            ctl as HWND, crate::app::WM_APP_ITEMS_LOADED, 0, raw as isize);
        if ok == 0 {
            drop(Box::from_raw(raw)); // controller gone (shutting down)
        }
        windows_sys::Win32::System::Com::CoUninitialize();
    });
}
```

In `src/app.rs`: add `pub const WM_APP_ITEMS_LOADED: u32 = WM_APP + 2;`. In `run()` replace `paths.into_iter().map(Item::load).collect()` with
`paths.iter().cloned().map(Item::placeholder).collect()`, keep a `let load_paths = paths;` and, **after** `a.controller.set(controller)`, call `items::spawn_loader(controller, load_paths);`. Add the handler in `controller_proc`:

```rust
WM_APP_ITEMS_LOADED => {
    let metas = *Box::from_raw(lparam as *mut Vec<(PathBuf, items::Meta)>);
    {
        let mut items = a.items.borrow_mut();
        for (p, m) in metas {
            if let Some(it) = items.iter_mut().find(|i| i.path == p) {
                it.apply(m);
            }
        }
    } // borrow ends before any window call
    let n = a.node.get();
    if !n.is_null() {
        InvalidateRect(n, std::ptr::null(), 0);
    }
    dlog!("METRIC items_loaded_async ms={:.1}", diag::since_start_ms());
    0
}
```
Remove the old synchronous `METRIC items_loaded` line (it measured the blocking load) or keep it as `items_placeholder`.

- [ ] **Step 4: Run tests and a smoke run**

Run: `cargo test --release --locked` -> PASS.
Run: `cargo build --release; .\target\release\DeskTuk.exe --no-save --allow-multiple --exit-after 1500 --log $env:TEMP\a1.log C:\Windows\notepad.exe`
Expected: log contains `METRIC items_loaded_async` and an `item loaded`-equivalent; process exits 0.

- [ ] **Step 5: Commit** (`Plan: ...phase2-a-item-layer.md Task A1`)

---

### Task A2: Follow shell change notifications

**Files:**
- Create: `src/watch.rs`
- Modify: `src/main.rs` (`mod watch;`), `src/app.rs`

**Interfaces:**
- Consumes: `items::spawn_loader`, `app().items`.
- Produces:
  `pub enum Change { Renamed { from: PathBuf, to: PathBuf }, Removed(PathBuf), Updated(PathBuf), Created(PathBuf) }`
  `pub fn apply_change(paths: &mut [PathBuf], change: &Change) -> Vec<PathBuf>` - rewrites paths in place, returns the paths to re-query.
  `pub fn register(controller: HWND) -> u32`, `pub fn deregister(id: u32)`, `pub fn decode(wparam: usize, lparam: isize) -> Option<Change>`
  `pub const WM_APP_SHELL_CHANGE: u32 = WM_APP + 3;` (in `app.rs`)

- [ ] **Step 1: Write the failing tests** (`src/watch.rs`, `#[cfg(test)]`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf { PathBuf::from(s) }

    #[test]
    fn rename_rewrites_exact_match_case_insensitively() {
        let mut v = vec![p(r"C:\Users\x\Desktop\Old.txt"), p(r"C:\other.txt")];
        let q = apply_change(&mut v, &Change::Renamed { from: p(r"c:\users\x\desktop\old.TXT"), to: p(r"C:\Users\x\Desktop\New.txt") });
        assert_eq!(v[0], p(r"C:\Users\x\Desktop\New.txt"));
        assert_eq!(v[1], p(r"C:\other.txt"));
        assert_eq!(q, vec![p(r"C:\Users\x\Desktop\New.txt")]);
    }

    #[test]
    fn folder_rename_rewrites_prefix_only_on_component_boundary() {
        let mut v = vec![p(r"C:\a\Docs\f.txt"), p(r"C:\a\Docs2\g.txt")];
        apply_change(&mut v, &Change::Renamed { from: p(r"C:\a\Docs"), to: p(r"C:\a\Papers") });
        assert_eq!(v[0], p(r"C:\a\Papers\f.txt"));
        assert_eq!(v[1], p(r"C:\a\Docs2\g.txt"));
    }

    #[test]
    fn removed_and_created_requery_the_same_path() {
        let mut v = vec![p(r"C:\a\x.txt")];
        assert_eq!(apply_change(&mut v, &Change::Removed(p(r"C:\A\X.txt"))), vec![p(r"C:\a\x.txt")]);
        assert_eq!(apply_change(&mut v, &Change::Created(p(r"C:\a\x.txt"))), vec![p(r"C:\a\x.txt")]);
        assert_eq!(v, vec![p(r"C:\a\x.txt")], "removal never deletes the reference");
    }

    #[test]
    fn unrelated_changes_are_a_no_op() {
        let mut v = vec![p(r"C:\a\x.txt")];
        assert!(apply_change(&mut v, &Change::Updated(p(r"C:\b\y.txt"))).is_empty());
    }
}
```

- [ ] **Step 2: Run:** `cargo test --release --locked watch::tests` -> FAIL (module missing).

- [ ] **Step 3: Implement `apply_change`**

```rust
use std::path::{Path, PathBuf};

fn same(a: &Path, b: &Path) -> bool {
    a.as_os_str().eq_ignore_ascii_case(b.as_os_str())
}

/// `Some(rest)` when `path` is `prefix` or lies below it (component-wise, case-insensitive).
fn strip_prefix_ci(path: &Path, prefix: &Path) -> Option<PathBuf> {
    let mut p = path.components();
    for c in prefix.components() {
        match p.next() {
            Some(x) if x.as_os_str().eq_ignore_ascii_case(c.as_os_str()) => {}
            _ => return None,
        }
    }
    Some(p.as_path().to_path_buf())
}

pub fn apply_change(paths: &mut [PathBuf], change: &Change) -> Vec<PathBuf> {
    let mut requery = Vec::new();
    match change {
        Change::Renamed { from, to } => {
            for p in paths.iter_mut() {
                if let Some(rest) = strip_prefix_ci(p, from) {
                    *p = if rest.as_os_str().is_empty() { to.clone() } else { to.join(rest) };
                    requery.push(p.clone());
                }
            }
        }
        Change::Removed(x) | Change::Updated(x) | Change::Created(x) => {
            for p in paths.iter() {
                if same(p, x) {
                    requery.push(p.clone());
                }
            }
        }
    }
    requery
}
```

Win32 part (same file, not unit-tested):

```rust
use windows_sys::Win32::Foundation::{HANDLE, HWND};
use windows_sys::Win32::UI::Shell::{
    SHChangeNotification_Lock, SHChangeNotification_Unlock, SHChangeNotifyDeregister, SHChangeNotifyEntry,
    SHChangeNotifyRegister, SHGetPathFromIDListW, SHCNE_CREATE, SHCNE_DELETE, SHCNE_RENAMEFOLDER, SHCNE_RENAMEITEM,
    SHCNE_UPDATEITEM, SHCNRF_NewDelivery, SHCNRF_ShellLevel,
};

pub fn register(controller: HWND) -> u32 {
    let entry = SHChangeNotifyEntry { pidl: std::ptr::null_mut(), fRecursive: 1 };
    let events = (SHCNE_RENAMEITEM | SHCNE_RENAMEFOLDER | SHCNE_DELETE | SHCNE_UPDATEITEM | SHCNE_CREATE) as i32;
    unsafe {
        SHChangeNotifyRegister(controller, SHCNRF_ShellLevel | SHCNRF_NewDelivery, events, crate::app::WM_APP_SHELL_CHANGE, 1, &entry)
    }
}

pub fn deregister(id: u32) {
    if id != 0 {
        unsafe { SHChangeNotifyDeregister(id) };
    }
}

unsafe fn pidl_path(pidl: *const windows_sys::Win32::UI::Shell::Common::ITEMIDLIST) -> Option<PathBuf> {
    if pidl.is_null() {
        return None;
    }
    let mut buf = [0u16; 520];
    if SHGetPathFromIDListW(pidl, buf.as_mut_ptr()) == 0 {
        return None;
    }
    Some(PathBuf::from(crate::win::from_wide(&buf)))
}

/// Decodes one `SHCNRF_NewDelivery` message (wParam = change handle, lParam = process id).
pub fn decode(wparam: usize, lparam: isize) -> Option<Change> {
    unsafe {
        let mut pidls: *mut *mut windows_sys::Win32::UI::Shell::Common::ITEMIDLIST = std::ptr::null_mut();
        let mut event = 0i32;
        let lock: HANDLE = SHChangeNotification_Lock(wparam as HANDLE, lparam as u32, &mut pidls, &mut event);
        if lock.is_null() || pidls.is_null() {
            return None;
        }
        let first = pidl_path(*pidls);
        let second = pidl_path(*pidls.add(1));
        SHChangeNotification_Unlock(lock);
        let ev = event as u32;
        match (ev, first, second) {
            (e, Some(f), Some(t)) if e == SHCNE_RENAMEITEM || e == SHCNE_RENAMEFOLDER => Some(Change::Renamed { from: f, to: t }),
            (e, Some(f), _) if e == SHCNE_DELETE => Some(Change::Removed(f)),
            (e, Some(f), _) if e == SHCNE_UPDATEITEM => Some(Change::Updated(f)),
            (e, Some(f), _) if e == SHCNE_CREATE => Some(Change::Created(f)),
            _ => None,
        }
    }
}
```

In `src/app.rs`: `pub const WM_APP_SHELL_CHANGE: u32 = WM_APP + 3;`, a field `notify_id: Cell<u32>` on `App`, register right after `create_controller()` (`a.notify_id.set(watch::register(controller))`), deregister in the controller's `WM_DESTROY`, and the handler:

```rust
WM_APP_SHELL_CHANGE => {
    if let Some(change) = watch::decode(wparam, lparam) {
        // Copy paths out, compute, write back: no borrow is held across a window call.
        let mut paths: Vec<PathBuf> = a.items.borrow().iter().map(|i| i.path.clone()).collect();
        let before = paths.clone();
        let requery = watch::apply_change(&mut paths, &change);
        if !requery.is_empty() {
            {
                let mut items = a.items.borrow_mut();
                for (it, p) in items.iter_mut().zip(paths.iter()) {
                    it.path = p.clone();
                }
            }
            if paths != before {
                a.save_config();
            }
            items::spawn_loader(a.controller.get(), requery);
        }
    }
    0
}
```
(`Item::path` is already `pub`.)

- [ ] **Step 4: Run** `cargo test --release --locked` -> PASS. Manual: start DeskTuk with a file on the Desktop as item, rename it in Explorer -> node shows the new name within ~1 s (log line `METRIC items_loaded_async`).

- [ ] **Step 5: Commit** (`Task A2`)

---

### Task A3: Real shell context menu (on demand)

**Files:**
- Create: `src/shell_menu.rs`
- Modify: `Cargo.toml`, `src/main.rs`, `src/node.rs` (`context_menu`)

**Interfaces:**
- Produces: `pub fn show(owner: HWND, path: &Path, screen: POINT, extra: &[(usize, &str)]) -> Choice`
  `pub enum Choice { Shell, Extra(usize), None }` - `Shell` means a shell verb was invoked.
  DeskTuk's own command ids must be `>= 0x8000` (shell ids use `1..=0x7FFF`).

**Why the `windows` crate:** `windows-sys` has no COM interface wrappers (`IContextMenu`, `IShellFolder`). Add `windows` (latest 0.6x release; `cargo add windows --features Win32_UI_Shell,Win32_UI_Shell_Common,Win32_System_Com,Win32_UI_WindowsAndMessaging,Win32_Foundation`). It is only used in this module.

- [ ] **Step 1: Measure the baseline** with the module count/commit script (`scripts/memmap.ps1 -Exe target\release\DeskTuk.exe`) and record `private commit` and `modules` in the commit body.

- [ ] **Step 2: Implement `shell_menu::show`** with this call sequence (all errors -> `Choice::None`, never panic):
  1. `SHParseDisplayName(path)` -> absolute PIDL; `SHBindToParent` -> `IShellFolder` + child PIDL.
  2. `folder.GetUIObjectOf(owner, &[child], &IContextMenu::IID)`.
  3. `CreatePopupMenu`; `ctx.QueryContextMenu(hmenu, 0, 1, 0x7FFF, flags)` where `flags = CMF_NORMAL`, plus `CMF_EXTENDEDVERBS` when Shift is held.
  4. Append a separator and the `extra` entries with ids `>= 0x8000`.
  5. `SetForegroundWindow(owner)` (the controller, as `node.rs` does today), `TrackPopupMenuEx(.., TPM_RETURNCMD | TPM_RIGHTBUTTON, ..)`, then `PostMessageW(owner, WM_NULL, 0, 0)`.
  6. If the result is `1..=0x7FFF`: `InvokeCommand` with `CMINVOKECOMMANDINFO { lpVerb: MAKEINTRESOURCE(id - 1), nShow: SW_SHOWNORMAL, hwnd: owner }`.
  7. Free PIDLs (`CoTaskMemFree`), `DestroyMenu`.

- [ ] **Step 3: Wire into `node.rs::context_menu`.** Replace the hand-built menu for item right-clicks: call `shell_menu::show(ctl, &path, screen, &[(0x8001, "Remove from DeskTuk"), (0x8002, "Host: window"), (0x8010, "Exit DeskTuk")])` and map `Choice::Extra(0x8010)` to `crate::app::quit()`. Keep the existing menu for right-clicks on empty space. Copy the item path into a local **before** calling `show` (RefCell rule).

- [ ] **Step 4: Verify manually and record results in the commit body.**
  * Right-click a `.txt` item: shows Open / Open with / Properties / Send to ... ; "Properties" opens the Properties dialog; Esc dismisses without crash.
  * Right-click a `.lnk`: "Open file location" works.
  * `private commit` and `modules` immediately after first use vs. Step 1 baseline. If idle (before first use) numbers regressed, the crate is being initialised at start-up: fix.
  * Known limitation to accept: sub-menus that need owner-draw (`IContextMenu2/3`, e.g. some "Send to" icons) may render without icons. Do not forward `WM_INITMENUPOPUP` in this plan.

- [ ] **Step 5: Commit** (`Task A3`)

---

## Self-review

* Spec coverage: loading off-thread (A1), sync with FS (A2), shell context menu (A3).
* A1 defines `Meta`, `spawn_loader`, `WM_APP_ITEMS_LOADED`; A2 reuses `spawn_loader` with the same signature; A3 uses only `Item::path`.
* Review Focus items: case-insensitive match + prefix rename (A2 tests), late loader result (A1 handler ignores unknown paths), notification flood (A2 `apply_change` returns empty -> no work), delete/re-create (A2 test), RefCell (A1/A2 handlers scope the borrows).
