//! Explicit one-shot alignment; selecting a grid never moves existing circles.
use super::{circle_win, geom, grid, ring, state, Which};
use crate::{app::app, win};
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;

pub fn now() {
    if circle_win::pressed_window().is_some() || ring::item_drag_pending() {
        return;
    }
    let n = app().board.try_borrow().map(|b| b.grid).unwrap_or(0);
    if !matches!(n, 2..=4 | 6 | 8) {
        return;
    }
    let windows = state()
        .and_then(|s| s.windows.try_borrow().ok().map(|w| w.iter().map(|w| (w.hwnd, w.which)).collect::<Vec<_>>()))
        .unwrap_or_default();
    // No borrow spans DPI/window calls. Reserve every current circle so even excess
    // circles left in place cannot be covered by an earlier alignment candidate.
    let mut placements: Vec<_> = windows
        .iter()
        .map(|(hwnd, _)| {
            let rect = win::window_rect(*hwnd);
            let d = geom::metrics(unsafe { GetDpiForWindow(*hwnd) }.max(96)).circle_d;
            ((rect.left, rect.top), d)
        })
        .collect();
    ring::close_now();
    crate::hint::hide();
    for index in 0..placements.len() {
        let (origin, diameter) = placements[index];
        let Some(work) = circle_win::work_area(origin, diameter) else { continue };
        let mut candidates = Vec::with_capacity(usize::from(n) * usize::from(n));
        for row in 0..n {
            for col in 0..n {
                let x = work.0
                    + ((f64::from(col) + 0.5) * f64::from(work.2 - work.0) / f64::from(n) - f64::from(diameter) / 2.0).round() as i32;
                let y = work.1
                    + ((f64::from(row) + 0.5) * f64::from(work.3 - work.1) / f64::from(n) - f64::from(diameter) / 2.0).round() as i32;
                let candidate = grid::position((x, y), diameter, work, 0);
                if !candidates.contains(&candidate) {
                    candidates.push(candidate);
                }
            }
        }
        candidates.sort_by_key(|p| {
            let dx = i64::from(p.0) - i64::from(origin.0);
            let dy = i64::from(p.1) - i64::from(origin.1);
            dx * dx + dy * dy
        });
        if let Some(candidate) = candidates.into_iter().find(|p| {
            placements.iter().enumerate().all(|(other, (at, d))| {
                if other == index {
                    return true;
                }
                let dx = 2 * (i64::from(p.0) - i64::from(at.0)) + i64::from(diameter - d);
                let dy = 2 * (i64::from(p.1) - i64::from(at.1)) + i64::from(diameter - d);
                let distance = i64::from(diameter + d + 4);
                dx * dx + dy * dy >= distance * distance
            })
        }) {
            placements[index].0 = candidate;
        }
    }
    {
        let Ok(mut board) = app().board.try_borrow_mut() else { return };
        for ((_, which), (position, _)) in windows.iter().zip(placements) {
            match which {
                Which::Recent => board.recent_pos = Some(position),
                Which::User(ci) => {
                    if let Some(circle) = board.circles.get_mut(*ci) {
                        circle.pos = position;
                    }
                }
            }
        }
    }
    app().save_config();
    super::refresh_all();
}
