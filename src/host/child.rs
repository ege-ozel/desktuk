//! EXPERIMENTAL host: the node is a `WS_CHILD` of the window that hosts Explorer's
//! desktop icon view (`Progman`, or a `WorkerW` on older builds), placed above the
//! icon view in the sibling z-order.
//!
//! Being part of the desktop window tree, the node is unaffected by "Show desktop"
//! and sits below every application window. However:
//! * It depends on undocumented Explorer window structure, which changed in
//!   Windows 11 24H2 and may change again.
//! * Cross-process parent/child attaches the input queues of DeskTuk's UI thread and
//!   Explorer's desktop thread: if either hangs, both freeze.
//! * When Explorer exits the node window is destroyed with its parent and has to be
//!   re-created once the new desktop exists.

use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetParent, IsWindow, SetWindowPos, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_CHILD, WS_CLIPCHILDREN,
    WS_CLIPSIBLINGS, WS_EX_NOPARENTNOTIFY,
};

use super::{CreateSpec, DesktopHost, HostKind};
use crate::shell;

pub struct ChildHost;

impl DesktopHost for ChildHost {
    fn kind(&self) -> HostKind {
        HostKind::Child
    }

    fn prepare(&self) -> Result<CreateSpec, String> {
        let d = shell::desktop_windows().ok_or("desktop icon view (SHELLDLL_DefView) not found")?;
        Ok(CreateSpec {
            // NOPARENTNOTIFY: don't send WM_PARENTNOTIFY into Explorer for our clicks/lifetime.
            ex_style: WS_EX_NOPARENTNOTIFY,
            style: WS_CHILD | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
            parent: d.defview_parent,
        })
    }

    fn attached(&self, node: HWND) {
        // Above SHELLDLL_DefView among the parent's children.
        unsafe { SetWindowPos(node, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
    }

    fn place(&self, node: HWND, r: &RECT) {
        let mut pt = POINT { x: r.left, y: r.top };
        unsafe {
            ScreenToClient(GetParent(node), &mut pt);
            SetWindowPos(node, std::ptr::null_mut(), pt.x, pt.y, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE);
        }
    }

    fn is_healthy(&self, node: HWND) -> bool {
        unsafe { IsWindow(GetParent(node)) != 0 }
    }

    fn shell_restarted(&self, node: HWND) -> bool {
        match shell::desktop_windows() {
            Some(d) => node.is_null() || unsafe { GetParent(node) } != d.defview_parent,
            None => true,
        }
    }
}
