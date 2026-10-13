//! A 32-bit top-down DIB section for painting per-pixel-alpha (layered) windows.
//!
//! GDI draws into `dc()`; `geom::mask_disc` then turns the pixels into a premultiplied
//! disc and `present` hands them to the window with `UpdateLayeredWindow`. The system
//! keeps its own copy of what was presented, so a surface can be dropped right after.

use windows_sys::Win32::Foundation::{HWND, POINT, SIZE};
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GdiFlush, SelectObject, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{UpdateLayeredWindow, ULW_ALPHA};

pub struct Surface {
    dc: HDC,
    bmp: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u32,
    pub w: i32,
    pub h: i32,
}

impl Surface {
    /// `None` if GDI is out of resources.
    pub fn new(w: i32, h: i32) -> Option<Surface> {
        let (w, h) = (w.max(1), h.max(1));
        unsafe {
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            if dc.is_null() {
                return None;
            }
            let mut bi: BITMAPINFO = std::mem::zeroed();
            bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bi.bmiHeader.biWidth = w;
            bi.bmiHeader.biHeight = -h; // top-down rows
            bi.bmiHeader.biPlanes = 1;
            bi.bmiHeader.biBitCount = 32;
            bi.bmiHeader.biCompression = BI_RGB;
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let bmp = CreateDIBSection(dc, &bi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
            if bmp.is_null() || bits.is_null() {
                if !bmp.is_null() {
                    DeleteObject(bmp);
                }
                DeleteDC(dc);
                return None;
            }
            let old = SelectObject(dc, bmp);
            Some(Surface { dc, bmp, old, bits: bits.cast(), w, h })
        }
    }

    pub fn dc(&self) -> HDC {
        self.dc
    }

    /// The pixels as `0xAARRGGBB`, row-major, `w * h` long. Flushes pending GDI drawing first.
    pub fn pixels(&mut self) -> &mut [u32] {
        unsafe {
            GdiFlush();
            std::slice::from_raw_parts_mut(self.bits, (self.w * self.h) as usize)
        }
    }

    /// Shows the (premultiplied) pixels as the content of layered window `hwnd`, whose
    /// top-left corner is moved to `at` (screen pixels) and whose size becomes `w`x`h`.
    pub fn present(&self, hwnd: HWND, at: POINT) -> bool {
        let size = SIZE { cx: self.w, cy: self.h };
        let src = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
        unsafe { UpdateLayeredWindow(hwnd, std::ptr::null_mut(), &at, &size, self.dc, &src, 0, &blend, ULW_ALPHA) != 0 }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old);
            DeleteObject(self.bmp);
            DeleteDC(self.dc);
        }
    }
}
