//! Shared DeskTuk identity, rendered with the platform's native text and controls.
use crate::win;
use windows_sys::Win32::Graphics::Gdi::{CreateFontIndirectW, FW_NORMAL, FW_SEMIBOLD, HFONT, LOGFONTW};
use windows_sys::Win32::UI::HiDpi::SystemParametersInfoForDpi;
use windows_sys::Win32::UI::WindowsAndMessaging::SPI_GETICONTITLELOGFONT;

pub const BG: u32 = win::rgb(18, 27, 36);
pub const SURFACE: u32 = win::rgb(25, 38, 50);
pub const RAISED: u32 = win::rgb(34, 51, 65);
pub const BORDER: u32 = win::rgb(43, 60, 78);
pub const ACCENT: u32 = win::rgb(127, 170, 255);
pub const MINT: u32 = win::rgb(113, 216, 194);
pub const TEXT: u32 = win::rgb(236, 242, 248);
pub const MUTED: u32 = win::rgb(148, 168, 188);

/// Retain the user's Windows text size; use a consistent installed UI family.
pub fn font(dpi: u32, semibold: bool, percent: i32) -> HFONT {
    let mut lf: LOGFONTW = unsafe { std::mem::zeroed() };
    unsafe {
        SystemParametersInfoForDpi(
            SPI_GETICONTITLELOGFONT,
            std::mem::size_of::<LOGFONTW>() as u32,
            (&mut lf as *mut LOGFONTW).cast(),
            0,
            dpi,
        );
    }
    if lf.lfHeight == 0 {
        lf.lfHeight = -win::scale(12, dpi);
    }
    lf.lfHeight = lf.lfHeight * percent / 100;
    lf.lfQuality = 5; // CLEARTYPE_QUALITY: use Windows text smoothing for native controls.
    lf.lfWeight = if semibold { FW_SEMIBOLD } else { FW_NORMAL } as i32;
    lf.lfFaceName = [0; 32];
    for (slot, ch) in lf.lfFaceName.iter_mut().zip("Segoe UI".encode_utf16()) {
        *slot = ch;
    }
    unsafe { CreateFontIndirectW(&lf) }
}
