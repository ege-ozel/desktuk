//! Small, dependency-free helpers around raw Win32 calls.

use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::Path;

use windows_sys::Win32::Foundation::{FILETIME, HWND, POINT, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetWindowRect};

/// Null-terminated UTF-16 string.
pub fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(std::iter::once(0)).collect()
}

pub fn wide_path(p: &Path) -> Vec<u16> {
    wide(p.as_os_str())
}

/// Converts a (possibly null-terminated) UTF-16 buffer to a String.
pub fn from_wide(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    OsString::from_wide(&buf[..len]).to_string_lossy().into_owned()
}

/// Converts a null-terminated UTF-16 pointer to an OsString.
///
/// # Safety
/// `p` must be null or point to a valid null-terminated UTF-16 string.
pub unsafe fn os_from_pwstr(p: *const u16) -> OsString {
    if p.is_null() {
        return OsString::new();
    }
    let mut len = 0;
    while *p.add(len) != 0 {
        len += 1;
    }
    OsString::from_wide(std::slice::from_raw_parts(p, len))
}

pub fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    if n <= 0 {
        return String::new();
    }
    from_wide(&buf[..n as usize])
}

pub fn window_rect(hwnd: HWND) -> RECT {
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    unsafe { GetWindowRect(hwnd, &mut r) };
    r
}

pub fn rect(left: i32, top: i32, right: i32, bottom: i32) -> RECT {
    RECT { left, top, right, bottom }
}

pub fn rect_contains(r: &RECT, p: POINT) -> bool {
    p.x >= r.left && p.x < r.right && p.y >= r.top && p.y < r.bottom
}

pub fn filetime_to_u64(ft: &FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
}

/// Scales a 96-DPI value to `dpi`.
pub fn scale(v: i32, dpi: u32) -> i32 {
    (v as i64 * dpi as i64 / 96) as i32
}

/// Signed x coordinate from an LPARAM (GET_X_LPARAM).
#[inline]
pub fn x_lparam(l: isize) -> i32 {
    (l & 0xffff) as u16 as i16 as i32
}

/// Signed y coordinate from an LPARAM (GET_Y_LPARAM).
#[inline]
pub fn y_lparam(l: isize) -> i32 {
    ((l >> 16) & 0xffff) as u16 as i16 as i32
}

pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    r as u32 | ((g as u32) << 8) | ((b as u32) << 16)
}
