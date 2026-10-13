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
