//! Node items: real files, folders and shortcuts referenced by path.
//!
//! Items are *references*. DeskTuk never moves, renames, deletes or changes the
//! attributes of the underlying file; it only reads shell metadata (display name,
//! icon) and asks the shell to open it.

use std::path::{Path, PathBuf};

use windows_sys::core::GUID;
use windows_sys::Win32::Foundation::{CloseHandle, HWND};
use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::System::Threading::GetProcessId;
use windows_sys::Win32::UI::Controls::{ImageList_GetIcon, HIMAGELIST, ILD_TRANSPARENT};
use windows_sys::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_PublicDesktop, ILCreateFromPathW, ILFree, SHGetFileInfoW, SHGetImageList,
    SHGetKnownFolderPath, SHOpenFolderAndSelectItems, ShellExecuteExW, SEE_MASK_FLAG_LOG_USAGE, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
    SHFILEINFOW, SHGFI_DISPLAYNAME, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_OVERLAYINDEX, SHGFI_SYSICONINDEX, SHGFI_USEFILEATTRIBUTES,
    SHIL_EXTRALARGE, SHIL_JUMBO,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, HICON, SW_SHOWNORMAL};

use crate::dlog;
use crate::win::{os_from_pwstr, wide_path};

/// IID_IImageList {46EB5926-582E-4017-9FDF-E8998DAA0950}
const IID_IIMAGELIST: GUID = GUID::from_u128(0x46eb5926_582e_4017_9fdf_e8998daa0950);

pub struct Item {
    pub path: PathBuf,
    pub name: String,
    pub missing: bool,
    sys_index: i32,
    overlay: u32,
    /// Cached icon rendered for `icon_px`.
    icon: HICON,
    icon_px: i32,
}

impl Item {
    pub fn load(path: PathBuf) -> Item {
        let missing = !path.exists();
        let w = wide_path(&path);
        let mut sfi: SHFILEINFOW = unsafe { std::mem::zeroed() };
        let mut flags = SHGFI_SYSICONINDEX | SHGFI_DISPLAYNAME | SHGFI_ICON | SHGFI_LARGEICON | SHGFI_OVERLAYINDEX;
        if missing {
            flags |= SHGFI_USEFILEATTRIBUTES;
        }
        let ok =
            unsafe { SHGetFileInfoW(w.as_ptr(), FILE_ATTRIBUTE_NORMAL, &mut sfi, std::mem::size_of::<SHFILEINFOW>() as u32, flags) } != 0;
        if !sfi.hIcon.is_null() {
            // Only requested because SHGFI_OVERLAYINDEX requires SHGFI_ICON.
            unsafe { DestroyIcon(sfi.hIcon) };
        }
        let mut name = crate::win::from_wide(&sfi.szDisplayName);
        if name.is_empty() {
            name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
        }
        if missing {
            name = format!("{name} (missing)");
        }
        let (sys_index, overlay) = if ok { (sfi.iIcon & 0x00ff_ffff, ((sfi.iIcon as u32) >> 24) & 0xff) } else { (0, 0) };
        dlog!("item loaded path={} name={name:?} icon_index={sys_index} overlay={overlay} missing={missing}", path.display());
        Item { path, name, missing, sys_index, overlay, icon: std::ptr::null_mut(), icon_px: 0 }
    }

    /// Returns an icon for drawing at `px` pixels, extracting it from the shell's
    /// system image list (the same icons Explorer shows, including the shortcut overlay).
    pub fn icon(&mut self, px: i32) -> HICON {
        if self.icon.is_null() || self.icon_px != px {
            self.release_icon();
            let list = if px <= 48 { SHIL_EXTRALARGE } else { SHIL_JUMBO };
            let mut himl: *mut core::ffi::c_void = std::ptr::null_mut();
            let hr = unsafe { SHGetImageList(list as i32, &IID_IIMAGELIST, &mut himl) };
            if hr >= 0 && !himl.is_null() {
                // An IImageList* may be used directly as an HIMAGELIST (documented interop).
                let style = ILD_TRANSPARENT | (self.overlay << 8); // INDEXTOOVERLAYMASK
                self.icon = unsafe { ImageList_GetIcon(himl as HIMAGELIST, self.sys_index, style) };
            }
            self.icon_px = px;
        }
        self.icon
    }

    pub fn release_icon(&mut self) {
        if !self.icon.is_null() {
            unsafe { DestroyIcon(self.icon) };
            self.icon = std::ptr::null_mut();
        }
    }
}

/// Opens the item with its default shell verb, exactly like a double-click in Explorer.
/// Returns the launched process id when the shell reports one (0 otherwise; e.g. a
/// folder opened in an existing Explorer window or a DDE/COM activation).
pub fn open(path: &Path, owner: HWND) -> Result<u32, u32> {
    let file = wide_path(path);
    let is_link = path.extension().map(|e| e.eq_ignore_ascii_case("lnk")).unwrap_or(false);
    // Like Explorer, start programs in their own folder; shortcuts keep their own "Start in".
    let dir = if is_link { None } else { path.parent().map(wide_path) };
    let mut sei: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    sei.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    sei.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_LOG_USAGE;
    sei.hwnd = owner;
    sei.lpFile = file.as_ptr();
    sei.lpDirectory = dir.as_ref().map(|d| d.as_ptr()).unwrap_or(std::ptr::null());
    sei.nShow = SW_SHOWNORMAL;
    let ok = unsafe { ShellExecuteExW(&mut sei) } != 0;
    if !ok {
        let err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        dlog!("LAUNCH failed path={} error={err}", path.display());
        return Err(err);
    }
    let mut pid = 0;
    if !sei.hProcess.is_null() {
        pid = unsafe { GetProcessId(sei.hProcess) };
        unsafe { CloseHandle(sei.hProcess) };
    }
    dlog!("LAUNCH ok path={} pid={pid}", path.display());
    Ok(pid)
}

/// Opens the containing folder in Explorer with the item selected.
pub fn open_location(path: &Path) {
    let w = wide_path(path);
    unsafe {
        let pidl = ILCreateFromPathW(w.as_ptr());
        if !pidl.is_null() {
            SHOpenFolderAndSelectItems(pidl, 0, std::ptr::null(), 0);
            ILFree(pidl);
        }
    }
}

impl Drop for Item {
    fn drop(&mut self) {
        self.release_icon();
    }
}

pub fn known_folder(id: &GUID) -> Option<PathBuf> {
    let mut p: windows_sys::core::PWSTR = std::ptr::null_mut();
    let hr = unsafe { SHGetKnownFolderPath(id, 0, std::ptr::null_mut(), &mut p) };
    let out = if hr >= 0 { Some(PathBuf::from(unsafe { os_from_pwstr(p) })) } else { None };
    if !p.is_null() {
        unsafe { CoTaskMemFree(p as *const _) };
    }
    out
}

fn listing(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| !p.file_name().map(|n| n.eq_ignore_ascii_case("desktop.ini")).unwrap_or(true))
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    v
}

/// Initial items for a first run: a few entries already on the user's desktop,
/// padded with well-known system targets so the node always has at least two items.
pub fn default_items() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for id in [&FOLDERID_Desktop, &FOLDERID_PublicDesktop] {
        if let Some(dir) = known_folder(id) {
            for p in listing(&dir) {
                if out.len() < 4 {
                    out.push(p);
                }
            }
        }
    }
    let windir = std::env::var_os("WINDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\Windows"));
    let fallbacks = [Some(windir.join("System32").join("notepad.exe")), known_folder(&FOLDERID_Documents)];
    for p in fallbacks.into_iter().flatten() {
        if out.len() >= 2 {
            break;
        }
        if p.exists() && !out.contains(&p) {
            out.push(p);
        }
    }
    out
}
