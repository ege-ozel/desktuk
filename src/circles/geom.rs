//! Pure geometry for circles and rings: slot positions, hit testing, paging, the
//! anti-aliased disc mask and the open/close animation curve. No Win32 here.

use crate::win;

/// Slots shown per ring page: 8 on the inner ring + 12 on the outer ring.
pub const PER_PAGE: usize = 20;
pub const INNER_SLOTS: usize = 8;
pub const OUTER_SLOTS: usize = PER_PAGE - INNER_SLOTS;

/// Opening begins immediately and eases into the full ring.
pub const OPEN_MS: f64 = 120.0;
/// Ring closing animation length.
pub const CLOSE_MS: f64 = 110.0;

/// Pixel sizes for one DPI. All values are 96-DPI design values scaled by `win::scale`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Metrics {
    /// Diameter of a closed circle window.
    pub circle_d: i32,
    /// Radius of an open ring; the ring window is `2 * ring_r` square.
    pub ring_r: i32,
    /// Side of one (square) slot.
    pub slot: i32,
    /// Icon size inside a slot.
    pub icon: i32,
    /// Distance from the ring centre to inner / outer slot centres.
    pub inner_r: i32,
    pub outer_r: i32,
    pub inner_slots: usize,
    pub outer_slots: usize,
}

pub fn metrics(dpi: u32) -> Metrics {
    let s = |v| win::scale(v, dpi);
    Metrics {
        circle_d: s(104),
        ring_r: s(186),
        slot: s(54),
        icon: s(32),
        inner_r: s(90),
        outer_r: s(150),
        inner_slots: INNER_SLOTS,
        outer_slots: OUTER_SLOTS,
    }
}

impl Metrics {
    pub fn per_page(&self) -> usize {
        self.inner_slots + self.outer_slots
    }
}

/// Keep native-sized icons and two lines of desktop-font text. Reduce page capacity
/// when a 20-slot ring would exceed the monitor; do not shrink the icons to fit.
pub fn desktop_metrics(dpi: u32, icon: i32, max_diameter: i32) -> Metrics {
    let mut best = metrics(dpi);
    best.icon = icon;
    best.slot = icon + win::scale(36, dpi);
    let pad = win::scale(8, dpi) as f64;
    let half_diagonal = best.slot as f64 / 2.0 * std::f64::consts::SQRT_2;
    let separation = 2.0 * half_diagonal + pad;
    let min_r = best.circle_d as f64 / 2.0 + half_diagonal + pad;
    let radius_for = |n: usize| {
        if n < 2 {
            min_r
        } else {
            min_r.max(separation / (2.0 * (std::f64::consts::PI / n as f64).sin()))
        }
    };
    // The one-slot fallback still keeps its icon clear of the centre.
    best.inner_slots = 0;
    best.outer_slots = 1;
    best.outer_r = min_r.ceil() as i32;
    best.ring_r = (min_r + half_diagonal + pad).ceil() as i32;
    for inner in 0..=INNER_SLOTS {
        for outer in 1..=OUTER_SLOTS {
            let ir = radius_for(inner).ceil();
            let or = radius_for(outer).max(if inner == 0 { min_r } else { ir + separation }).ceil();
            let radius = (or + half_diagonal + pad).ceil() as i32;
            let count = inner + outer;
            if 2 * radius <= max_diameter && (count > best.per_page() || (count == best.per_page() && radius < best.ring_r)) {
                best.inner_slots = inner;
                best.outer_slots = outer;
                best.inner_r = ir as i32;
                best.outer_r = or as i32;
                best.ring_r = radius;
            }
        }
    }
    best
}

/// Centre of slot `k` relative to the ring centre. Slot 0 is at 12 o'clock on the inner
/// ring; the outer ring starts half a step clockwise of 12 o'clock.
pub fn slot_center(m: &Metrics, k: usize) -> (i32, i32) {
    let (r, deg) = if k < m.inner_slots {
        (m.inner_r, -90.0 + k as f64 * (360.0 / m.inner_slots as f64))
    } else {
        let j = (k - m.inner_slots) % m.outer_slots;
        (m.outer_r, -90.0 + j as f64 * (360.0 / m.outer_slots as f64))
    };
    let a = deg.to_radians();
    ((r as f64 * a.cos()).round() as i32, (r as f64 * a.sin()).round() as i32)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RingHit {
    /// Inside the centre disc (the circle's own title area).
    Center,
    Slot(usize),
    /// Inside the ring but not on a slot.
    Ring,
    Outside,
}

/// What is under `(dx, dy)` (relative to the ring centre) on a ring showing `n` slots.
pub fn ring_hit(m: &Metrics, n: usize, dx: i32, dy: i32) -> RingHit {
    let d2 = dx as i64 * dx as i64 + dy as i64 * dy as i64;
    if d2 > m.ring_r as i64 * m.ring_r as i64 {
        return RingHit::Outside;
    }
    let half = m.slot / 2;
    for k in 0..n.min(m.per_page()) {
        let (cx, cy) = slot_center(m, k);
        if (dx - cx).abs() <= half && (dy - cy).abs() <= half {
            return RingHit::Slot(k);
        }
    }
    let c = m.circle_d / 2;
    if d2 <= c as i64 * c as i64 {
        RingHit::Center
    } else {
        RingHit::Ring
    }
}

/// Retain the last valid slot across a small boundary margin; fixed geometry never
/// chases the pointer. Non-slot regions remain invalid for release.
pub fn drag_slot(m: &Metrics, n: usize, dx: i32, dy: i32, previous: Option<usize>, margin: i32) -> Option<usize> {
    if let Some(k) = previous.filter(|&k| k < n) {
        let (cx, cy) = slot_center(m, k);
        if (dx - cx).abs() <= m.slot / 2 + margin && (dy - cy).abs() <= m.slot / 2 + margin {
            return Some(k);
        }
    }
    match ring_hit(m, n, dx, dy) {
        RingHit::Slot(k) => Some(k),
        _ => None,
    }
}

/// Normalises a page start: a multiple of `capacity`, never past the last page.
pub fn clamp_start(total: usize, start: usize, capacity: usize) -> usize {
    if total == 0 {
        return 0;
    }
    let last = (total - 1) / capacity * capacity;
    (start / capacity * capacity).min(last)
}

/// Indices shown on the page beginning at `start`.
pub fn page_range(total: usize, start: usize, capacity: usize) -> std::ops::Range<usize> {
    let s = clamp_start(total, start, capacity);
    s..(s + capacity).min(total)
}

/// New page start after `pages` wheel steps (positive = next page, negative = previous).
pub fn scroll(total: usize, start: usize, pages: i32, capacity: usize) -> usize {
    let s = clamp_start(total, start, capacity) as i64 + pages as i64 * capacity as i64;
    clamp_start(total, s.max(0) as usize, capacity)
}

/// Converts accumulated wheel delta into whole page steps. Returns `(pages, remainder)`.
/// `WM_MOUSEWHEEL` reports +120 per notch *away* from the user (= previous page).
pub fn wheel_pages(acc: i32) -> (i32, i32) {
    let notches = acc / 120;
    (-notches, acc - notches * 120)
}

/// "21-40 / 300"; empty when everything fits on one page.
pub fn page_label(total: usize, start: usize, capacity: usize) -> String {
    if total <= capacity {
        return String::new();
    }
    let r = page_range(total, start, capacity);
    format!("{}-{} / {}", r.start + 1, r.end, total)
}

/// Coverage (0..=255) of the pixel whose centre is `(dx, dy)` from the disc centre.
pub fn coverage(dx: f32, dy: f32, r: f32) -> u8 {
    let d = (dx * dx + dy * dy).sqrt();
    let c = (r - d + 0.5).clamp(0.0, 1.0);
    (c * 255.0).round() as u8
}

/// Turns an opaque BGRA image (`0x00RRGGBB` or any alpha) into a premultiplied
/// image of a disc of radius `r` centred in the `w`x`h` buffer: alpha = coverage,
/// colour scaled by alpha. Pixels outside the disc become fully transparent.
pub fn mask_disc(px: &mut [u32], w: usize, h: usize, r: f32) {
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    for y in 0..h {
        let dy = y as f32 + 0.5 - cy;
        for x in 0..w {
            let a = coverage(x as f32 + 0.5 - cx, dy, r) as u32;
            let i = y * w + x;
            px[i] = if a == 0 { 0 } else { premultiply(px[i] | 0xff00_0000, a) };
        }
    }
}

/// Scales all four channels of a premultiplied pixel by `a / 255`.
fn premultiply(p: u32, a: u32) -> u32 {
    let ch = |shift: u32| (((p >> shift) & 0xff) * a + 127) / 255;
    (ch(24) << 24) | (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

/// Writes `full` (a premultiplied image, see `mask_disc`) into `out`, cut to a disc of
/// radius `r`. Used for the reveal animation; `full` itself is never modified.
pub fn reveal(full: &[u32], out: &mut [u32], w: usize, h: usize, r: f32) {
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    for y in 0..h {
        let dy = y as f32 + 0.5 - cy;
        for x in 0..w {
            let i = y * w + x;
            let a = coverage(x as f32 + 0.5 - cx, dy, r) as u32;
            out[i] = match a {
                0 => 0,
                255 => full[i],
                _ => premultiply(full[i], a),
            };
        }
    }
}

pub fn ease_out_cubic(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Radius of a ring that animates from `from` to `to` over `dur_ms`, `elapsed_ms` in.
/// Returns `(radius, finished)`.
pub fn anim_radius(from: f32, to: f32, elapsed_ms: f64, dur_ms: f64) -> (f32, bool) {
    if dur_ms <= 0.0 || elapsed_ms >= dur_ms {
        return (to, true);
    }
    let k = ease_out_cubic(elapsed_ms / dur_ms) as f32;
    (from + (to - from) * k, false)
}

#[cfg(test)]
mod tests {
    #[test]
    fn insertion_target_stays_fixed_near_slot_boundary() {
        for dpi in [96, 120, 144, 192] {
            let m = metrics(dpi);
            let (x, y) = slot_center(&m, 0);
            let margin = crate::win::scale(6, dpi);
            assert_eq!(drag_slot(&m, 20, x + m.slot / 2 + margin, y, Some(0), margin), Some(0));
            assert_ne!(drag_slot(&m, 20, x + m.slot / 2 + margin + 1, y, Some(0), margin), Some(0));
            assert_eq!(drag_slot(&m, 20, 0, 0, Some(0), margin), None);
            assert_eq!(drag_slot(&m, 20, 0, 0, None, margin), None);
        }
    }

    use super::*;

    #[test]
    fn slots_fit_the_ring_and_never_overlap() {
        for dpi in [96, 120, 144, 192] {
            let m = metrics(dpi);
            let half = m.slot / 2;
            let centers: Vec<(i32, i32)> = (0..PER_PAGE).map(|k| slot_center(&m, k)).collect();
            for (k, &(x, y)) in centers.iter().enumerate() {
                // Every slot corner stays inside the ring window square.
                assert!(x.abs() + half <= m.ring_r && y.abs() + half <= m.ring_r, "dpi {dpi} slot {k}");
                // ...and outside the centre disc.
                let d = ((x * x + y * y) as f64).sqrt();
                assert!(d - half as f64 * std::f64::consts::SQRT_2 >= (m.circle_d / 2) as f64 - 1.0, "dpi {dpi} slot {k} hits centre");
                for (j, &(x2, y2)) in centers.iter().enumerate().skip(k + 1) {
                    assert!((x - x2).abs() >= m.slot || (y - y2).abs() >= m.slot, "dpi {dpi} slots {k} and {j} overlap");
                }
            }
        }
    }

    #[test]
    fn slot_zero_is_at_twelve_o_clock() {
        let m = metrics(96);
        assert_eq!(slot_center(&m, 0), (0, -m.inner_r));
    }

    #[test]
    fn ring_hit_finds_centre_slots_ring_and_outside() {
        let m = metrics(96);
        assert_eq!(ring_hit(&m, 20, 0, 0), RingHit::Center);
        let (x, y) = slot_center(&m, 9);
        assert_eq!(ring_hit(&m, 20, x, y), RingHit::Slot(9));
        assert_eq!(ring_hit(&m, 5, x, y), RingHit::Ring, "slot 9 is empty when only 5 are shown");
        assert_eq!(ring_hit(&m, 20, m.ring_r + 1, 0), RingHit::Outside);
        assert_eq!(ring_hit(&m, 20, m.ring_r - 1, m.ring_r - 1), RingHit::Outside, "square corner is outside the disc");
    }

    #[test]
    fn paging_walks_300_items_in_steps_of_20() {
        assert_eq!(page_range(300, 0, PER_PAGE), 0..20);
        assert_eq!(scroll(300, 0, 1, PER_PAGE), 20);
        assert_eq!(page_range(300, 20, PER_PAGE), 20..40);
        assert_eq!(scroll(300, 20, -1, PER_PAGE), 0);
        assert_eq!(scroll(300, 0, -3, PER_PAGE), 0, "never before the first page");
        assert_eq!(scroll(300, 280, 5, PER_PAGE), 280, "never past the last page");
        assert_eq!(page_range(301, 300, PER_PAGE), 300..301);
        assert_eq!(page_label(300, 20, PER_PAGE), "21-40 / 300");
        assert_eq!(page_label(20, 0, PER_PAGE), "");
        assert_eq!(page_range(0, 40, PER_PAGE), 0..0);
        assert_eq!(clamp_start(300, 37, PER_PAGE), 20);
        assert_eq!(clamp_start(25, 400, PER_PAGE), 20, "item count shrank while scrolled: clamped");
    }

    #[test]
    fn wheel_delta_becomes_page_steps() {
        assert_eq!(wheel_pages(-120), (1, 0), "wheel towards the user = next page");
        assert_eq!(wheel_pages(240), (-2, 0));
        assert_eq!(wheel_pages(-50), (0, -50), "partial touchpad deltas accumulate");
        assert_eq!(wheel_pages(-170), (1, -50));
    }

    #[test]
    fn disc_mask_is_opaque_inside_clear_outside_and_premultiplied() {
        let (w, h) = (20usize, 20usize);
        let mut px = vec![0x00ff_8000u32; w * h];
        mask_disc(&mut px, w, h, 8.0);
        assert_eq!(px[10 * w + 10], 0xffff_8000, "centre opaque, colour unchanged");
        assert_eq!(px[0], 0, "corner transparent");
        let edge = px[10 * w + 2]; // pixel centre 7.5 from the disc centre -> partly covered
        let a = edge >> 24;
        assert!(a > 0 && a < 255, "anti-aliased edge, alpha {a}");
        assert!((edge >> 16 & 0xff) <= a, "premultiplied: colour <= alpha");
    }

    #[test]
    fn reveal_cuts_a_smaller_disc_without_touching_the_source() {
        let (w, h) = (20usize, 20usize);
        let mut full = vec![0x0012_3456u32; w * h];
        mask_disc(&mut full, w, h, 10.0);
        let copy = full.clone();
        let mut out = vec![0u32; w * h];
        reveal(&full, &mut out, w, h, 4.0);
        assert_eq!(out[10 * w + 10], full[10 * w + 10]);
        assert_eq!(out[10 * w + 2], 0, "outside the small disc");
        assert_eq!(full, copy);
    }

    #[test]
    fn animation_eases_and_finishes() {
        assert_eq!(anim_radius(10.0, 100.0, 0.0, CLOSE_MS), (10.0, false));
        let (mid, done) = anim_radius(10.0, 100.0, CLOSE_MS / 2.0, CLOSE_MS);
        assert!(!done && mid > 55.0, "ease-out is past half way at half time: {mid}");
        assert_eq!(anim_radius(10.0, 100.0, CLOSE_MS + 1.0, CLOSE_MS), (100.0, true));
        assert_eq!(anim_radius(100.0, 10.0, CLOSE_MS, CLOSE_MS), (10.0, true));
    }
}
