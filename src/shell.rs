//! Read-only inspection of the Explorer desktop window structure and shell settings.
//!
//! The class names used here (`Progman`, `WorkerW`, `SHELLDLL_DefView`, `SysListView32`)
//! are long-standing but *undocumented* implementation details of Explorer. Their
//! arrangement differs between Windows versions (e.g. Windows 11 24H2 moved the
//! wallpaper `WorkerW` to be a child of `Progman`). Nothing in this module modifies
//! Explorer; callers must cope with every lookup failing.

use std::ptr::null;

use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ};
use windows_sys::Win32::UI::Shell::{SHGetSetSettings, SHELLSTATEA, SSF_HIDEICONS};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, FindWindowW, GetAncestor, GetParent, GetWindowLongW, GetWindowThreadProcessId, IsWindowVisible, GA_ROOT,
    GWL_STYLE, WS_CLIPSIBLINGS,
};

use crate::win::{class_name, wide};

pub fn find_progman() -> HWND {
    let cls = wide("Progman");
    unsafe { FindWindowW(cls.as_ptr(), null()) }
}

fn find_child(parent: HWND, class: &str) -> HWND {
    let cls = wide(class);
    unsafe { FindWindowExW(parent, std::ptr::null_mut(), cls.as_ptr(), null()) }
}

/// Where Explorer's desktop icon view currently lives.
#[derive(Clone, Copy, Debug)]
pub struct DesktopWindows {
    /// `SHELLDLL_DefView` - hosts the desktop icon list view.
    pub defview: HWND,
    /// Parent of `defview`: `Progman` normally, a top-level `WorkerW` after a wallpaper
    /// layer was spawned on pre-24H2 builds.
    pub defview_parent: HWND,
    /// `SysListView32` inside the def view (the actual icons).
    pub listview: HWND,
}

pub fn desktop_windows() -> Option<DesktopWindows> {
    let progman = find_progman();
    if progman.is_null() {
        return None;
    }
    let mut defview = find_child(progman, "SHELLDLL_DefView");
    let mut parent = progman;
    if defview.is_null() {
        // Pre-24H2 layout: the def view may have been moved into a top-level WorkerW.
        let mut found: (HWND, HWND) = (std::ptr::null_mut(), std::ptr::null_mut());
        unsafe extern "system" fn cb(hwnd: HWND, l: LPARAM) -> windows_sys::core::BOOL {
            let out = &mut *(l as *mut (HWND, HWND));
            if class_name(hwnd) == "WorkerW" {
                let dv = find_child(hwnd, "SHELLDLL_DefView");
                if !dv.is_null() {
                    *out = (dv, hwnd);
                    return 0;
                }
            }
            1
        }
        unsafe { EnumWindows(Some(cb), &mut found as *mut _ as LPARAM) };
        defview = found.0;
        parent = found.1;
    }
    if defview.is_null() {
        return None;
    }
    Some(DesktopWindows { defview, defview_parent: parent, listview: find_child(defview, "SysListView32") })
}

/// True if `hwnd` is the window Explorer activates for "Show desktop" / a desktop click.
pub fn is_desktop_window(hwnd: HWND) -> bool {
    if hwnd.is_null() {
        return false;
    }
    match class_name(hwnd).as_str() {
        "Progman" => true,
        "WorkerW" => !find_child(hwnd, "SHELLDLL_DefView").is_null(),
        _ => false,
    }
}

/// True if `hwnd` belongs to the desktop (icon view / wallpaper) window tree.
pub fn is_desktop_tree(hwnd: HWND) -> bool {
    if hwnd.is_null() {
        return false;
    }
    let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    let c = class_name(root);
    c == "Progman" || c == "WorkerW"
}

pub fn window_pid(hwnd: HWND) -> u32 {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    pid
}

/// Explorer's user setting "View > Show desktop icons" (SHELLSTATE.fHideIcons).
pub fn desktop_icons_hidden() -> bool {
    unsafe {
        let mut ss: SHELLSTATEA = std::mem::zeroed();
        SHGetSetSettings(&mut ss, SSF_HIDEICONS, 0);
        // fHideIcons is bit 12 of the first SHELLSTATE bitfield.
        (ss._bitfield1 >> 12) & 1 == 1
    }
}

fn reg_string(key: &str, value: &str) -> Option<String> {
    let k = wide(key);
    let v = wide(value);
    let mut buf = [0u16; 128];
    let mut size = (buf.len() * 2) as u32;
    let rc = unsafe {
        RegGetValueW(HKEY_LOCAL_MACHINE, k.as_ptr(), v.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut size)
    };
    (rc == 0).then(|| crate::win::from_wide(&buf))
}

fn reg_dword(root: windows_sys::Win32::System::Registry::HKEY, key: &str, value: &str) -> Option<u32> {
    let k = wide(key);
    let v = wide(value);
    let mut out = 0u32;
    let mut size = 4u32;
    let rc = unsafe {
        RegGetValueW(root, k.as_ptr(), v.as_ptr(), RRF_RT_REG_DWORD, std::ptr::null_mut(), (&mut out as *mut u32).cast(), &mut size)
    };
    (rc == 0).then_some(out)
}

/// e.g. "Windows Server 2022 Datacenter 21H2 build 20348.2849"
pub fn windows_version() -> String {
    const CV: &str = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion";
    let product = reg_string(CV, "ProductName").unwrap_or_default();
    let display = reg_string(CV, "DisplayVersion").unwrap_or_default();
    let build = reg_string(CV, "CurrentBuild").unwrap_or_default();
    let ubr = reg_dword(HKEY_LOCAL_MACHINE, CV, "UBR").map(|u| format!(".{u}")).unwrap_or_default();
    format!("{product} {display} build {build}{ubr}")
}

/// Value of HKCU\...\Explorer\Advanced\HideIcons (the registry backing of fHideIcons).
pub fn hide_icons_registry() -> Option<u32> {
    reg_dword(HKEY_CURRENT_USER, "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced", "HideIcons")
}

/// Human-readable dump of the desktop window structure, for the log.
pub fn describe() -> String {
    let mut s = String::new();
    let progman = find_progman();
    s += &format!("progman={progman:?}");
    if !progman.is_null() {
        s += &format!(" progman_pid={} progman_visible={}", window_pid(progman), unsafe { IsWindowVisible(progman) } != 0);
        let tray = unsafe { FindWindowW(wide("Shell_TrayWnd").as_ptr(), null()) };
        s += &format!(" tray={tray:?}");
    }
    match desktop_windows() {
        Some(d) => {
            // WS_CLIPSIBLINGS on the def view decides whether it can paint over a sibling node.
            let style = unsafe { GetWindowLongW(d.defview, GWL_STYLE) } as u32;
            s += &format!(
                " defview={:?} defview_style=0x{style:08x} defview_clipsiblings={} defview_parent={:?}({}) listview={:?}",
                d.defview,
                style & WS_CLIPSIBLINGS != 0,
                d.defview_parent,
                class_name(d.defview_parent),
                d.listview
            );
        }
        None => s += " defview=<not found>",
    }
    // Count WorkerW windows: top-level and (24H2+) children of Progman.
    let mut top_level_workerw = 0u32;
    unsafe extern "system" fn cb(hwnd: HWND, l: LPARAM) -> windows_sys::core::BOOL {
        if class_name(hwnd) == "WorkerW" && GetParent(hwnd).is_null() {
            *(l as *mut u32) += 1;
        }
        1
    }
    unsafe { EnumWindows(Some(cb), &mut top_level_workerw as *mut u32 as LPARAM) };
    let child_workerw = if progman.is_null() { std::ptr::null_mut() } else { find_child(progman, "WorkerW") };
    s += &format!(" toplevel_workerw={top_level_workerw} progman_child_workerw={}", !child_workerw.is_null());
    s
}
