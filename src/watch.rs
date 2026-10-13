//! Follows shell change notifications so items track renames, deletes and updates made in Explorer.

use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::{HANDLE, HWND};
use windows_sys::Win32::UI::Shell::Common::ITEMIDLIST;
use windows_sys::Win32::UI::Shell::{
    SHCNRF_NewDelivery, SHCNRF_ShellLevel, SHChangeNotification_Lock, SHChangeNotification_Unlock, SHChangeNotifyDeregister,
    SHChangeNotifyEntry, SHChangeNotifyRegister, SHGetPathFromIDListW, SHCNE_CREATE, SHCNE_DELETE, SHCNE_RENAMEFOLDER, SHCNE_RENAMEITEM,
    SHCNE_UPDATEITEM,
};

/// One decoded shell notification about a path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Renamed { from: PathBuf, to: PathBuf },
    Removed(PathBuf),
    Updated(PathBuf),
    Created(PathBuf),
}

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

/// Rewrites `paths` in place for `change` and returns the paths whose shell metadata must be re-queried.
/// A removal never deletes a reference: the item stays and is re-queried (it becomes "missing").
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

/// Registers `controller` for shell change messages (`WM_APP_SHELL_CHANGE`). Returns 0 on failure.
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

unsafe fn pidl_path(pidl: *const ITEMIDLIST) -> Option<PathBuf> {
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
        let mut pidls: *mut *mut ITEMIDLIST = std::ptr::null_mut();
        let mut event = 0i32;
        let lock: HANDLE = SHChangeNotification_Lock(wparam as HANDLE, lparam as u32, &mut pidls, &mut event);
        if lock.is_null() {
            return None;
        }
        if pidls.is_null() {
            SHChangeNotification_Unlock(lock);
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
#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

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
