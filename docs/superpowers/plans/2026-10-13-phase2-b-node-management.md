# Phase 2 / Plan B - Node management

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax. Read `AGENTS.md` first.

**Goal:** The owner can build a Node without editing a config file: drop files on it, remove items from its menu, find it again from a tray icon, and have it start with Windows.

**Architecture:** Small pure functions (`merge_paths`, `remove_path`, `run_value`) carry the logic and are unit-tested; thin Win32 glue calls them: `WM_DROPFILES` in `node.rs`, a tray module on the controller window, and `HKCU\...\Run` for autostart.

**Tech Stack:** Rust, `windows-sys 0.61` (features `Win32_UI_Shell`, `Win32_System_Registry`, `Win32_UI_WindowsAndMessaging` are already enabled).

**Spec:** `docs/superpowers/specs/2026-10-13-phase-2-design.md`
**Depends on:** Plan A (uses `items::spawn_loader`, the `shell_menu::show` extra-entries API and `WM_APP + 1..3`).

## Global Constraints

* Never move/copy/delete a dropped file. Dropping stores a **reference** (path) only.
* RefCell rule from the spec: no `items` borrow across message-pumping calls.
* Autostart writes only `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, value name `DeskTuk`, and removes exactly that value. No scheduled tasks, no services.
* Pre-commit: `cargo fmt`, `cargo clippy --release --locked -- -D warnings`, `cargo test --release --locked`.
* New window messages continue the numbering: `WM_APP + 4` = tray.

## Review Focus

* Dropping the same file twice, or the same file with different case: no duplicate item.
* Dropping a file that does not exist / a path longer than `MAX_PATH` / a folder: folders accepted, missing paths rejected, long paths handled (buffer sized by `DragQueryFileW` length query).
* Dropping 200 files at once: one config save, one relayout.
* Removing the last item: Node still shows (empty state, `layout` already uses `count.max(1)`).
* Exe path containing spaces or non-ASCII characters in the autostart value (must be quoted).
* Explorer restarts: tray icon reappears (`TaskbarCreated`).
* Autostart toggled while the registry value was removed externally: menu check state is read from the registry each time the menu opens, not cached.

## File structure

| File | Responsibility |
|------|----------------|
| `src/items.rs` (modify) | `merge_paths`, `remove_path` (pure) |
| `src/node.rs` (modify) | `DragAcceptFiles`, `WM_DROPFILES`, "Remove" menu command |
| `src/tray.rs` (create) | tray icon add/remove/menu |
| `src/autostart.rs` (create) | `run_value` (pure), `is_enabled`, `set_enabled` |
| `src/app.rs` (modify) | tray message routing, re-add on `TaskbarCreated` |

---

### Task B1: Drop files onto the Node to add them

**Files:**
- Modify: `src/items.rs`, `src/node.rs`

**Interfaces:**
- Consumes: `Item::placeholder`, `items::spawn_loader` (Plan A), `App::save_config`, `node::reposition`.
- Produces: `pub fn merge_paths(existing: &[PathBuf], dropped: Vec<PathBuf>) -> Vec<PathBuf>` - the subset of `dropped` that is new (case-insensitive, de-duplicated, existing on disk), in drop order.

- [ ] **Step 1: Failing tests** (`src/items.rs` tests module)

```rust
#[test]
fn merge_skips_known_duplicate_and_missing_paths() {
    let dir = std::env::temp_dir().join("desktuk-merge-test");
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("A.txt");
    let b = dir.join("b.txt");
    std::fs::write(&a, b"x").unwrap();
    std::fs::write(&b, b"x").unwrap();
    let existing = vec![a.clone()];
    let dropped = vec![
        PathBuf::from(a.to_string_lossy().to_lowercase()), // same file, different case
        b.clone(),
        b.clone(),                                        // duplicate in one drop
        dir.join("nope.txt"),                             // does not exist
    ];
    assert_eq!(merge_paths(&existing, dropped), vec![b]);
}
```

- [ ] **Step 2: Run** `cargo test --release --locked items::tests::merge` -> FAIL (fn missing).

- [ ] **Step 3: Implement** in `src/items.rs`:

```rust
pub fn merge_paths(existing: &[PathBuf], dropped: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for p in dropped {
        let known = existing.iter().chain(out.iter()).any(|e| e.as_os_str().eq_ignore_ascii_case(p.as_os_str()));
        if !known && p.exists() {
            out.push(p);
        }
    }
    out
}
```

In `src/node.rs`: in `create()` after the window exists call `unsafe { windows_sys::Win32::UI::Shell::DragAcceptFiles(hwnd, 1) };`. Add to `wndproc`:

```rust
WM_DROPFILES => {
    use windows_sys::Win32::UI::Shell::{DragFinish, DragQueryFileW, HDROP};
    let hdrop = wparam as HDROP;
    let n = DragQueryFileW(hdrop, u32::MAX, std::ptr::null_mut(), 0);
    let mut dropped = Vec::new();
    for i in 0..n {
        let len = DragQueryFileW(hdrop, i, std::ptr::null_mut(), 0) as usize;
        let mut buf = vec![0u16; len + 1];
        DragQueryFileW(hdrop, i, buf.as_mut_ptr(), buf.len() as u32);
        dropped.push(PathBuf::from(win::from_wide(&buf)));
    }
    DragFinish(hdrop);
    let existing: Vec<PathBuf> = a.items.borrow().iter().map(|i| i.path.clone()).collect();
    let fresh = items::merge_paths(&existing, dropped);
    if !fresh.is_empty() {
        dlog!("dropped {} new item(s)", fresh.len());
        {
            let mut items = a.items.borrow_mut();
            items.extend(fresh.iter().cloned().map(items::Item::placeholder));
        }
        a.save_config();
        reposition(hwnd);
        items::spawn_loader(a.controller.get(), fresh);
    }
    0
}
```

- [ ] **Step 4: Verify.** `cargo test --release --locked` -> PASS. **Manual (mandatory):** drag a file from the Desktop onto the Node; the node grows to include it **and the file is still on the Desktop afterwards**. If Explorer treats the drop as a *move* and the file disappears from the source, stop and report in the commit body - do not ship.

- [ ] **Step 5: Commit** (`Task B1`)

---

### Task B2: "Remove from DeskTuk" menu command

**Files:**
- Modify: `src/items.rs`, `src/node.rs`

**Interfaces:**
- Produces: `pub fn remove_path(items: &mut Vec<Item>, path: &Path) -> bool`.
- Consumes: the `shell_menu::show` extra-entry API from Plan A Task A3 (`Choice::Extra(0x8001)` = remove).

- [ ] **Step 1: Failing test**

```rust
#[test]
fn remove_path_is_case_insensitive_and_reports_result() {
    let mut v = vec![Item::placeholder(PathBuf::from(r"C:\a\X.txt")), Item::placeholder(PathBuf::from(r"C:\a\y.txt"))];
    assert!(remove_path(&mut v, Path::new(r"c:\A\x.TXT")));
    assert_eq!(v.len(), 1);
    assert!(!remove_path(&mut v, Path::new(r"C:\a\zzz.txt")));
}
```

- [ ] **Step 2: Run** -> FAIL.

- [ ] **Step 3: Implement**

```rust
pub fn remove_path(items: &mut Vec<Item>, path: &Path) -> bool {
    let before = items.len();
    items.retain(|i| !i.path.as_os_str().eq_ignore_ascii_case(path.as_os_str()));
    items.len() != before
}
```

In `node::context_menu`, on `Choice::Extra(0x8001)`: take the path local (already copied before showing the menu), then
```rust
let removed = { remove_path(&mut a.items.borrow_mut(), &path) };
if removed {
    a.selected.set(None);
    a.hover.set(None);
    a.save_config();
    reposition(hwnd);
}
```
Dropping an `Item` calls `release_icon()` via `Drop`, so no icon leak.

- [ ] **Step 4: Run** `cargo test --release --locked` -> PASS. Manual: right-click item -> "Remove from DeskTuk"; item disappears; file untouched; config file no longer lists it.

- [ ] **Step 5: Commit** (`Task B2`)

---

### Task B3: Tray icon and "Start with Windows"

**Files:**
- Create: `src/tray.rs`, `src/autostart.rs`
- Modify: `src/main.rs` (`mod tray; mod autostart;`), `src/app.rs`

**Interfaces:**
- Produces:
  `autostart::run_value(exe: &Path) -> String` (quoted path), `autostart::is_enabled() -> bool`, `autostart::set_enabled(on: bool)`
  `tray::add(controller: HWND)`, `tray::remove(controller: HWND)`, `tray::on_message(controller: HWND, lparam: isize)`
  `pub const WM_APP_TRAY: u32 = WM_APP + 4;` in `app.rs`

- [ ] **Step 1: Failing test** (`src/autostart.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn run_value_quotes_paths_with_spaces() {
        assert_eq!(run_value(Path::new(r"C:\Program Files\DeskTuk\DeskTuk.exe")), r#""C:\Program Files\DeskTuk\DeskTuk.exe""#);
        assert_eq!(run_value(Path::new(r"C:\Üser\DeskTuk.exe")), r#""C:\Üser\DeskTuk.exe""#);
    }
}
```

- [ ] **Step 2: Run** -> FAIL (module missing).

- [ ] **Step 3: Implement `autostart.rs`**

```rust
use std::path::Path;

use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegGetValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ,
    RRF_RT_REG_SZ,
};

use crate::win::wide;

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE: &str = "DeskTuk";

pub fn run_value(exe: &Path) -> String {
    format!("\"{}\"", exe.display())
}

pub fn is_enabled() -> bool {
    let (k, v) = (wide(RUN_KEY), wide(VALUE));
    let mut size = 0u32;
    unsafe { RegGetValueW(HKEY_CURRENT_USER, k.as_ptr(), v.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), std::ptr::null_mut(), &mut size) == 0 }
}

pub fn set_enabled(on: bool) {
    let k = wide(RUN_KEY);
    let mut key: HKEY = std::ptr::null_mut();
    unsafe {
        if RegOpenKeyExW(HKEY_CURRENT_USER, k.as_ptr(), 0, KEY_SET_VALUE, &mut key) != 0 {
            return;
        }
        let name = wide(VALUE);
        if on {
            if let Ok(exe) = std::env::current_exe() {
                let data = wide(run_value(&exe));
                RegSetValueExW(key, name.as_ptr(), 0, REG_SZ, data.as_ptr().cast(), (data.len() * 2) as u32);
            }
        } else {
            RegDeleteValueW(key, name.as_ptr());
        }
        RegCloseKey(key);
    }
}
```

`tray.rs`: build a `NOTIFYICONDATAW` zeroed with `cbSize = size_of::<NOTIFYICONDATAW>()`, `hWnd = controller`, `uID = 1`, `uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP`, `uCallbackMessage = WM_APP_TRAY`, `hIcon = LoadIconW(GetModuleHandleW(null), 1 as PCWSTR)` (resource id 1 is `res/desktuk.ico`), `szTip` = "DeskTuk" as UTF-16. `Shell_NotifyIconW(NIM_ADD, &nid)`, then set `Anonymous.uVersion = NOTIFYICON_VERSION_4` and `Shell_NotifyIconW(NIM_SETVERSION, &nid)`. `remove` = `NIM_DELETE` with the same hWnd/uID. `on_message`: with version 4 the event is `lparam & 0xffff`; on `WM_CONTEXTMENU` or `WM_RBUTTONUP` show a popup menu owned by the controller (`SetForegroundWindow`, `TrackPopupMenu(TPM_RETURNCMD)`, `PostMessageW(WM_NULL)` - same pattern as `node::context_menu`) with entries **Show Node** (`reposition` + `ShowWindow(node, SW_SHOWNOACTIVATE)`), **Start with Windows** (`MF_CHECKED` iff `autostart::is_enabled()` read at menu-open time), separator, **Exit** (`app::quit()`).

In `app.rs`: `pub const WM_APP_TRAY: u32 = WM_APP + 4;`; call `tray::add(controller)` after the node is created; `tray::remove(a.controller.get())` in `quit()` **before** `DestroyWindow`; route `WM_APP_TRAY => { tray::on_message(hwnd, lparam); 0 }` in `controller_proc`; call `tray::add(a.controller.get())` at the top of `on_shell_restarted()` (Explorer recreated the taskbar, icon must be re-added).

- [ ] **Step 4: Verify.** `cargo test --release --locked` -> PASS. Manual: icon appears; right-click menu works; toggling "Start with Windows" creates/removes the `DeskTuk` value in `HKCU\...\Run` (`reg query HKCU\Software\Microsoft\Windows\CurrentVersion\Run /v DeskTuk`); killing and restarting `explorer.exe` re-adds the icon (log shows `TaskbarCreated`). **Leave autostart OFF when you finish testing.**

- [ ] **Step 5: Commit** (`Task B3`)

---

## Self-review

* Coverage: add (B1), remove (B2), find again + autostart (B3).
* Names consistent: `merge_paths`/`remove_path` (items.rs), `Choice::Extra(0x8001)` matches Plan A A3 ids, `WM_APP_TRAY = WM_APP + 4` continues A's `+1..+3`.
* Review Focus: duplicates (B1 test), missing path (B1 test), last-item removal (layout `count.max(1)`), quoting (B3 test), `TaskbarCreated` (B3 wiring), registry state read at menu open (B3).
