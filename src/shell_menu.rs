//! On-demand shell context menu (`IContextMenu`) for one path.
//!
//! Everything here runs only when the user right-clicks an item, so the shell-extension DLLs
//! the menu loads never contribute to the idle footprint. All failures degrade to
//! `Choice::None`; nothing in this module may panic (`panic = "abort"`).

use std::ffi::c_void;
use std::path::Path;

use windows::core::{PCSTR, PCWSTR};
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    IContextMenu, IShellFolder, SHBindToParent, SHParseDisplayName, CMF_EXTENDEDVERBS, CMF_NORMAL, CMINVOKECOMMANDINFO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, PostMessageW, SetForegroundWindow, TrackPopupMenuEx, MF_SEPARATOR, MF_STRING, SW_SHOWNORMAL,
    TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_NULL,
};

/// Shell command ids occupy `1..=SHELL_LAST`; DeskTuk's own entries must be above it.
const SHELL_FIRST: u32 = 1;
const SHELL_LAST: u32 = 0x7FFF;
pub const EXTRA_FIRST: usize = 0x8000;

#[derive(Debug, PartialEq, Eq)]
pub enum Choice {
    /// A shell verb was invoked.
    Shell,
    /// One of the caller's `extra` entries (the id the caller passed).
    Extra(usize),
    /// The user dismissed the menu.
    None,
    /// The shell menu could not be built (e.g. the path no longer exists); nothing was shown.
    Failed,
}

/// Owns a PIDL allocated by the shell.
struct Pidl(*mut ITEMIDLIST);

impl Drop for Pidl {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CoTaskMemFree(Some(self.0 as *const c_void)) };
        }
    }
}

/// Pops up the shell context menu for `path` at `screen`, plus `extra` entries (ids `>= 0x8000`).
/// `owner` receives the menu's messages; it should be the hidden controller window.
pub fn show(
    owner: windows_sys::Win32::Foundation::HWND,
    path: &Path,
    screen: windows_sys::Win32::Foundation::POINT,
    extra: &[(usize, &str)],
) -> Choice {
    unsafe { show_impl(HWND(owner), path, POINT { x: screen.x, y: screen.y }, extra) }.unwrap_or(Choice::Failed)
}

unsafe fn show_impl(owner: HWND, path: &Path, at: POINT, extra: &[(usize, &str)]) -> windows::core::Result<Choice> {
    let wide = crate::win::wide_path(path);
    let mut abs = Pidl(std::ptr::null_mut());
    SHParseDisplayName(PCWSTR(wide.as_ptr()), None, &mut abs.0, 0, None)?;

    let mut child: *mut ITEMIDLIST = std::ptr::null_mut(); // points into `abs`; not freed separately
    let folder: IShellFolder = SHBindToParent(abs.0, Some(&mut child))?;
    let items = [child as *const ITEMIDLIST];
    let ctx: IContextMenu = folder.GetUIObjectOf(owner, &items, None)?;

    let menu = CreatePopupMenu()?;
    let shift =
        windows_sys::Win32::UI::Input::KeyboardAndMouse::GetKeyState(windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_SHIFT as i32) < 0;
    let flags = if shift { CMF_NORMAL | CMF_EXTENDEDVERBS } else { CMF_NORMAL };
    let result = (|| -> windows::core::Result<Choice> {
        ctx.QueryContextMenu(menu, 0, SHELL_FIRST, SHELL_LAST, flags).ok()?;
        if !extra.is_empty() {
            AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null())?;
            for (id, label) in extra {
                debug_assert!(*id >= EXTRA_FIRST);
                let w: Vec<u16> = label.encode_utf16().chain(std::iter::once(0)).collect();
                AppendMenuW(menu, MF_STRING, *id, PCWSTR(w.as_ptr()))?;
            }
        }
        // The controller owns the menu so the node never needs activation (as node.rs did before).
        let _ = SetForegroundWindow(owner);
        let cmd = TrackPopupMenuEx(menu, (TPM_RETURNCMD | TPM_RIGHTBUTTON).0, at.x, at.y, owner, None).0 as u32;
        let _ = PostMessageW(Some(owner), WM_NULL, Default::default(), Default::default());
        if cmd == 0 {
            return Ok(Choice::None);
        }
        if (SHELL_FIRST..=SHELL_LAST).contains(&cmd) {
            let info = CMINVOKECOMMANDINFO {
                cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
                hwnd: owner,
                lpVerb: PCSTR((cmd - SHELL_FIRST) as usize as *const u8), // MAKEINTRESOURCE(offset)
                nShow: SW_SHOWNORMAL.0,
                ..Default::default()
            };
            // The menu was shown and a verb chosen; a verb that fails reports its own error UI.
            let _ = ctx.InvokeCommand(&info);
            return Ok(Choice::Shell);
        }
        Ok(Choice::Extra(cmd as usize))
    })();
    let _ = DestroyMenu(menu);
    result
}
