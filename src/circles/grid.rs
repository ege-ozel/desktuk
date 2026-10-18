//! Cell-centre snapping in a monitor's usable work area, in physical screen pixels.

pub fn cell(pos: (i32, i32), diameter: i32, work: (i32, i32, i32, i32), divisions: u8) -> Option<(i32, i32)> {
    if !matches!(divisions, 2..=4) || diameter <= 0 || work.2 <= work.0 || work.3 <= work.1 {
        return None;
    }
    let axis = |p: i32, lo: i32, hi: i32| {
        (((p as f64 + diameter as f64 / 2.0 - lo as f64) * divisions as f64 / (hi as f64 - lo as f64)).floor() as i32)
            .clamp(0, divisions as i32 - 1)
    };
    Some((axis(pos.0, work.0, work.2), axis(pos.1, work.1, work.3)))
}

/// Snap the circle centre to the nearest cell centre, then keep the whole circle
/// inside `work`. Off/unknown divisions only clamp. Negative monitor origins work.
pub fn position(pos: (i32, i32), diameter: i32, work: (i32, i32, i32, i32), divisions: u8) -> (i32, i32) {
    let target = cell(pos, diameter, work, divisions);
    let axis = |p: i32, lo: i32, hi: i32, cell: Option<i32>| {
        if hi <= lo || diameter <= 0 {
            return p;
        }
        let end = hi.saturating_sub(diameter).max(lo);
        let Some(cell) = cell else {
            return p.clamp(lo, end);
        };
        let size = hi as f64 - lo as f64;
        let n = divisions as f64;
        let snapped = lo as f64 + (cell as f64 + 0.5) * size / n - diameter as f64 / 2.0;
        (snapped.round() as i32).clamp(lo, end)
    };
    (axis(pos.0, work.0, work.2, target.map(|c| c.0)), axis(pos.1, work.1, work.3, target.map(|c| c.1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_use_work_area_with_taskbars_on_every_edge() {
        for work in [(0, 0, 1800, 1080), (120, 0, 1920, 1080), (0, 80, 1920, 1080), (0, 0, 1920, 1000)] {
            for n in 2..=4 {
                for row in 0..n {
                    for col in 0..n {
                        let x = work.0 + ((col as f64 + 0.5) * (work.2 - work.0) as f64 / n as f64 - 44.0).round() as i32;
                        let y = work.1 + ((row as f64 + 0.5) * (work.3 - work.1) as f64 / n as f64 - 44.0).round() as i32;
                        assert_eq!(position((x + 9, y - 7), 88, work, n), (x, y));
                    }
                }
            }
        }
    }

    #[test]
    fn snapping_supports_negative_monitors_scaled_circles_and_clamping() {
        assert_eq!(position((-1800, -900), 132, (-1920, -1080, -120, 0), 3), (-1686, -966));
        assert_eq!(position((-30000, 30000), 176, (-1920, 0, 0, 1040), 4), (-1768, 822));
        assert_eq!(position((1990, -100), 88, (0, 0, 1800, 1080), 0), (1712, 0));
        assert_eq!(position((70, 70), 176, (0, 0, 200, 200), 4), (24, 24));
        assert_eq!(position((3, 4), 88, (0, 0, 40, 40), 2), (0, 0));
    }

    #[test]
    fn drag_landing_jumps_once_per_entered_cell_without_rebasing_the_pointer() {
        let work = (0, 0, 900, 600);
        let first_raw = (20, 20);
        let first = position(first_raw, 104, work, 3);
        assert_eq!(cell(first, 104, work, 3), Some((0, 0)));
        assert_eq!(first, (98, 48));

        let crossed_raw = (251, 20); // centre passes the vertical boundary into column 1
        let crossed = position(crossed_raw, 104, work, 3);
        assert_eq!(cell(crossed, 104, work, 3), Some((1, 0)));
        assert_eq!(crossed, (398, 48));

        let still_in_cell_raw = (280, 40);
        assert_eq!(position(still_in_cell_raw, 104, work, 3), crossed);
    }
}
