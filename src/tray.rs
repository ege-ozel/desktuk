//! Notification-area access for both UIs. All callbacks run on the controller thread.
use std::cell::Cell;

use windows_sys::Win32::Foundation::{GetLastError, HWND, POINT};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION, NIN_SELECT, NOTIFYICONDATAW,
    NOTIFYICON_VERSION_4,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::{app, autostart, dlog, win};

// NIN_KEYSELECT = NIN_SELECT | NINF_KEY in the Windows SDK (not exported by windows-sys).
const NIN_KEYSELECT: u32 = NIN_SELECT | 1;

const ICON_ID: u32 = 1;
const SHOW: u32 = 1;
const STARTUP: u32 = 2;
const EXIT: u32 = 3;

thread_local! {
    static VERSION4: Cell<bool> = const { Cell::new(false) };
    static MENU_OPEN: Cell<bool> = const { Cell::new(false) };
}

fn identity(controller: HWND) -> NOTIFYICONDATAW {
    let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = controller;
    data.uID = ICON_ID;
    data
}

pub fn add(controller: HWND) -> bool {
    let mut data = identity(controller);
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.uCallbackMessage = app::WM_APP_TRAY;
    data.hIcon = unsafe { LoadIconW(GetModuleHandleW(std::ptr::null()), std::ptr::without_provenance(1)) };
    if data.hIcon.is_null() {
        dlog!("tray icon resource unavailable: {}", unsafe { GetLastError() });
        return false;
    }
    let tip = win::wide("DeskTuk");
    data.szTip[..tip.len()].copy_from_slice(&tip);
    // NIM_MODIFY also makes repeated TaskbarCreated notifications harmless.
    let added = unsafe { Shell_NotifyIconW(NIM_ADD, &data) != 0 || Shell_NotifyIconW(NIM_MODIFY, &data) != 0 };
    if !added {
        dlog!("tray add failed: {}", unsafe { GetLastError() });
        return false;
    }
    data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
    VERSION4.set(unsafe { Shell_NotifyIconW(NIM_SETVERSION, &data) } != 0);
    dlog!("tray icon ready version4={}", VERSION4.get());
    true
}

pub fn remove(controller: HWND) {
    unsafe { Shell_NotifyIconW(NIM_DELETE, &identity(controller)) };
    VERSION4.set(false);
}

#[derive(Debug, PartialEq, Eq)]
enum Action {
    Show,
    Menu,
    Ignore,
}

fn action(wparam: usize, lparam: isize, version4: bool) -> Action {
    let (event, id) = if version4 { ((lparam as u32) & 0xffff, ((lparam as u32) >> 16) & 0xffff) } else { (lparam as u32, wparam as u32) };
    if id != ICON_ID {
        return Action::Ignore;
    }
    match event {
        WM_CONTEXTMENU | WM_RBUTTONUP => Action::Menu,
        NIN_SELECT | NIN_KEYSELECT | WM_LBUTTONUP => Action::Show,
        _ => Action::Ignore,
    }
}

pub fn on_message(controller: HWND, wparam: usize, lparam: isize) {
    if app::app().quitting.get() {
        return;
    }
    match action(wparam, lparam, VERSION4.get()) {
        Action::Show => app::show(),
        Action::Menu => {
            let mut pos = POINT { x: 0, y: 0 };
            if VERSION4.get() {
                pos.x = wparam as u16 as i16 as i32;
                pos.y = (wparam >> 16) as u16 as i16 as i32;
            } else {
                unsafe { GetCursorPos(&mut pos) };
            }
            menu(controller, pos);
        }
        Action::Ignore => {}
    }
}

struct MenuGuard;
impl Drop for MenuGuard {
    fn drop(&mut self) {
        MENU_OPEN.set(false);
    }
}

fn menu(controller: HWND, pos: POINT) {
    if MENU_OPEN.replace(true) {
        return;
    }
    let _guard = MenuGuard;
    let startup = autostart::is_enabled();
    let flags = match startup {
        Ok(true) => MF_STRING | MF_CHECKED,
        Ok(false) => MF_STRING,
        Err(ref e) => {
            dlog!("startup status unavailable: {e}");
            MF_STRING | MF_GRAYED
        }
    };
    let selected = unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        let built = AppendMenuW(menu, MF_STRING, SHOW as usize, win::wide("&Desktuk'u göster").as_ptr()) != 0
            && AppendMenuW(menu, flags, STARTUP as usize, win::wide("&Windows ile başlat").as_ptr()) != 0
            && AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null()) != 0
            && AppendMenuW(menu, MF_STRING, EXIT as usize, win::wide("&Çıkış").as_ptr()) != 0;
        let selected = if built {
            SetForegroundWindow(controller);
            TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, pos.x, pos.y, 0, controller, std::ptr::null()) as u32
        } else {
            0
        };
        PostMessageW(controller, WM_NULL, 0, 0);
        DestroyMenu(menu);
        selected
    };
    if app::app().quitting.get() {
        return;
    }
    match selected {
        SHOW => app::show(),
        STARTUP => {
            // Re-read after TrackPopupMenu: the registry may have changed in the meantime.
            let result = autostart::is_enabled().and_then(|enabled| autostart::set_enabled(!enabled));
            if let Err(e) = result {
                dlog!("startup change failed: {e}");
                unsafe {
                    MessageBoxW(
                        controller,
                        win::wide(format!("Windows başlangıç ayarı değiştirilemedi.\n\n{e}")).as_ptr(),
                        win::wide("DeskTuk").as_ptr(),
                        MB_OK | MB_ICONERROR,
                    );
                }
            }
        }
        EXIT => app::quit(),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version4_callbacks_decode_event_and_icon_id_not_screen_coordinates() {
        for event in [NIN_SELECT, NIN_KEYSELECT, WM_LBUTTONUP] {
            assert_eq!(action(0xfffe_fffd, ((ICON_ID << 16) | event) as isize, true), Action::Show);
        }
        assert_eq!(action(0, ((ICON_ID << 16) | WM_CONTEXTMENU) as isize, true), Action::Menu);
        assert_eq!(action(0, ((2 << 16) | NIN_SELECT) as isize, true), Action::Ignore);
        assert_eq!(action(0, ((ICON_ID << 16) | WM_MOUSEMOVE) as isize, true), Action::Ignore);
    }
    #[test]
    fn legacy_callbacks_remain_usable_when_setversion_fails() {
        assert_eq!(action(ICON_ID as usize, WM_RBUTTONUP as isize, false), Action::Menu);
        assert_eq!(action(ICON_ID as usize, WM_LBUTTONUP as isize, false), Action::Show);
        assert_eq!(action(2, WM_RBUTTONUP as isize, false), Action::Ignore);
    }
}
