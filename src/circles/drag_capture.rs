//! Temporary foreground ownership for a drag from a normally non-activating window.
//! SetCapture only captures outside a window when it is foreground. Restore focus and
//! the non-activating style at release, without retaining borrows across window calls.

use std::cell::Cell;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetCapture, GetKeyState, ReleaseCapture, SetCapture, VK_LBUTTON};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

#[derive(Clone, Copy)]
struct Capture {
    hwnd: HWND,
    previous: HWND,
    no_activate: bool,
    topmost: bool,
}

thread_local! {
    static ACTIVE: Cell<Option<Capture>> = const { Cell::new(None) };
}

pub fn is_active() -> bool {
    ACTIVE.get().is_some()
}

pub fn is_active_for(hwnd: HWND) -> bool {
    ACTIVE.get().is_some_and(|capture| capture.hwnd == hwnd)
}

pub fn begin(hwnd: HWND) -> bool {
    if let Some(old) = ACTIVE.take() {
        finish(old);
    }
    // Capture/focus ownership belongs to a real button-down input sequence. A
    // routed WM_LBUTTONDOWN alone does not update the thread's input key state;
    // taking capture in that case can inject unrelated current-cursor movement.
    if unsafe { GetKeyState(VK_LBUTTON as i32) } >= 0 {
        crate::dlog!("drag capture skipped: no queued left-button-down state");
        return false;
    }
    let style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
    let capture = Capture {
        hwnd,
        previous: unsafe { GetForegroundWindow() },
        no_activate: style & WS_EX_NOACTIVATE != 0,
        topmost: style & WS_EX_TOPMOST != 0,
    };
    ACTIVE.set(Some(capture));
    unsafe {
        SetWindowLongW(hwnd, GWL_EXSTYLE, (style & !WS_EX_NOACTIVATE) as i32);
        let accepted = SetForegroundWindow(hwnd);
        crate::dlog!("drag foreground request accepted={accepted} actual={:?} target={hwnd:?}", GetForegroundWindow());
        if GetForegroundWindow() == hwnd {
            SetCapture(hwnd);
            if GetCapture() == hwnd {
                return true;
            }
        }
    }
    end(hwnd);
    false
}

pub fn end(hwnd: HWND) {
    if ACTIVE.get().map(|c| c.hwnd == hwnd).unwrap_or(false) {
        if let Some(capture) = ACTIVE.take() {
            finish(capture);
        }
    }
}

fn finish(capture: Capture) {
    unsafe {
        if GetCapture() == capture.hwnd {
            ReleaseCapture();
        }
        // Alt+Tab/capture loss may already have selected a different app; leave that choice.
        if GetForegroundWindow() == capture.hwnd && capture.previous != capture.hwnd && IsWindow(capture.previous) != 0 {
            SetForegroundWindow(capture.previous);
        }
        if IsWindow(capture.hwnd) != 0 {
            if capture.no_activate {
                let style = GetWindowLongW(capture.hwnd, GWL_EXSTYLE) as u32;
                SetWindowLongW(capture.hwnd, GWL_EXSTYLE, (style | WS_EX_NOACTIVATE) as i32);
            }
            // Restore the host's current desktop/app ordering, including a normal-band
            // tray reveal. A non-topmost window is not necessarily a bottom window.
            let host = super::host_of(capture.hwnd).map(|h| (capture.hwnd, h)).or_else(|| {
                let owner = GetWindow(capture.hwnd, GW_OWNER);
                super::host_of(owner).map(|h| (owner, h))
            });
            if let Some((node, host)) = host {
                host.capture_finished(node, GetForegroundWindow(), capture.topmost);
            }
        }
    }
}
