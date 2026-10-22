//! On-demand native edit/search popup. No filesystem reads, icons, workers or idle timers.
use super::surface::Surface;
use super::view::{self, SlotRef, Target};
use crate::{app::app, i18n, theme, win};
use std::cell::RefCell;
use windows_sys::Win32::Foundation::{GetLastError, ERROR_CLASS_ALREADY_EXISTS, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::{DRAWITEMSTRUCT, EM_SETSEL, MEASUREITEMSTRUCT, ODS_SELECTED};
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::*;
const CLASS: &str = "DeskTuk.Search";
const CLOSE: u32 = WM_APP + 75;
const OPEN: u32 = WM_APP + 76;
const EM_LIMITTEXT: u32 = 0x00c5;
// Four tiny cached corner tiles keep coverage smooth without a full popup bitmap.
struct FrameCorners {
    pixels: Surface,
    size: i32,
    inset: i32,
}
impl FrameCorners {
    fn new(w: i32, h: i32, dpi: u32) -> Option<Self> {
        let size = win::scale(12, dpi);
        let inset = win::scale(2, dpi);
        let radius = win::scale(6, dpi) as f32;
        let mut surface = Surface::new(size * 4, size)?;
        for corner in 0..4 {
            let ox = if corner % 2 == 0 { 0 } else { w - size };
            let oy = if corner < 2 { 0 } else { h - size };
            for y in 0..size {
                for x in 0..size {
                    let qx = (ox as f32 + x as f32 + 0.5 - w as f32 / 2.0).abs() - (w as f32 / 2.0 - inset as f32 - 0.5 - radius);
                    let qy = (oy as f32 + y as f32 + 0.5 - h as f32 / 2.0).abs() - (h as f32 / 2.0 - inset as f32 - 0.5 - radius);
                    let d = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - radius;
                    let alpha = (1.0 - d.abs()).clamp(0.0, 1.0);
                    let channel = |shift: u32| {
                        (((theme::BG >> shift & 255) as f32 * (1.0 - alpha) + (theme::MINT >> shift & 255) as f32 * alpha).round()) as u32
                    };
                    surface.pixels()[(y * size * 4 + corner * size + x) as usize] =
                        0xff00_0000 | channel(0) << 16 | channel(8) << 8 | channel(16);
                }
            }
        }
        Some(Self { pixels: surface, size, inset })
    }
    unsafe fn draw(&self, dc: HDC, w: i32, h: i32) {
        let brush = GetStockObject(DC_BRUSH);
        SetDCBrushColor(dc, theme::MINT);
        let c = self.size;
        let i = self.inset;
        for rect in [
            win::rect(c, i, w - c, i + 1),
            win::rect(c, h - i - 1, w - c, h - i),
            win::rect(i, c, i + 1, h - c),
            win::rect(w - i - 1, c, w - i, h - c),
        ] {
            FillRect(dc, &rect, brush);
        }
        for corner in 0..4 {
            let x = if corner % 2 == 0 { 0 } else { w - c };
            let y = if corner < 2 { 0 } else { h - c };
            BitBlt(dc, x, y, c, c, self.pixels.dc(), corner * c, 0, SRCCOPY);
        }
    }
}
struct State {
    hwnd: HWND,
    edit: HWND,
    list: HWND,
    status: HWND,
    font: HFONT,
    brush: HBRUSH,
    corners: Option<FrameCorners>,
    target: Option<Target>,
    base: Vec<SlotRef>,
    source: Vec<SlotRef>,
    matches: Vec<SlotRef>,
}
thread_local! { static ACTIVE: RefCell<Option<State>> = const { RefCell::new(None) }; }
pub fn is_active() -> bool {
    ACTIVE.with(|s| s.borrow().is_some())
}
fn handles() -> Option<(HWND, HWND, HWND)> {
    ACTIVE.with(|s| s.borrow().as_ref().map(|s| (s.hwnd, s.edit, s.list)))
}
pub fn close_ring() {
    let ring = ACTIVE.with(|s| s.borrow().as_ref().is_some_and(|s| s.target.is_none()));
    if ring {
        close();
    }
}
pub fn close() {
    let state = ACTIVE.with(|s| s.borrow_mut().take());
    if let Some(s) = state {
        unsafe {
            DestroyWindow(s.hwnd);
            DeleteObject(s.font);
            DeleteObject(s.brush);
        }
        if s.target.is_none() {
            super::ring::editing_finished();
        }
    }
}
pub fn open(target: Target, at: POINT) {
    let dpi = unsafe { GetDpiForWindow(crate::dock::window()) }.max(96);
    create(Some(target), at, "", dpi);
}
pub fn edit_ring(at: POINT, initial: &str, dpi: u32) {
    if handles().is_none() {
        create(None, at, initial, dpi);
    }
}
fn text(hwnd: HWND) -> String {
    unsafe {
        let n = GetWindowTextLengthW(hwnd).max(0) as usize;
        let mut buf = vec![0u16; n + 1];
        let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
        String::from_utf16_lossy(&buf[..n])
    }
}
fn refs(target: Target) -> Vec<SlotRef> {
    if target == Target::Recent {
        super::recent_paths().into_iter().map(SlotRef::Item).collect()
    } else {
        app().board.try_borrow().map(|b| view::slot_refs(&b, target, &[], &[])).unwrap_or_default()
    }
}
fn candidates(target: Target, base: &[SlotRef]) -> Vec<SlotRef> {
    app().board.try_borrow().map(|b| view::search_candidates(&b, target, base.to_vec())).unwrap_or_else(|_| base.to_vec())
}
pub fn hover_editor() -> bool {
    ACTIVE.with(|s| s.borrow().as_ref().is_some_and(|s| s.target.is_none() && unsafe { GetForegroundWindow() } != s.hwnd))
}
pub fn contains_editor(point: POINT) -> bool {
    let window = ACTIVE.with(|s| s.borrow().as_ref().filter(|s| s.target.is_none()).map(|s| s.hwnd));
    window.is_some_and(|h| win::rect_contains(&win::window_rect(h), point))
}
pub fn dismiss_outside(point: POINT) {
    let window = ACTIVE.with(|s| s.borrow().as_ref().filter(|s| s.target.is_none()).map(|s| s.hwnd));
    if let Some(h) = window {
        if !win::rect_contains(&win::window_rect(h), point) {
            unsafe {
                PostMessageW(h, CLOSE, 0, 0);
            }
        }
    }
}
pub fn ring_key(key: u32, modifiers: usize) {
    let edit = ACTIVE.with(|s| s.borrow().as_ref().filter(|s| s.target.is_none()).map(|s| s.edit));
    let Some(edit) = edit else {
        return;
    };
    if modifiers == 0 && matches!(key as u16, VK_UP | VK_DOWN | VK_RETURN) {
        super::ring::search_key(key);
        return;
    }
    unsafe {
        if modifiers & 2 != 0 {
            let message = match key {
                0x43 => WM_COPY,
                0x58 => WM_CUT,
                0x56 => WM_PASTE,
                0x5a => WM_UNDO,
                _ => 0,
            };
            if message != 0 {
                SendMessageW(edit, message, 0, 0);
                return;
            }
        }
        let mut previous = [0u8; 256];
        GetKeyboardState(previous.as_mut_ptr());
        let mut state = previous;
        for vk in [VK_SHIFT, VK_LSHIFT, VK_RSHIFT] {
            state[vk as usize] = if modifiers & 1 != 0 { 0x80 } else { 0 };
        }
        for vk in [VK_CONTROL, VK_LCONTROL, VK_RCONTROL] {
            state[vk as usize] = if modifiers & 2 != 0 { 0x80 } else { 0 };
        }
        SetKeyboardState(state.as_ptr());
        SendMessageW(edit, WM_KEYDOWN, key as usize, 1);
        SetKeyboardState(previous.as_ptr());
    }
}
pub fn ring_char(value: u32) -> bool {
    let edit = ACTIVE.with(|s| s.borrow().as_ref().filter(|s| s.target.is_none()).map(|s| s.edit));
    let Some(edit) = edit else {
        return false;
    };
    if value == 27 {
        unsafe {
            SetWindowTextW(edit, win::wide("").as_ptr());
        }
        if let Some((hwnd, _, _)) = handles() {
            unsafe {
                PostMessageW(hwnd, CLOSE, 0, 0);
            }
        }
        return true;
    }
    if value == 1 {
        unsafe {
            SendMessageW(edit, EM_SETSEL, 0, -1);
        }
        return true;
    }
    if let Some(c) = char::from_u32(value) {
        let mut buf = [0u16; 2];
        for unit in c.encode_utf16(&mut buf) {
            unsafe {
                SendMessageW(edit, WM_CHAR, *unit as usize, 0);
            }
        }
    }
    true
}
fn create(target: Option<Target>, at: POINT, initial: &str, dpi: u32) {
    close();
    crate::hint::hide();
    let sc = |v| win::scale(v, dpi);
    let (mut w, h) = (sc(340), sc(if target.is_some() { 380 } else { 48 }));
    let mut mi: MONITORINFO = unsafe { std::mem::zeroed() };
    mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    unsafe {
        if GetMonitorInfoW(MonitorFromPoint(at, MONITOR_DEFAULTTONEAREST), &mut mi) == 0 {
            return;
        }
    }
    let (preferred_x, preferred_y) = if target.is_some() {
        let dock = win::window_rect(crate::dock::window());
        (if dock.right + w + sc(4) <= mi.rcWork.right { dock.right + sc(4) } else { dock.left - w - sc(4) }, at.y)
    } else if let Some((bounds, center, circle_d)) = super::ring::search_area() {
        let left = bounds.left - mi.rcWork.left - sc(8);
        let right = mi.rcWork.right - bounds.right - sc(8);
        if left.max(right) >= sc(180) {
            w = w.min(left.max(right));
            (if right >= left { bounds.right + sc(8) } else { bounds.left - sc(8) - w }, center.y - h / 2)
        } else if bounds.top - mi.rcWork.top >= h + sc(8) {
            (center.x - w / 2, bounds.top - h - sc(8))
        } else if mi.rcWork.bottom - bounds.bottom >= h + sc(8) {
            (center.x - w / 2, bounds.bottom + sc(8))
        } else {
            // A compact field inside the centre disc cannot cover file slots.
            w = circle_d - sc(16);
            (center.x - w / 2, center.y - h / 2)
        }
    } else {
        (at.x, at.y)
    };
    let x = preferred_x.clamp(mi.rcWork.left, (mi.rcWork.right - w).max(mi.rcWork.left));
    let y = preferred_y.clamp(mi.rcWork.top, (mi.rcWork.bottom - h).max(mi.rcWork.top));
    let base = target.map(refs).unwrap_or_default();
    let source = target.map(|t| candidates(t, &base)).unwrap_or_default();
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let name = win::wide(CLASS);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance,
            lpszClassName: name.as_ptr(),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            ..std::mem::zeroed()
        };
        if RegisterClassW(&wc) == 0 {
            let error = GetLastError();
            if error != ERROR_CLASS_ALREADY_EXISTS {
                crate::dlog!("search popup class registration failed: {error}");
                return;
            }
        }
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            name.as_ptr(),
            win::wide(i18n::choose("Dosya ara", "Search files")).as_ptr(),
            WS_POPUP | WS_CLIPCHILDREN,
            x,
            y,
            w,
            h,
            app().controller.get(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            crate::dlog!("search popup creation failed: {}", GetLastError());
            return;
        }
        let region = CreateRoundRectRgn(0, 0, w + 1, h + 1, sc(12), sc(12));
        if !region.is_null() && SetWindowRgn(hwnd, region, 0) == 0 {
            DeleteObject(region);
        }
        let child = |class: &str, label: &str, style: u32, id: usize, x, y, w, h| {
            CreateWindowExW(
                0,
                win::wide(class).as_ptr(),
                win::wide(label).as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                x,
                y,
                w,
                h,
                hwnd,
                id as _,
                instance,
                std::ptr::null(),
            )
        };
        let edit = child("EDIT", initial, WS_TABSTOP | ES_AUTOHSCROLL as u32 | ES_NOHIDESEL as u32, 1, sc(10), sc(10), w - sc(22), sc(26));
        let list = if target.is_some() {
            child(
                "LISTBOX",
                "",
                WS_TABSTOP | WS_VSCROLL | LBS_NOTIFY as u32 | LBS_NOINTEGRALHEIGHT as u32 | LBS_OWNERDRAWFIXED as u32 | LBS_NODATA as u32,
                2,
                sc(10),
                sc(48),
                w - sc(22),
                h - sc(82),
            )
        } else {
            std::ptr::null_mut()
        };
        if edit.is_null() || target.is_some() && list.is_null() {
            crate::dlog!("search popup control creation failed: {}", GetLastError());
            DestroyWindow(hwnd);
            return;
        }
        let status = if target.is_some() {
            child("STATIC", "", 1 /* SS_CENTER */, 5, sc(10), h - sc(27), w - sc(20), sc(22))
        } else {
            std::ptr::null_mut()
        };
        let font = theme::font(dpi, false, 110);
        SendMessageW(edit, WM_SETFONT, font as usize, 1);
        SendMessageW(edit, EM_LIMITTEXT, 80, 0);
        SendMessageW(edit, EM_SETSEL, initial.encode_utf16().count(), initial.encode_utf16().count() as isize);
        SendMessageW(edit, 0x1501, 1, win::wide(i18n::choose("Dosya ara…", "Search files…")).as_ptr() as isize);
        SetWindowSubclass(edit, Some(input_proc), 1, 0);
        if !list.is_null() {
            SendMessageW(list, WM_SETFONT, font as usize, 1);
            let dc = GetDC(list);
            if !dc.is_null() {
                let old = SelectObject(dc, font);
                let mut metrics: TEXTMETRICW = std::mem::zeroed();
                GetTextMetricsW(dc, &mut metrics);
                SendMessageW(list, LB_SETITEMHEIGHT, 0, (metrics.tmHeight + sc(5)).max(sc(20)) as isize);
                SelectObject(dc, old);
                ReleaseDC(list, dc);
            }
            SetWindowSubclass(list, Some(input_proc), 1, 0);
            SendMessageW(status, WM_SETFONT, font as usize, 1);
        }
        let brush = CreateSolidBrush(theme::BG);
        let corners = FrameCorners::new(w, h, dpi);
        ACTIVE.with(|s| {
            *s.borrow_mut() = Some(State { hwnd, edit, list, status, font, brush, corners, target, base, source, matches: Vec::new() })
        });
        if target.is_none() {
            super::ring::editing_started();
        } else {
            refresh();
        }
        ShowWindow(hwnd, if target.is_none() { SW_SHOWNOACTIVATE } else { SW_SHOW });
        SetForegroundWindow(hwnd);
        if target.is_none() && GetForegroundWindow() != hwnd {
            // Keep a visible native editor and the transient hover keyboard route.
            // Never inject input or force another application's foreground permission.
            super::search_input::install();
        } else {
            if target.is_none() {
                super::search_input::uninstall();
            }
            SetFocus(edit);
        }
    }
}
fn label(slot: &SlotRef, target: Target) -> String {
    match slot {
        SlotRef::Item(p) => p.file_name().unwrap_or(p.as_os_str()).to_string_lossy().into_owned(),
        SlotRef::Back => i18n::choose("‹ Geri", "‹ Back").to_owned(),
        SlotRef::Group(gi) => {
            let name = app()
                .board
                .try_borrow()
                .ok()
                .and_then(
                    |b| if let Target::Circle(ci) = target { b.circles.get(ci)?.groups.get(*gi).map(|g| g.name.clone()) } else { None },
                )
                .unwrap_or_default();
            format!("{name} ›")
        }
        _ => String::new(),
    }
}
fn refresh() {
    let Some((hwnd, edit, _)) = handles() else {
        return;
    };
    let query = text(edit);
    let target = ACTIVE.with(|s| s.borrow().as_ref().and_then(|s| s.target));
    let Some(_) = target else {
        super::ring::set_search(&query);
        return;
    };
    let snapshot = ACTIVE.with(|s| {
        let mut state = s.borrow_mut();
        let s = state.as_mut()?;
        s.matches = if query.is_empty() { s.base.clone() } else { view::filter_refs(&s.source, &query) };
        Some(s.hwnd)
    });
    if snapshot == Some(hwnd) {
        render();
    }
}
fn render() {
    let snapshot = ACTIVE.with(|s| {
        let s = s.borrow();
        let s = s.as_ref()?;
        Some((s.list, s.status, s.matches.len()))
    });
    let Some((list, status, total)) = snapshot else {
        return;
    };
    unsafe {
        SendMessageW(list, WM_SETREDRAW, 0, 0);
        SendMessageW(list, LB_SETCOUNT, total.min(i32::MAX as usize), 0);
        SendMessageW(list, LB_SETCURSEL, usize::MAX, 0);
        SendMessageW(list, LB_SETTOPINDEX, 0, 0);
        SendMessageW(list, WM_SETREDRAW, 1, 0);
        InvalidateRect(list, std::ptr::null(), 1);
        let caption = format!("{total} {}", i18n::choose("sonuç", "results"));
        SetWindowTextW(status, win::wide(caption).as_ptr());
    }
}
unsafe fn draw_row(draw: &DRAWITEMSTRUCT) {
    if draw.itemID == u32::MAX {
        return;
    }
    let snapshot = ACTIVE.with(|s| {
        let state = s.borrow();
        let s = state.as_ref()?;
        Some((s.matches.get(draw.itemID as usize)?.clone(), s.target?, s.font))
    });
    let Some((slot, target, font)) = snapshot else {
        return;
    };
    let selected = draw.itemState & ODS_SELECTED != 0;
    let background = if selected { theme::RAISED } else { theme::BG };
    SetDCBrushColor(draw.hDC, background);
    FillRect(draw.hDC, &draw.rcItem, GetStockObject(DC_BRUSH));
    let old = SelectObject(draw.hDC, font);
    SetBkMode(draw.hDC, TRANSPARENT as i32);
    SetTextColor(draw.hDC, if selected { theme::MINT } else { theme::TEXT });
    let dpi = GetDpiForWindow(draw.hwndItem).max(96);
    let mut rect = draw.rcItem;
    rect.left += win::scale(6, dpi);
    rect.right -= win::scale(6, dpi);
    let mut text = win::wide(label(&slot, target));
    DrawTextW(draw.hDC, text.as_mut_ptr(), -1, &mut rect, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX);
    SelectObject(draw.hDC, old);
}

fn activate() {
    let Some((_, _, list)) = handles() else {
        return;
    };
    let index = unsafe { SendMessageW(list, LB_GETCURSEL, 0, 0) };
    if index < 0 {
        return;
    }
    let selected = ACTIVE.with(|s| {
        let s = s.borrow();
        let s = s.as_ref()?;
        Some((s.target?, s.matches.get(index as usize)?.clone()))
    });
    match selected {
        Some((_, SlotRef::Item(path))) => {
            close();
            let _ = crate::items::open(&path, app().controller.get());
        }
        Some((Target::Circle(ci), SlotRef::Group(gi))) => navigate(Target::Group(ci, gi)),
        Some((Target::Group(ci, _), SlotRef::Back)) => navigate(Target::Circle(ci)),
        _ => {}
    }
}
fn navigate(target: Target) {
    let base = refs(target);
    let source = candidates(target, &base);
    ACTIVE.with(|s| {
        if let Some(s) = s.borrow_mut().as_mut() {
            s.target = Some(target);
            s.base = base;
            s.source = source;
        }
    });
    if let Some((_, edit, _)) = handles() {
        unsafe {
            SetWindowTextW(edit, win::wide("").as_ptr());
            SetFocus(edit);
        }
    }
    refresh();
}
unsafe extern "system" fn input_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM, _id: usize, _data: usize) -> LRESULT {
    match msg {
        WM_KEYDOWN if w == VK_ESCAPE as usize => {
            let ring = ACTIVE.with(|s| s.borrow().as_ref().is_some_and(|s| s.target.is_none()));
            if ring {
                super::ring::set_search("");
            }
            if let Some((h, _, _)) = handles() {
                PostMessageW(h, CLOSE, 0, 0);
            }
            0
        }
        WM_KEYDOWN if w == 0x41 && GetKeyState(VK_CONTROL as i32) < 0 && GetKeyState(VK_MENU as i32) >= 0 => {
            if let Some((_, edit, _)) = handles() {
                SetFocus(edit);
                SendMessageW(edit, EM_SETSEL, 0, -1);
            }
            0
        }
        WM_KEYDOWN if w == VK_RETURN as usize => {
            if let Some((h, _, list)) = handles() {
                if list.is_null() {
                    super::ring::search_key(VK_RETURN as u32);
                } else {
                    if SendMessageW(list, LB_GETCURSEL, 0, 0) < 0 && SendMessageW(list, LB_GETCOUNT, 0, 0) > 0 {
                        SendMessageW(list, LB_SETCURSEL, 0, 0);
                    }
                    PostMessageW(h, OPEN, 0, 0);
                }
            }
            0
        }
        WM_KEYDOWN if matches!(w as u16, VK_DOWN | VK_UP) && GetKeyState(VK_SHIFT as i32) >= 0 => {
            if let Some((_, edit, list)) = handles() {
                if list.is_null() {
                    super::ring::search_key(w as u32);
                    return 0;
                }
                if hwnd == edit {
                    SetFocus(list);
                    let first = if w == VK_UP as usize { SendMessageW(list, LB_GETCOUNT, 0, 0) - 1 } else { 0 };
                    if first >= 0 {
                        SendMessageW(list, LB_SETCURSEL, first as usize, 0);
                    }
                    return 0;
                }
            }
            DefSubclassProc(hwnd, msg, w, l)
        }
        WM_KEYDOWN if w == VK_TAB as usize => {
            if let Some((_, edit, list)) = handles() {
                SetFocus(if hwnd == edit && !list.is_null() { list } else { edit });
            }
            0
        }
        WM_CHAR if matches!(w, 1 | 9 | 13 | 27) => 0,
        WM_CHAR if w >= 32 || w == 8 => {
            if let Some((_, edit, list)) = handles() {
                if hwnd == list {
                    SetFocus(edit);
                    SendMessageW(edit, WM_CHAR, w, l);
                    return 0;
                }
            }
            DefSubclassProc(hwnd, msg, w, l)
        }
        WM_NCDESTROY => {
            RemoveWindowSubclass(hwnd, Some(input_proc), 1);
            DefSubclassProc(hwnd, msg, w, l)
        }
        _ => DefSubclassProc(hwnd, msg, w, l),
    }
}
unsafe extern "system" fn proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        WM_ACTIVATE if w & 0xffff == WA_INACTIVE as usize => {
            PostMessageW(hwnd, CLOSE, 0, 0);
            0
        }
        CLOSE | WM_CLOSE => {
            if handles().is_some_and(|(h, _, _)| h == hwnd) {
                close();
            }
            0
        }
        OPEN => {
            if handles().is_some_and(|(h, _, _)| h == hwnd) {
                activate();
            }
            0
        }
        WM_MEASUREITEM => {
            let measure = &mut *(l as *mut MEASUREITEMSTRUCT);
            measure.itemHeight = win::scale(24, GetDpiForWindow(hwnd).max(96)) as u32;
            1
        }
        WM_DRAWITEM if w == 2 => {
            draw_row(&*(l as *const DRAWITEMSTRUCT));
            1
        }
        WM_COMMAND => {
            let id = w & 0xffff;
            let notification = (w >> 16) & 0xffff;
            if id == 1 && notification == EN_CHANGE as usize {
                refresh();
            } else if id == 2 && notification == LBN_DBLCLK as usize {
                PostMessageW(hwnd, OPEN, 0, 0);
            }
            0
        }
        WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX | WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
            SetTextColor(w as HDC, theme::TEXT);
            SetBkColor(w as HDC, theme::BG);
            ACTIVE.with(|s| s.borrow().as_ref().map(|s| s.brush as LRESULT).unwrap_or(0))
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let dc = BeginPaint(hwnd, &mut ps);
            let brush = ACTIVE.with(|s| s.borrow().as_ref().map(|s| s.brush));
            if let Some(brush) = brush {
                FillRect(dc, &ps.rcPaint, brush);
            }
            let mut rect = win::rect(0, 0, 0, 0);
            GetClientRect(hwnd, &mut rect);
            ACTIVE.with(|s| {
                let state = s.borrow();
                if let Some(corners) = state.as_ref().and_then(|s| s.corners.as_ref()) {
                    corners.draw(dc, rect.right, rect.bottom);
                }
            });
            EndPaint(hwnd, &ps);
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}
