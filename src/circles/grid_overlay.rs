//! Drag-only guides: upload the grid once, then move a reusable cell highlight.
//! No retained bitmap, idle timer or full-monitor repaint on cell crossings.
use std::cell::Cell;
use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use super::{grid, surface::Surface};
use crate::win;

const CLASS: &str = "DeskTuk.GridOverlay";
const TARGET_CLASS: &str = "DeskTuk.GridTarget";
const LINE: u32 = 0xb42a78b4;
const FILL: u32 = 0x30102030;
type Work = (i32, i32, i32, i32);
#[derive(Clone, Copy, PartialEq, Eq)]
struct Layout {
    work: Work,
    divisions: u8,
    cell: (i32, i32),
    snap: (i32, i32),
    diameter: i32,
}
#[derive(Clone, Copy)]
struct Guide {
    frame: HWND,
    target: HWND,
    layout: Layout,
    line_uploads: usize,
    target_uploads: usize,
    cell_moves: usize,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Shape {
    w: i32,
    h: i32,
    cx: i32,
    cy: i32,
    stroke: i32,
}
thread_local! {
    static WINDOW: Cell<Option<Guide>> = const { Cell::new(None) };
}

pub fn register_class() {
    for class in [CLASS, TARGET_CLASS] {
        let name = win::wide(class);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(DefWindowProcW),
            hInstance: unsafe { GetModuleHandleW(std::ptr::null()) },
            lpszClassName: name.as_ptr(),
            ..unsafe { std::mem::zeroed() }
        };
        unsafe { RegisterClassW(&wc) };
    }
}

fn rectangle(pixels: &mut [u32], width: i32, height: i32, area: Work, color: u32) {
    let (left, top, right, bottom) = area;
    let (left, right) = (left.clamp(0, width) as usize, right.clamp(0, width) as usize);
    for y in top.clamp(0, height)..bottom.clamp(0, height) {
        let row = y as usize * width as usize;
        pixels[row + left..row + right].fill(color);
    }
}

fn paint_lines(layout: Layout) -> Option<Surface> {
    let (left, top, right, bottom) = layout.work;
    let (w, h, n) = (right - left, bottom - top, layout.divisions as i32);
    let mut surface = Surface::new(w, h)?;
    let pixels = surface.pixels();
    pixels.fill(0);
    let stroke = (layout.diameter / 44).max(1);
    for line in 0..=n {
        let x = line * w / n;
        let y = line * h / n;
        rectangle(pixels, w, h, (x - stroke, 0, x + stroke, h), LINE);
        rectangle(pixels, w, h, (0, y - stroke, w, y + stroke), LINE);
    }
    Some(surface)
}

fn target_geometry(layout: Layout) -> (POINT, Shape) {
    let (left, top, right, bottom) = layout.work;
    let (w, h, n) = (right - left, bottom - top, layout.divisions as i32);
    let x = left + layout.cell.0 * w / n;
    let y = top + layout.cell.1 * h / n;
    (
        POINT { x, y },
        Shape {
            w: left + (layout.cell.0 + 1) * w / n - x,
            h: top + (layout.cell.1 + 1) * h / n - y,
            cx: layout.snap.0 + layout.diameter / 2 - x,
            cy: layout.snap.1 + layout.diameter / 2 - y,
            stroke: (layout.diameter / 44).max(1),
        },
    )
}

fn paint_target(shape: Shape) -> Option<Surface> {
    let mut surface = Surface::new(shape.w, shape.h)?;
    let pixels = surface.pixels();
    pixels.fill(FILL);
    let Shape { w, h, cx, cy, stroke: s } = shape;
    rectangle(pixels, w, h, (cx - s, cy - 10 * s, cx + s, cy + 10 * s), LINE);
    rectangle(pixels, w, h, (cx - 10 * s, cy - s, cx + 10 * s, cy + s), LINE);
    Some(surface)
}

fn create(class: &str) -> HWND {
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            win::wide(class).as_ptr(),
            win::wide("Snap grid").as_ptr(),
            WS_POPUP,
            0,
            0,
            1,
            1,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    };
    if !hwnd.is_null() {
        crate::host::exclude_from_peek(hwnd);
    }
    hwnd
}

pub fn update(owner: HWND, pos: (i32, i32), diameter: i32, work: Work, divisions: u8) {
    let Some(cell) = grid::cell(pos, diameter, work, divisions) else {
        close();
        return;
    };
    let layout = Layout { work, divisions, cell, snap: grid::position(pos, diameter, work, divisions), diameter };
    let old = WINDOW.get();
    if old.is_some_and(|guide| guide.layout == layout) {
        return;
    }
    let (at, shape) = target_geometry(layout);
    let same_frame = old.is_some_and(|g| g.layout.work == work && g.layout.divisions == divisions && g.layout.diameter == diameter);
    let same_target = old.is_some_and(|g| target_geometry(g.layout).1 == shape);
    let mut guide = old.unwrap_or_else(|| Guide {
        frame: create(CLASS),
        target: create(TARGET_CLASS),
        layout,
        line_uploads: 0,
        target_uploads: 0,
        cell_moves: 0,
    });
    guide.layout = layout;
    WINDOW.set(Some(guide)); // cleanup owns both handles before any presentation
    if guide.frame.is_null() || guide.target.is_null() {
        close();
        return;
    }
    if !same_frame {
        let Some(surface) = paint_lines(layout) else {
            close();
            return;
        };
        if !surface.present(guide.frame, POINT { x: work.0, y: work.1 }) {
            close();
            return;
        }
        guide.line_uploads += 1;
    }
    if !same_target {
        let Some(surface) = paint_target(shape) else {
            close();
            return;
        };
        if !surface.present(guide.target, at) {
            close();
            return;
        }
        guide.target_uploads += 1;
    }
    // Normal cell changes only reposition the existing system-owned bitmap.
    unsafe {
        if !same_frame {
            SetWindowPos(guide.frame, owner, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW);
        }
        SetWindowPos(
            guide.target,
            if same_frame { std::ptr::null_mut() } else { guide.frame },
            at.x,
            at.y,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW | if same_frame { SWP_NOZORDER } else { 0 },
        );
    }
    guide.cell_moves += 1;
    WINDOW.set(Some(guide));
}

pub fn close() {
    if let Some(guide) = WINDOW.take() {
        crate::dlog!(
            "METRIC grid_drag line_uploads={} target_uploads={} cell_moves={}",
            guide.line_uploads,
            guide.target_uploads,
            guide.cell_moves
        );
        unsafe {
            if !guide.target.is_null() {
                DestroyWindow(guide.target);
            }
            if !guide.frame.is_null() {
                DestroyWindow(guide.frame);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(pos: (i32, i32), work: Work, n: u8) -> Layout {
        Layout {
            work,
            divisions: n,
            cell: grid::cell(pos, 88, work, n).expect("cell"),
            snap: grid::position(pos, 88, work, n),
            diameter: 88,
        }
    }

    #[test]
    fn guide_pixels_and_target_geometry_match_release_snapping() {
        for work in [(0, 0, 900, 600), (-1920, -1080, -119, -13)] {
            for n in 2..=4 {
                let l = layout((work.0 + 400, work.1 + 250), work, n);
                let mut lines = paint_lines(l).expect("grid frame");
                assert_eq!(lines.pixels()[0], LINE);
                let width = lines.w as usize;
                assert_eq!(lines.pixels()[50 * width + 50], 0);
                let (at, shape) = target_geometry(l);
                assert_eq!((at.x + shape.cx, at.y + shape.cy), (l.snap.0 + 44, l.snap.1 + 44));
                assert!(at.x >= work.0 && at.y >= work.1 && at.x + shape.w <= work.2 && at.y + shape.h <= work.3);
                let mut target = paint_target(shape).expect("target");
                assert_eq!(target.pixels()[(shape.cy * shape.w + shape.cx) as usize], LINE);
                assert!(target.pixels().contains(&FILL));
                for p in lines.pixels().iter().chain(target.pixels().iter()) {
                    assert!(((p >> 16) & 255) <= p >> 24 && ((p >> 8) & 255) <= p >> 24 && (p & 255) <= p >> 24);
                }
            }
        }
    }

    #[test]
    fn crossing_cells_reuses_native_bitmaps_without_taking_focus() {
        register_class();
        let foreground = unsafe { GetForegroundWindow() };
        let work = (-1200, -900, -300, -300);
        for n in 2..=4 {
            let mut handles = None;
            for pos in [(-1190, -850), (-820, -550), (-400, -400)] {
                update(HWND_TOP, pos, 88, work, n);
                let g = WINDOW.get().expect("guides");
                assert_eq!(g.layout.snap, grid::position(pos, 88, work, n));
                let rect = win::window_rect(g.frame);
                assert_eq!((rect.left, rect.top, rect.right, rect.bottom), work);
                let (at, shape) = target_geometry(g.layout);
                let rect = win::window_rect(g.target);
                assert_eq!((rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top), (at.x, at.y, shape.w, shape.h));
                for hwnd in [g.frame, g.target] {
                    let style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
                    assert_eq!(style & (WS_EX_TRANSPARENT | WS_EX_NOACTIVATE), WS_EX_TRANSPARENT | WS_EX_NOACTIVATE);
                }
                if let Some(previous) = handles {
                    assert_eq!((g.frame, g.target), previous);
                }
                handles = Some((g.frame, g.target));
                assert_eq!(g.line_uploads, 1, "cell changes must not rebuild the full grid");
                assert_eq!(unsafe { GetForegroundWindow() }, foreground);
            }
            let (frame, target) = handles.expect("handles");
            close();
            assert_eq!(unsafe { IsWindow(frame) | IsWindow(target) }, 0);
        }
        update(HWND_TOP, (-1100, -800), 88, work, 3);
        let first = WINDOW.get().expect("first monitor");
        let next_work = (-900, -700, -1, -101);
        update(HWND_TOP, (-500, -300), 88, next_work, 3);
        let next = WINDOW.get().expect("changed monitor layout");
        assert_eq!((first.frame, first.target), (next.frame, next.target));
        assert_eq!(next.line_uploads, 2, "one upload for each distinct monitor layout");
        let r = win::window_rect(next.frame);
        assert_eq!((r.left, r.top, r.right, r.bottom), next_work);
        update(HWND_TOP, (-1100, -800), 88, work, 0);
        assert!(WINDOW.get().is_none());
        assert_eq!(unsafe { IsWindow(next.frame) | IsWindow(next.target) }, 0);
    }

    #[test]
    #[ignore = "explicit native drag performance measurement"]
    fn measure_4k_cell_crossings() {
        register_class();
        let work = (-10000, -10000, -6160, -7840);
        let t = std::time::Instant::now();
        update(HWND_TOP, (work.0 + 100, work.1 + 100), 104, work, 3);
        let initial = t.elapsed();
        let t = std::time::Instant::now();
        for i in 0..500 {
            update(HWND_TOP, (work.0 + (i % 3) * 1280 + 100, work.1 + ((i / 3) % 3) * 720 + 100), 104, work, 3);
        }
        let g = WINDOW.get().expect("guides");
        assert_eq!(g.line_uploads, 1);
        assert_eq!(g.target_uploads, 1);
        eprintln!(
            "4K initial={initial:?}; 500 native cell updates={:?}; grid_uploads={} target_uploads={}",
            t.elapsed(),
            g.line_uploads,
            g.target_uploads
        );
        close();
    }
}
