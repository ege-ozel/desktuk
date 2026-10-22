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
    let samples = output.join("samples");
    std::fs::create_dir_all(&samples).expect("example directory");
    // Only exporter-owned files under target/. Never enumerate the real Desktop.
    let names =
        ["Mix notes.txt", "Mixer preset.txt", "Summer mix.wav", "Session.wav", "Artwork.bmp", "Reference.txt", "Readme.txt", "Master.wav"];
    let mut items: Vec<crate::items::Item> = names
        .iter()
        .map(|name| {
            let path = samples.join(name);
            if !path.exists() {
                std::fs::write(&path, b"DeskTuk README example\r\n").expect("example file");
            }
            crate::items::Item::load(path)
        })
        .collect();
    let dpi = 192;
    let m = geom::desktop_metrics(dpi, win::scale(48, dpi), win::scale(700, dpi));
    let views: Vec<draw::SlotView<'_>> = items
        .iter_mut()
        .map(|item| draw::SlotView { label: "", look: draw::SlotLook::Item { icon: item.icon(m.icon), dim: false } })
        .zip(names)
        .map(|(mut view, name)| {
            view.label = name;
            view
        })
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
    let (w, h) = (2160usize, 1600usize);
    let mut canvas = vec![0xff0c1219u32; w * h];
    composite(&mut canvas, w, h, &mut ring, 180, 120);
    let mut circle = Surface::new(m.circle_d, m.circle_d).expect("circle surface");
    for (x, y, label, count) in [(36, 120, "Recent", 12), (60, 1210, "Apps", 5)] {
        draw::circle(&mut circle, dpi, label, count, false);
        let d = circle.w as usize;
        geom::mask_disc(circle.pixels(), d, d, d as f32 / 2.0);
        composite(&mut canvas, w, h, &mut circle, x, y);
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
    composite(&mut canvas, w, h, &mut dock, 1776, 420);
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
    let mut search = vec![0xff0c1219u32; (side + 48) * (side + 48)];
    composite(&mut search, side + 48, side + 48, &mut ring, 24, 24);
    save_bgra(&output.join("search-results.bgra"), &search);
    std::fs::write(output.join("sizes.txt"), format!("desktop-overview {w} {h}\nsearch-results {} {}\n", side + 48, side + 48))
        .expect("size manifest");
    drop(views);
    drop(items);
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
            let channel = |shift: u32| (((src >> shift) & 255) + ((*dst >> shift) & 255) * (255 - a) / 255).min(255) << shift;
            *dst = 0xff000000 | channel(16) | channel(8) | channel(0);
        }
    }
}
fn save_bgra(path: &Path, pixels: &[u32]) {
    let bytes: Vec<u8> = pixels.iter().flat_map(|p| p.to_le_bytes()).collect();
    std::fs::write(path, bytes).expect("BGRA export");
}
