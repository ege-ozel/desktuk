//! GDI drawing of closed circles and open rings into a `Surface`.
//!
//! Everything is drawn on an opaque background; the caller then applies
//! `geom::mask_disc`, which sets the alpha channel (GDI text leaves alpha bytes at 0,
//! so the mask must always run last).

use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Gdi::{
    CreateFontIndirectW, CreateSolidBrush, DeleteObject, DrawTextW, Ellipse, FillRect, GetStockObject, SelectObject, SetBkMode,
    SetTextColor, DT_CENTER, DT_EDITCONTROL, DT_END_ELLIPSIS, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK, FW_SEMIBOLD, HDC,
    HFONT, LOGFONTW, NULL_PEN, TRANSPARENT,
};
use windows_sys::Win32::UI::HiDpi::SystemParametersInfoForDpi;
use windows_sys::Win32::UI::WindowsAndMessaging::{DrawIconEx, DI_NORMAL, HICON, SPI_GETICONTITLELOGFONT};

use super::geom::{self, Metrics};
use super::surface::Surface;
use crate::win;

pub const CIRCLE_BG: u32 = win::rgb(30, 31, 36);
pub const RECENT_BG: u32 = win::rgb(32, 48, 68);
pub const CENTER_BG: u32 = win::rgb(40, 42, 50);
pub const ACCENT: u32 = crate::node::ACCENT;
pub const HOVER: u32 = win::rgb(56, 60, 72);
pub const GROUP: u32 = win::rgb(70, 90, 130);
pub const TEXT: u32 = win::rgb(240, 240, 240);
pub const SUBTEXT: u32 = win::rgb(170, 175, 185);

/// The icon-title font scaled to `pct` percent, deleted on drop.
struct Font(HFONT);

impl Font {
    fn new(dpi: u32, bold: bool, pct: i32) -> Font {
        unsafe {
            let mut lf: LOGFONTW = std::mem::zeroed();
            SystemParametersInfoForDpi(
                SPI_GETICONTITLELOGFONT,
                std::mem::size_of::<LOGFONTW>() as u32,
                (&mut lf as *mut LOGFONTW).cast(),
                0,
                dpi,
            );
            lf.lfHeight = lf.lfHeight * pct / 100;
            if bold {
                lf.lfWeight = FW_SEMIBOLD as i32;
            }
            Font(CreateFontIndirectW(&lf))
        }
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

fn fill_ellipse(dc: HDC, r: &RECT, color: u32) {
    unsafe {
        let b = CreateSolidBrush(color);
        let old_b = SelectObject(dc, b);
        let old_p = SelectObject(dc, GetStockObject(NULL_PEN));
        // With a null pen the filled area is one pixel smaller; compensate.
        Ellipse(dc, r.left, r.top, r.right + 1, r.bottom + 1);
        SelectObject(dc, old_p);
        SelectObject(dc, old_b);
        DeleteObject(b);
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
pub fn circle(s: &Surface, dpi: u32, name: &str, count: usize, recent: bool) {
    let dc = s.dc();
    let d = s.w;
    let sc = |v| win::scale(v, dpi);
    fill_rect(dc, &win::rect(0, 0, d, d), ACCENT);
    let rim = sc(3).max(2);
    fill_ellipse(dc, &win::rect(rim, rim, d - rim, d - rim), if recent { RECENT_BG } else { CIRCLE_BG });
    unsafe { SetBkMode(dc, TRANSPARENT as i32) };
    let bold = Font::new(dpi, true, 100);
    let small = Font::new(dpi, false, 90);
    let pad = sc(12);
    text(
        dc,
        name,
        win::rect(pad, d * 24 / 100, d - pad, d * 62 / 100),
        TEXT,
        &bold,
        DT_CENTER | DT_WORDBREAK | DT_EDITCONTROL | DT_END_ELLIPSIS,
    );
    text(
        dc,
        &count.to_string(),
        win::rect(pad, d * 62 / 100, d - pad, d * 80 / 100),
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
    pub slots: &'a [SlotView<'a>],
    pub hover: Option<usize>,
    pub recent: bool,
}

/// An open ring: rim, centre disc with title/subtitle, and up to `PER_PAGE` slots.
pub fn ring(s: &Surface, dpi: u32, m: &Metrics, v: &RingView) {
    let dc = s.dc();
    let size = s.w;
    let c = size / 2;
    let sc = |x| win::scale(x, dpi);
    let bg = if v.recent { RECENT_BG } else { CIRCLE_BG };
    fill_rect(dc, &win::rect(0, 0, size, size), ACCENT);
    let rim = sc(3).max(2);
    fill_ellipse(dc, &win::rect(rim, rim, size - rim, size - rim), bg);
    fill_ellipse(dc, &square(c, c, m.circle_d), CENTER_BG);
    unsafe { SetBkMode(dc, TRANSPARENT as i32) };
    let bold = Font::new(dpi, true, 100);
    let small = Font::new(dpi, false, 100);
    let inner = m.circle_d / 2 - sc(8);
    text(
        dc,
        v.title,
        win::rect(c - inner, c - inner, c + inner, c + sc(6)),
        TEXT,
        &bold,
        DT_CENTER | DT_WORDBREAK | DT_EDITCONTROL | DT_END_ELLIPSIS,
    );
    text(dc, v.subtitle, win::rect(c - inner, c + sc(8), c + inner, c + inner), SUBTEXT, &small, DT_CENTER | DT_WORDBREAK);

    for (k, slot) in v.slots.iter().enumerate().take(m.per_page()) {
        let (dx, dy) = geom::slot_center(m, k);
        let b = square(c + dx, c + dy, m.slot);
        if v.hover == Some(k) {
            fill_rect(dc, &b, HOVER);
        }
        let ix = b.left + (m.slot - m.icon) / 2;
        let iy = b.top + sc(3);
        let glyph = |color: u32, letter: &str| {
            fill_ellipse(dc, &win::rect(ix, iy, ix + m.icon, iy + m.icon), color);
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

    /// Visual check, not run by default: `cargo test --release preview -- --ignored`
    /// then open `%TEMP%\desktuk-preview-*.bmp`.
    #[test]
    #[ignore]
    fn preview() {
        let dpi = 144;
        let m = geom::metrics(dpi);
        let d = m.circle_d;
        let mut s = Surface::new(d, d).unwrap();
        circle(&s, dpi, "Kitaplar ve Dergiler", 128, false);
        geom::mask_disc(s.pixels(), d as usize, d as usize, d as f32 / 2.0);
        save_bmp(&std::env::temp_dir().join("desktuk-preview-circle.bmp"), s.pixels(), d as usize, d as usize);

        let size = 2 * m.ring_r;
        let mut r = Surface::new(size, size).unwrap();
        let labels: Vec<String> = (0..20).map(|i| format!("Song {i:02}.wav")).collect();
        let mut slots: Vec<SlotView> =
            labels.iter().map(|l| SlotView { label: l, look: SlotLook::Item { icon: std::ptr::null_mut(), dim: false } }).collect();
        slots[0] = SlotView { label: "Titled Project", look: SlotLook::Group };
        slots[1] = SlotView { label: ".wav \u{2192} Music", look: SlotLook::Suggestion { count: 12 } };
        slots[2] = SlotView { label: "Back", look: SlotLook::Back };
        ring(&r, dpi, &m, &RingView { title: "Music", subtitle: "21-40 / 300", slots: &slots, hover: Some(5), recent: false });
        geom::mask_disc(r.pixels(), size as usize, size as usize, m.ring_r as f32);
        save_bmp(&std::env::temp_dir().join("desktuk-preview-ring.bmp"), r.pixels(), size as usize, size as usize);
    }
}
