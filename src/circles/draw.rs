//! GDI drawing of closed circles and open rings into a `Surface`.
//!
//! Everything is drawn on an opaque background; the caller then applies
//! `geom::mask_disc`, which sets the alpha channel (GDI text leaves alpha bytes at 0,
//! so the mask must always run last).

use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Gdi::{
    CreateSolidBrush, DeleteObject, DrawTextW, FillRect, SelectObject, SetBkMode, SetTextColor, DT_CENTER, DT_EDITCONTROL, DT_END_ELLIPSIS,
    DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK, HDC, HFONT, TRANSPARENT,
};

use windows_sys::Win32::UI::WindowsAndMessaging::{DrawIconEx, LoadIconW, DI_NORMAL, HICON, IDI_APPLICATION};

use super::geom::{self, Metrics};
use super::surface::Surface;
use crate::win;

pub const CIRCLE_BG: u32 = crate::theme::BG;
pub const RECENT_BG: u32 = CIRCLE_BG;
pub const CENTER_BG: u32 = crate::theme::SURFACE;
pub const ACCENT: u32 = crate::theme::ACCENT;
pub const HOVER: u32 = crate::theme::RAISED;
pub const GROUP: u32 = crate::theme::BORDER;
pub const TEXT: u32 = crate::theme::TEXT;
pub const SUBTEXT: u32 = crate::theme::MUTED;
const COUNT_PERCENT: i32 = 100;

/// A small translucent disc carrying the loaded icon (or a shared native fallback).
pub fn drag_icon(icon: HICON, dpi: u32) -> Option<Surface> {
    let px = win::scale(40, dpi);
    let pad = win::scale(8, dpi);
    let d = px + 2 * pad;
    let mut s = Surface::new(d, d)?;
    fill_rect(s.dc(), &win::rect(0, 0, d, d), ACCENT);
    let rim = win::scale(2, dpi);
    fill_disc(&mut s, &win::rect(rim, rim, d - rim, d - rim), CIRCLE_BG);
    let icon = if icon.is_null() { unsafe { LoadIconW(std::ptr::null_mut(), IDI_APPLICATION) } } else { icon };
    unsafe { DrawIconEx(s.dc(), pad, pad, icon, px, px, 0, std::ptr::null_mut(), DI_NORMAL) };
    geom::mask_disc(s.pixels(), d as usize, d as usize, d as f32 / 2.0);
    // Fade all premultiplied channels together, including alpha.
    for p in s.pixels() {
        let fade = |shift: u32| (((*p >> shift) & 255) * 210 / 255) << shift;
        *p = fade(24) | fade(16) | fade(8) | fade(0);
    }
    Some(s)
}

/// The icon-title font scaled to `pct` percent, deleted on drop.
struct Font(HFONT);

impl Font {
    fn new(dpi: u32, bold: bool, pct: i32) -> Font {
        Font(crate::theme::font(dpi, bold, pct))
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        unsafe { DeleteObject(self.0) };
    }
}

fn fill_rect(dc: HDC, r: &RECT, color: u32) {
    unsafe {
        let b = CreateSolidBrush(color);
        FillRect(dc, r, b);
        DeleteObject(b);
    }
}

fn fill_disc(s: &mut Surface, r: &RECT, color: u32) {
    let (w, h) = (s.w, s.h);
    let cx = (r.left + r.right) as f32 / 2.0;
    let cy = (r.top + r.bottom) as f32 / 2.0;
    let radius = (r.right - r.left) as f32 / 2.0;
    // COLORREF is 0x00BBGGRR; DIB pixels are 0xAARRGGBB.
    let rgb = ((color & 0xff) << 16) | (color & 0xff00) | ((color >> 16) & 0xff);
    // Flush pending GDI text/fills before accessing the shared DIB.
    let pixels = s.pixels();
    for y in r.top.max(0)..r.bottom.min(h) {
        for x in r.left.max(0)..r.right.min(w) {
            let a = geom::coverage(x as f32 + 0.5 - cx, y as f32 + 0.5 - cy, radius) as u32;
            let index = (y * w + x) as usize;
            let previous = pixels[index];
            let channel = |shift: u32| ((((rgb >> shift) & 0xff) * a + ((previous >> shift) & 0xff) * (255 - a) + 127) / 255) << shift;
            pixels[index] = (previous & 0xff00_0000) | channel(16) | channel(8) | channel(0);
        }
    }
}

fn text(dc: HDC, s: &str, r: RECT, color: u32, font: &Font, flags: u32) {
    let mut w = win::wide(s);
    let mut r = r;
    unsafe {
        let old = SelectObject(dc, font.0);
        SetTextColor(dc, color);
        DrawTextW(dc, w.as_mut_ptr(), -1, &mut r, flags | DT_NOPREFIX);
        SelectObject(dc, old);
    }
}

/// Square around `(cx, cy)` with side `side`.
fn square(cx: i32, cy: i32, side: i32) -> RECT {
    win::rect(cx - side / 2, cy - side / 2, cx - side / 2 + side, cy - side / 2 + side)
}

/// A closed circle: accent rim, name (up to two lines) and the item count.
pub fn circle(s: &mut Surface, dpi: u32, name: &str, count: usize, locked: bool) {
    let dc = s.dc();
    let d = s.w;
    let sc = |v| win::scale(v, dpi);
    fill_rect(dc, &win::rect(0, 0, d, d), ACCENT);
    let rim = sc(3).max(2);
    if locked {
        // Static diagonal teal/accent rim: drawn only when the circle changes.
        let pixels = s.pixels();
        for y in 0..d {
            for x in 0..d {
                let t = (x + y) as u32 * 255 / (2 * (d - 1)) as u32;
                let a = crate::theme::MINT;
                let b = crate::theme::ACCENT;
                let channel = |shift: u32| (((a >> shift) & 255) * (255 - t) + ((b >> shift) & 255) * t + 127) / 255;
                pixels[(y * d + x) as usize] = 0xff00_0000 | channel(0) << 16 | channel(8) << 8 | channel(16);
            }
        }
    }
    fill_disc(s, &win::rect(rim, rim, d - rim, d - rim), CIRCLE_BG);
    if locked {
        let side = sc(22);
        if let Some(lock) = crate::dock::lock_surface_tint(side, CIRCLE_BG, true, SUBTEXT) {
            unsafe {
                windows_sys::Win32::Graphics::Gdi::BitBlt(
                    dc,
                    (d - side) / 2,
                    d * 56 / 100 - side / 2,
                    side,
                    side,
                    lock.dc(),
                    0,
                    0,
                    windows_sys::Win32::Graphics::Gdi::SRCCOPY,
                )
            };
        }
    }
    unsafe { SetBkMode(dc, TRANSPARENT as i32) };
    let bold = Font::new(dpi, true, 100);
    let small = Font::new(dpi, false, COUNT_PERCENT);
    let pad = sc(10);
    text(
        dc,
        name,
        win::rect(pad, d * 20 / 100, d - pad, d * 46 / 100),
        TEXT,
        &bold,
        DT_CENTER | DT_WORDBREAK | DT_EDITCONTROL | DT_END_ELLIPSIS,
    );
    text(
        dc,
        &count.to_string(),
        win::rect(pad, d * 65 / 100, d - pad, d * 86 / 100),
        SUBTEXT,
        &small,
        DT_CENTER | DT_SINGLELINE | DT_VCENTER,
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotLook {
    /// A file: its shell icon. `dim` for files that no longer exist.
    Item { icon: HICON, dim: bool },
    /// A group: a filled disc with the group's first letter.
    Group,
    /// "Back to the circle" inside a group view.
    Back,
    /// A rule suggestion: accent disc with "+N".
    Suggestion { count: usize },
}

pub struct SlotView<'a> {
    pub label: &'a str,
    pub look: SlotLook,
}

pub struct RingView<'a> {
    pub title: &'a str,
    /// Page indicator or hint, under the title.
    pub subtitle: &'a str,
    pub search: Option<(&'a str, usize)>,
    pub slots: &'a [SlotView<'a>],
    pub hover: Option<usize>,
    pub recent: bool,
    pub insertion: Option<(usize, bool)>,
}

/// An open ring: rim, centre disc with title/subtitle, and up to `PER_PAGE` slots.
pub fn ring(s: &mut Surface, dpi: u32, m: &Metrics, v: &RingView) {
    let dc = s.dc();
    let size = s.w;
    let c = size / 2;
    let sc = |x| win::scale(x, dpi);
    let bg = if v.recent { RECENT_BG } else { CIRCLE_BG };
    fill_rect(dc, &win::rect(0, 0, size, size), ACCENT);
    let rim = sc(3).max(2);
    fill_disc(s, &win::rect(rim, rim, size - rim, size - rim), bg);
    fill_disc(s, &square(c, c, m.circle_d), ACCENT);
    fill_disc(s, &square(c, c, m.circle_d - sc(4)), CENTER_BG);
    unsafe { SetBkMode(dc, TRANSPARENT as i32) };
    let bold = Font::new(dpi, true, 100);
    let small = Font::new(dpi, false, 100);
    let inner = m.circle_d / 2 - sc(8);
    text(
        dc,
        v.title,
        win::rect(
            c - m.circle_d / 2 + sc(10),
            c - m.circle_d / 2 + m.circle_d * 20 / 100,
            c + m.circle_d / 2 - sc(10),
            c - m.circle_d / 2 + m.circle_d * 46 / 100,
        ),
        TEXT,
        &bold,
        DT_CENTER | DT_WORDBREAK | DT_EDITCONTROL | DT_END_ELLIPSIS,
    );
    let count_font = Font::new(dpi, false, COUNT_PERCENT);
    if let Some((query, count)) = v.search {
        // Fixed centre bands keep query/result feedback from moving the title.
        text(
            dc,
            &format!("Ara: {query}"),
            win::rect(c - inner, c - sc(2), c + inner, c + sc(14)),
            crate::theme::MINT,
            &small,
            DT_CENTER | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS,
        );
        text(
            dc,
            &format!("{count} sonu\u{e7}"),
            win::rect(c - inner, c - m.circle_d / 2 + m.circle_d * 65 / 100, c + inner, c - m.circle_d / 2 + m.circle_d * 86 / 100),
            if count == 0 { ACCENT } else { crate::theme::MINT },
            &count_font,
            DT_CENTER | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS,
        );
    } else {
        text(
            dc,
            v.subtitle,
            win::rect(c - inner, c - m.circle_d / 2 + m.circle_d * 65 / 100, c + inner, c - m.circle_d / 2 + m.circle_d * 86 / 100),
            SUBTEXT,
            &count_font,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
        );
    }

    for (k, slot) in v.slots.iter().enumerate().take(m.per_page()) {
        let (dx, dy) = geom::slot_center(m, k);
        let b = square(c + dx, c + dy, m.slot);
        if v.hover == Some(k) {
            fill_rect(dc, &b, HOVER);
        }
        let ix = b.left + (m.slot - m.icon) / 2;
        let iy = b.top + sc(3);
        let mut glyph = |color: u32, letter: &str| {
            fill_disc(s, &win::rect(ix, iy, ix + m.icon, iy + m.icon), color);
            text(dc, letter, win::rect(ix, iy, ix + m.icon, iy + m.icon), TEXT, &bold, DT_CENTER | DT_SINGLELINE | DT_VCENTER);
        };
        let mut label_color = TEXT;
        match slot.look {
            SlotLook::Item { icon, dim } => {
                if !icon.is_null() {
                    unsafe { DrawIconEx(dc, ix, iy, icon, m.icon, m.icon, 0, std::ptr::null_mut(), DI_NORMAL) };
                }
                if dim {
                    label_color = SUBTEXT;
                }
            }
            SlotLook::Group => {
                let first: String = slot.label.chars().take(1).collect::<String>().to_uppercase();
                glyph(GROUP, &first);
            }
            SlotLook::Back => glyph(CENTER_BG, "\u{2039}"),
            SlotLook::Suggestion { count } => glyph(ACCENT, &format!("+{count}")),
        }
        if let Some((target, after)) = v.insertion.filter(|(target, _)| *target == k) {
            let _ = target;
            let y = if after { b.bottom - sc(2) } else { b.top };
            fill_rect(dc, &win::rect(b.left, y, b.right, y + sc(2).max(1)), crate::theme::MINT);
        }
        let lr = win::rect(b.left, iy + m.icon + sc(2), b.right, b.bottom);
        text(dc, slot.label, lr, label_color, &small, DT_CENTER | DT_WORDBREAK | DT_EDITCONTROL | DT_END_ELLIPSIS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circles::geom;

    /// Writes `px` (premultiplied BGRA) composited over a desktop-blue background as a 24-bit BMP.
    fn save_bmp(path: &std::path::Path, px: &[u32], w: usize, h: usize) {
        let row = (w * 3).div_ceil(4) * 4;
        let mut f: Vec<u8> = Vec::with_capacity(54 + row * h);
        let size = (54 + row * h) as u32;
        f.extend_from_slice(b"BM");
        f.extend_from_slice(&size.to_le_bytes());
        f.extend_from_slice(&[0, 0, 0, 0, 54, 0, 0, 0, 40, 0, 0, 0]);
        f.extend_from_slice(&(w as i32).to_le_bytes());
        f.extend_from_slice(&(-(h as i32)).to_le_bytes());
        f.extend_from_slice(&[1, 0, 24, 0]);
        f.extend_from_slice(&[0u8; 24]);
        let bg = [0x80u32, 0x60, 0x40]; // B, G, R
        for y in 0..h {
            for x in 0..w {
                let p = px[y * w + x];
                let a = p >> 24;
                for (i, shift) in [0u32, 8, 16].into_iter().enumerate() {
                    let c = (p >> shift) & 0xff;
                    f.push((c + bg[i] * (255 - a) / 255).min(255) as u8);
                }
            }
            f.extend(std::iter::repeat_n(0u8, row - w * 3));
        }
        std::fs::write(path, f).unwrap();
    }

    #[test]
    fn closed_circle_inner_rim_blends_at_desktop_dpis() {
        for dpi in [96, 120, 144, 192] {
            let d = geom::metrics(dpi).circle_d;
            let mut surface = Surface::new(d, d).expect("native test DIB");
            circle(&mut surface, dpi, "APPS", 4, false);
            let to_bgra = |c: u32| ((c & 0xff) << 16) | (c & 0xff00) | ((c >> 16) & 0xff);
            let accent = to_bgra(ACCENT);
            let background = to_bgra(CIRCLE_BG);
            // Only the top cap: no text or outer alpha mask can fake an inner-edge blend.
            let pixels = surface.pixels();
            for y in 0..d / 5 {
                for x in 0..d {
                    assert_eq!(
                        pixels[(y * d + x) as usize],
                        pixels[(y * d + d - 1 - x) as usize],
                        "dpi {dpi}: inner rim must be symmetric around the circle centre"
                    );
                }
            }
            let blended =
                pixels[..(d * (d / 5)) as usize].iter().filter(|p| **p & 0x00ff_ffff != accent && **p & 0x00ff_ffff != background).count();
            assert!(blended > 8, "dpi {dpi}: the inner rim still has a hard aliased edge");
        }
    }

    #[test]
    fn recent_matches_circle_palette_and_lock_is_visible_at_desktop_dpis() {
        for dpi in [96, 120, 144, 192] {
            let m = geom::metrics(dpi);
            let mut normal = Surface::new(m.circle_d, m.circle_d).unwrap();
            let mut locked = Surface::new(m.circle_d, m.circle_d).unwrap();
            circle(&mut normal, dpi, "Music", 4, false);
            circle(&mut locked, dpi, "Music", 4, true);
            assert_ne!(normal.pixels(), locked.pixels());
            let side = m.ring_r * 2;
            let mut a = Surface::new(side, side).unwrap();
            let mut b = Surface::new(side, side).unwrap();
            let mut v = RingView { title: "Title", subtitle: "4", search: None, slots: &[], hover: None, recent: false, insertion: None };
            ring(&mut a, dpi, &m, &v);
            v.recent = true;
            ring(&mut b, dpi, &m, &v);
            assert_eq!(a.pixels(), b.pixels());
            assert_ne!(CIRCLE_BG, CENTER_BG);
        }
    }

    /// Visual check, not run by default: `cargo test --release preview -- --ignored`
    /// then open `%TEMP%\desktuk-preview-*.bmp`.
    #[test]
    #[ignore]
    fn preview() {
        let dpi = 144;
        let mut drag = drag_icon(std::ptr::null_mut(), dpi).unwrap();
        let drag_size = drag.w as usize;
        save_bmp(&std::env::temp_dir().join("desktuk-preview-drag.bmp"), drag.pixels(), drag_size, drag_size);
        let m = geom::metrics(dpi);
        let d = m.circle_d;
        let mut s = Surface::new(d, d).unwrap();
        circle(&mut s, dpi, "Kitaplar ve Dergiler", 128, false);
        geom::mask_disc(s.pixels(), d as usize, d as usize, d as f32 / 2.0);
        save_bmp(&std::env::temp_dir().join("desktuk-preview-circle.bmp"), s.pixels(), d as usize, d as usize);

        circle(&mut s, dpi, "Music", 128, true);
        geom::mask_disc(s.pixels(), d as usize, d as usize, d as f32 / 2.0);
        save_bmp(&std::env::temp_dir().join("desktuk-preview-locked.bmp"), s.pixels(), d as usize, d as usize);

        let size = 2 * m.ring_r;
        let mut r = Surface::new(size, size).unwrap();
        let labels: Vec<String> = (0..20).map(|i| format!("Song {i:02}.wav")).collect();
        let mut slots: Vec<SlotView> =
            labels.iter().map(|l| SlotView { label: l, look: SlotLook::Item { icon: std::ptr::null_mut(), dim: false } }).collect();
        slots[0] = SlotView { label: "Titled Project", look: SlotLook::Group };
        slots[1] = SlotView { label: ".wav \u{2192} Music", look: SlotLook::Suggestion { count: 12 } };
        slots[2] = SlotView { label: "Back", look: SlotLook::Back };
        ring(
            &mut r,
            dpi,
            &m,
            &RingView {
                title: "Music",
                subtitle: "21-40 / 300",
                search: None,
                slots: &slots,
                hover: Some(5),
                recent: false,
                insertion: None,
            },
        );
        geom::mask_disc(r.pixels(), size as usize, size as usize, m.ring_r as f32);
        save_bmp(&std::env::temp_dir().join("desktuk-preview-ring.bmp"), r.pixels(), size as usize, size as usize);
    }
}
