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
    FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_PublicDesktop, ILCreateFromPathW, ILFree, SHGetFileInfoW, SHGetIconOverlayIndexW,
    SHGetImageList, SHGetKnownFolderPath, SHOpenFolderAndSelectItems, ShellExecuteExW, IDO_SHGIOI_LINK, SEE_MASK_FLAG_LOG_USAGE,
    SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, SHFILEINFOW, SHGFI_DISPLAYNAME, SHGFI_LARGEICON, SHGFI_SYSICONINDEX,
    SHGFI_USEFILEATTRIBUTES, SHIL_EXTRALARGE, SHIL_JUMBO,
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
    let missing = !path.exists();
    let w = wide_path(path);
    let mut sfi: SHFILEINFOW = unsafe { std::mem::zeroed() };
    // Deliberately no SHGFI_ICON / SHGFI_OVERLAYINDEX: those make the shell load every
    // registered icon-overlay handler (cloud-sync clients, thumbcache, ...), which pulled
    // ~60 extra DLLs and several MB into the process. The shortcut arrow is added below.
    let mut flags = SHGFI_SYSICONINDEX | SHGFI_DISPLAYNAME | SHGFI_LARGEICON;
    if missing {
        flags |= SHGFI_USEFILEATTRIBUTES;
    }
    let ok = unsafe { SHGetFileInfoW(w.as_ptr(), FILE_ATTRIBUTE_NORMAL, &mut sfi, std::mem::size_of::<SHFILEINFOW>() as u32, flags) } != 0;
    let mut name = crate::win::from_wide(&sfi.szDisplayName);
    if name.is_empty() {
        name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
    }
    if missing {
        name = format!("{name} (missing)");
    }
    let sys_index = if ok { sfi.iIcon & 0x00ff_ffff } else { 0 };
    let is_link = path.extension().map(|e| e.eq_ignore_ascii_case("lnk")).unwrap_or(false);
    // System "shortcut" overlay only; no third-party handlers are consulted.
    let overlay =
        if is_link { u32::try_from(unsafe { SHGetIconOverlayIndexW(std::ptr::null(), IDO_SHGIOI_LINK as i32) }).unwrap_or(0) } else { 0 };
    dlog!("item loaded path={} name={name:?} icon_index={sys_index} overlay={overlay} missing={missing}", path.display());
    Meta { name, missing, sys_index, overlay }
}

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

    #[allow(dead_code)]
    pub fn load(path: PathBuf) -> Item {
        let m = query_meta(&path);
        let mut it = Item::placeholder(path);
        it.apply(m);
        it
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
                // SHGetImageList returned a COM reference, not a borrowed global list.
                unsafe {
                    use windows::core::Interface;
                    drop(windows::core::IUnknown::from_raw(himl));
                }
            }
            self.icon_px = px;
        }
        self.icon
    }

    /// Already loaded icon, without shell calls while preparing a drag preview.
    pub fn cached_icon(&self) -> HICON {
        self.icon
    }

    pub fn release_icon(&mut self) {
        if !self.icon.is_null() {
            unsafe { DestroyIcon(self.icon) };
            self.icon = std::ptr::null_mut();
        }
    }

    pub fn set_icon(&mut self, mut icon: LoadedIcon, px: i32) {
        if icon.0 != 0 {
            self.release_icon();
            self.icon = icon.0 as HICON;
            self.icon_px = px;
            icon.0 = 0; // ownership transferred to Item
        }
    }
}

/// Owned HICON transported as plain data between threads. Dropping an obsolete batch
/// or a failed PostMessage releases every icon, just like closing a ring does.
pub struct LoadedIcon(usize);

impl Drop for LoadedIcon {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe { DestroyIcon(self.0 as HICON) };
        }
    }
}

pub struct RingBatch {
    pub request: u64,
    pub px: i32,
    pub entries: Vec<(PathBuf, Meta, LoadedIcon)>,
}

/// Request a bitmap at the actual display size rather than shrinking a padded jumbo
/// image-list cell. This runs only on the loader thread, never on the UI thread.
fn sized_icon(path: &Path, px: i32) -> LoadedIcon {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_ICONONLY, SIIGBF_SCALEUP};
    use windows_sys::Win32::Graphics::Gdi::{CreateBitmap, DeleteObject, GetObjectW, BITMAP};
    use windows_sys::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, ICONINFO};
    let w = wide_path(path);
    unsafe {
        let Ok(factory) = SHCreateItemFromParsingName::<_, _, IShellItemImageFactory>(PCWSTR(w.as_ptr()), None) else {
            return LoadedIcon(0);
        };
        let Ok(image) = factory.GetImage(SIZE { cx: px, cy: px }, SIIGBF_ICONONLY | SIIGBF_SCALEUP) else {
            return LoadedIcon(0);
        };
        let mut bitmap: BITMAP = std::mem::zeroed();
        let mut icon = std::ptr::null_mut();
        if GetObjectW(image.0, std::mem::size_of::<BITMAP>() as i32, (&mut bitmap as *mut BITMAP).cast()) != 0
            && (1..=512).contains(&bitmap.bmWidth)
            && (1..=512).contains(&bitmap.bmHeight)
        {
            let zeros = vec![0u8; ((bitmap.bmWidth as usize).div_ceil(16) * 2) * bitmap.bmHeight as usize];
            let mask = CreateBitmap(bitmap.bmWidth, bitmap.bmHeight, 1, 1, zeros.as_ptr().cast());
            if !mask.is_null() {
                let info = ICONINFO { fIcon: 1, xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: image.0 };
                icon = CreateIconIndirect(&info);
                DeleteObject(mask);
            }
        }
        DeleteObject(image.0);
        LoadedIcon(icon as usize)
    }
}

pub fn spawn_ring_loader(controller: HWND, paths: Vec<PathBuf>, px: i32, request: u64) {
    if paths.is_empty() {
        return;
    }
    let ctl = controller as usize;
    std::thread::spawn(move || unsafe {
        use windows_sys::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
        let hr = CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
        let entries = paths
            .into_iter()
            .map(|p| {
                let meta = query_meta(&p);
                let icon = if hr >= 0 { sized_icon(&p, px) } else { LoadedIcon(0) };
                (p, meta, icon)
            })
            .collect();
        let raw = Box::into_raw(Box::new(RingBatch { request, px, entries }));
        if windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(ctl as HWND, crate::app::WM_APP_RING_LOADED, 0, raw as isize) == 0 {
            drop(Box::from_raw(raw));
        }
        if hr >= 0 {
            CoUninitialize();
        }
    });
}

/// Queries `paths` on a worker thread and posts the result to `controller`.
pub fn spawn_loader(controller: HWND, paths: Vec<PathBuf>) {
    if paths.is_empty() {
        return;
    }
    let ctl = controller as usize; // HWND is not Send
    std::thread::spawn(move || unsafe {
        windows_sys::Win32::System::Com::CoInitializeEx(std::ptr::null(), windows_sys::Win32::System::Com::COINIT_APARTMENTTHREADED as u32);
        let metas: Vec<(PathBuf, Meta)> = paths
            .into_iter()
            .map(|p| {
                let m = query_meta(&p);
                (p, m)
            })
            .collect();
        let raw = Box::into_raw(Box::new(metas));
        let ok = windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(ctl as HWND, crate::app::WM_APP_ITEMS_LOADED, 0, raw as isize);
        if ok == 0 {
            drop(Box::from_raw(raw)); // controller gone (shutting down)
        }
        windows_sys::Win32::System::Com::CoUninitialize();
    });
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
