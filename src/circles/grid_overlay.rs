//! Event-driven drag guide. Only the layered window's system copy survives painting.
use std::cell::Cell;
use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use super::{grid, surface::Surface};
use crate::win;

const CLASS: &str = "DeskTuk.GridOverlay";
type Work = (i32, i32, i32, i32);
#[derive(Clone, Copy, PartialEq, Eq)]
struct Layout {
    work: Work,
    divisions: u8,
    cell: (i32, i32),
    snap: (i32, i32),
    diameter: i32,
}
thread_local! {
    static WINDOW: Cell<Option<(HWND, Layout)>> = const { Cell::new(None) };
}

pub fn register_class() {
    let name = win::wide(CLASS);
    let wc = WNDCLASSW {
        lpfnWndProc: Some(DefWindowProcW),
        hInstance: unsafe { GetModuleHandleW(std::ptr::null()) },
        lpszClassName: name.as_ptr(),
        ..unsafe { std::mem::zeroed() }
    };
    unsafe { RegisterClassW(&wc) };
}

fn paint(layout: Layout) -> Option<Surface> {
    let (left, top, right, bottom) = layout.work;
    let (w, h, n) = (right - left, bottom - top, layout.divisions as i32);
    let mut surface = Surface::new(w, h)?;
    let pixels = surface.pixels();
    let (cx, cy) = (layout.snap.0 - left + layout.diameter / 2, layout.snap.1 - top + layout.diameter / 2);
    let stroke = (layout.diameter / 44).max(1);
    for y in 0..h {
        for x in 0..w {
            let col = ((x as i64 * n as i64) / w as i64) as i32;
            let row = ((y as i64 * n as i64) / h as i64) as i32;
            let boundary =
                (0..=n).any(|i| (x - i * w / n).abs() < stroke || (y - i * h / n).abs() < stroke) || x >= w - stroke || y >= h - stroke;
            let cross =
                ((x - cx).abs() < stroke && (y - cy).abs() < 10 * stroke) || ((y - cy).abs() < stroke && (x - cx).abs() < 10 * stroke);
            // Premultiplied blue: clear outside target; translucent fill and bright guides.
            pixels[(y * w + x) as usize] = if boundary || cross {
                0xb42a78b4
            } else if (col, row) == layout.cell {
                0x30102030
            } else {
                0
            };
        }
    }
    Some(surface)
}

pub fn update(owner: HWND, pos: (i32, i32), diameter: i32, work: Work, divisions: u8) {
    let Some(cell) = grid::cell(pos, diameter, work, divisions) else {
        close();
        return;
    };
    let layout = Layout { work, divisions, cell, snap: grid::position(pos, diameter, work, divisions), diameter };
    if WINDOW.get().is_some_and(|(_, old)| old == layout) {
        return;
    }
    let Some(surface) = paint(layout) else {
        close();
        return;
    };
    let hwnd = if let Some((hwnd, _)) = WINDOW.get() {
        hwnd
    } else {
        unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                win::wide(CLASS).as_ptr(),
                win::wide("Snap grid").as_ptr(),
                WS_POPUP,
                work.0,
                work.1,
                surface.w,
                surface.h,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            )
        }
    };
    if hwnd.is_null() {
        return;
    }
    WINDOW.set(Some((hwnd, layout)));
    if !surface.present(hwnd, POINT { x: work.0, y: work.1 }) {
        close();
        return;
    }
    // Immediately behind the dragged circle, including when it is topmost.
    unsafe { SetWindowPos(hwnd, owner, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW) };
}

pub fn close() {
    if let Some((hwnd, _)) = WINDOW.take() {
        unsafe { DestroyWindow(hwnd) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guide_pixels_mark_only_the_target_cell_and_its_landing_centre() {
        let work = (0, 0, 900, 600);
        let pos = (400, 250);
        let layout = Layout {
            work,
            divisions: 3,
            cell: grid::cell(pos, 88, work, 3).expect("cell"),
            snap: grid::position(pos, 88, work, 3),
            diameter: 88,
        };
        let mut surface = paint(layout).expect("guide bitmap");
        let pixels = surface.pixels();
        assert_eq!(pixels[250 * 900 + 400], 0x30102030, "target cell fill");
        assert_eq!(pixels[50 * 900 + 50], 0, "other cells remain clear");
        assert_eq!(pixels[(layout.snap.1 as usize + 44) * 900 + layout.snap.0 as usize + 44], 0xb42a78b4);
        assert!(pixels.iter().all(|p| ((p >> 16) & 255) <= p >> 24 && ((p >> 8) & 255) <= p >> 24 && (p & 255) <= p >> 24));
    }

    #[test]
    fn highlighted_cell_matches_snap_and_native_overlay_does_not_take_focus() {
        register_class();
        let foreground = unsafe { GetForegroundWindow() };
        let work = (-1200, -900, -300, -300);
        for n in 2..=4 {
            for pos in [(-1190, -850), (-820, -550), (-400, -400)] {
                update(HWND_TOP, pos, 88, work, n);
                let (hwnd, layout) = WINDOW.get().expect("native overlay");
                assert_eq!(layout.snap, grid::position(pos, 88, work, n));
                assert_eq!(grid::cell(layout.snap, 88, work, n), Some(layout.cell));
                let r = win::window_rect(hwnd);
                assert_eq!((r.left, r.top, r.right, r.bottom), work);
                let style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
                assert_eq!(style & (WS_EX_TRANSPARENT | WS_EX_NOACTIVATE), WS_EX_TRANSPARENT | WS_EX_NOACTIVATE);
                assert_eq!(unsafe { GetForegroundWindow() }, foreground);
                update(HWND_TOP, pos, 88, work, n);
                assert_eq!(WINDOW.get().map(|v| v.0), Some(hwnd));
                close();
                assert_eq!(unsafe { IsWindow(hwnd) }, 0);
            }
        }
        update(HWND_TOP, (-1100, -800), 88, work, 0);
        assert!(WINDOW.get().is_none());
        close();
    }
}
