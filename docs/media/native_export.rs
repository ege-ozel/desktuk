//! Opt-in documentation export. No windows, hooks, physical input or user configuration.
use super::*;
use crate::circles::{draw, geom, surface::Surface};
use std::path::Path;
use windows_sys::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

// Execute only this exporter, not the application regression suite:
// See docs/media/README.md for the full command.
#[test]
#[ignore = "exports README assets using native drawing, not a regression test"]
fn docs_native_export() {
    crate::i18n::set_english(true);
    let com = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };
    assert!(com >= 0, "COM initialization: {com:#x}");
    let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/d58-media");
    std::fs::create_dir_all(&output).expect("export directory");
    // These are .ico example references, not new folder themes or file-type skins.
    let names =
        ["Mix notes.ico", "Mixer preset.ico", "Summer mix.ico", "Session.ico", "Artwork.ico", "Reference.ico", "Readme.ico", "Master.ico"];
    let files = ["text-documents", "folder", "music", "archive", "pictures", "web", "documents", "movies"];
    let dpi = 192;
    let m = geom::desktop_metrics(dpi, win::scale(48, dpi), win::scale(700, dpi));
    let icons: Vec<PreviewIcon> = files
        .iter()
        .map(|name| {
            let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("docs/media/icons/bluefx-{name}.ico"));
            PreviewIcon::load(&path, m.icon)
        })
        .collect();
    let views: Vec<draw::SlotView<'_>> = icons
        .iter()
        .zip(names)
        .map(|(icon, name)| draw::SlotView { label: name, look: draw::SlotLook::Item { icon: icon.0, dim: false } })
        .collect();
    let mut ring = Surface::new(m.ring_r * 2, m.ring_r * 2).expect("ring surface");
    draw::ring(
        &mut ring,
        dpi,
        &m,
        &draw::RingView {
            title: "Music",
            subtitle: "Type to search",
            search: None,
            slots: &views,
            hover: Some(0),
            recent: false,
            insertion: None,
        },
    );
    let side = ring.w as usize;
    geom::mask_disc(ring.pixels(), side, side, m.ring_r as f32);
    // Breathing room lets the oval shadow finish before the image boundary.
    let pad = 160;
    let (w, h) = (2160usize + pad * 2, 1600usize + pad * 2);
    let mut canvas = backdrop(w, h);
    composite(&mut canvas, w, h, &mut ring, 180 + pad, 120 + pad);
    let mut circle = Surface::new(m.circle_d, m.circle_d).expect("circle surface");
    for (x, y, label, count) in [(36, 120, "Recent", 12), (60, 1210, "Apps", 5)] {
        draw::circle(&mut circle, dpi, label, count, false);
        let d = circle.w as usize;
        geom::mask_disc(circle.pixels(), d, d, d as f32 / 2.0);
        composite(&mut canvas, w, h, &mut circle, x + pad, y + pad);
    }
    let entries = vec![
        (Entry::Target(Which::Recent), "Recent".into()),
        (Entry::Target(Which::User(0)), "Music".into()),
        (Entry::Target(Which::User(1)), "Apps".into()),
    ];
    EXPANDED.set(true);
    // Same dimensions as dimensions(): fixed header/tools + three visible rows + padding.
    let dw = expanded_width(dpi);
    let dh = scale(HEADER_H + GAP + ACTION_SIZE + TOOLS_GAP + TOOLS_H + 3 * ROW_H + PAD, dpi);
    let mut dock = Surface::new(dw, dh).expect("dock surface");
    paint_contents(dock.dc(), win::rect(0, 0, dw, dh), dpi, &entries, 3, true, false);
    // Offline GDI does not supply DWM's outer window clipping. Keep the rectangular
    // client border; do not invent a different rounded frame in its place.
    for pixel in dock.pixels() {
        *pixel |= 0xff000000;
    }
    composite(&mut canvas, w, h, &mut dock, 1776 + pad, 420 + pad);
    save_bgra(&output.join("desktop-overview.bgra"), &canvas);

    let filtered = &views[..3];
    draw::ring(
        &mut ring,
        dpi,
        &m,
        &draw::RingView {
            title: "Music",
            subtitle: "",
            search: Some(("mix", 3)),
            slots: filtered,
            hover: Some(0),
            recent: false,
            insertion: None,
        },
    );
    geom::mask_disc(ring.pixels(), side, side, m.ring_r as f32);
    let search_side = side + pad * 2;
    let mut search = backdrop(search_side, search_side);
    composite(&mut search, search_side, search_side, &mut ring, pad, pad);
    save_bgra(&output.join("search-results.bgra"), &search);
    std::fs::write(output.join("sizes.txt"), format!("desktop-overview {w} {h}\nsearch-results {search_side} {search_side}\n"))
        .expect("size manifest");
    drop(views);
    drop(icons);
    if com >= 0 {
        unsafe { CoUninitialize() };
    }
}

fn composite(canvas: &mut [u32], width: usize, height: usize, source: &mut Surface, left: usize, top: usize) {
    let (sw, sh) = (source.w as usize, source.h as usize);
    for (y, row) in source.pixels().chunks_exact(sw).take(sh).enumerate() {
        if top + y >= height {
            break;
        }
        for (x, &src) in row.iter().enumerate() {
            if left + x >= width {
                break;
            }
            let dst = &mut canvas[(top + y) * width + left + x];
            let a = src >> 24;
            let da = (*dst >> 24) * (255 - a) / 255;
            let out_a = a + da;
            // Native surfaces are premultiplied; the PNG canvas uses straight alpha.
            let channel = |shift: u32| {
                if out_a == 0 {
                    0
                } else {
                    ((((src >> shift) & 255) * 255 + ((*dst >> shift) & 255) * da) / out_a).min(255) << shift
                }
            };
            *dst = out_a << 24 | channel(16) | channel(8) | channel(0);
        }
    }
}
fn save_bgra(path: &Path, pixels: &[u32]) {
    let bytes: Vec<u8> = pixels.iter().flat_map(|p| p.to_le_bytes()).collect();
    std::fs::write(path, bytes).expect("BGRA export");
}

// Native ICO loader used only for the documentation fixture. Ownership is local.
struct PreviewIcon(windows_sys::Win32::UI::WindowsAndMessaging::HICON);
impl PreviewIcon {
    fn load(path: &Path, side: i32) -> Self {
        use windows_sys::Win32::UI::WindowsAndMessaging::{LoadImageW, IMAGE_ICON, LR_LOADFROMFILE};
        let icon = unsafe { LoadImageW(std::ptr::null_mut(), win::wide_path(path).as_ptr(), IMAGE_ICON, side, side, LR_LOADFROMFILE) };
        assert!(!icon.is_null(), "example ICO: {}", path.display());
        Self(icon)
    }
}
impl Drop for PreviewIcon {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::DestroyIcon(self.0);
        }
    }
}

// Presentation background only: keep native UI pixels/palette unchanged.
fn backdrop(w: usize, h: usize) -> Vec<u32> {
    let mut pixels = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let dx = 2.0 * x as f32 / (w - 1) as f32 - 1.0;
            let dy = 2.0 * y as f32 / (h - 1) as f32 - 1.0;
            // A single low-opacity ellipse: no nearest-edge/minimum operation,
            // rectangular plateau or diagonal seams on a white README page.
            let falloff = (1.0 - dx * dx - dy * dy).max(0.0);
            let alpha = (80.0 * falloff * falloff).round() as u32;
            pixels.push(alpha << 24 | 0x121b24);
        }
    }
    pixels
}
