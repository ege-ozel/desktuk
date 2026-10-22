//! Delayed, click-through contextual hints. One pending timer; no timer while visible/idle.
use crate::{app::app, circles::surface::Surface, i18n, theme, win};
use std::cell::Cell;
use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    DeleteObject, DrawTextW, GetMonitorInfoW, MonitorFromPoint, SelectObject, SetBkMode, SetTextColor, DT_LEFT, DT_WORDBREAK, MONITORINFO,
    MONITOR_DEFAULTTONEAREST, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub const TIMER: usize = 0xD48;
const CLASS: &str = "DeskTuk.LockHint";
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tip {
    Locked,
    Lock,
    Menu,
    Settings,
    Grid,
    Guide,
    Circles,
    Visibility,
    Files,
}
impl Tip {
    fn text(self) -> &'static str {
        match self {
            Self::Locked => i18n::choose(
                "Çemberler kilitli.\nKullanmak için soldaki kilidi çözün.",
                "Locked.\nUnlock using the left button to use these controls.",
            ),
            Self::Lock => i18n::choose(
                "Kilitle / kilidi çöz. Kilitliyken çemberler hover ile açılmaz; yine sürükleyebilirsiniz.",
                "Lock / unlock. Locked circles stay closed on hover; you can still drag them.",
            ),
            Self::Menu => i18n::choose(
                "Paneli aç / kapat. Boş alandan tutarak ekranın başka bir kenarına taşıyabilirsiniz.",
                "Expand / collapse the panel. Drag a blank area to move it to another screen edge.",
            ),
            Self::Settings => i18n::choose("Dil ve kullanım ipuçlarını ayarlayın.", "Choose your language and turn usage tips on or off."),
            Self::Grid => i18n::choose(
                "Izgara seçin. Yerleşimi değiştirmek için ayrıca Şimdi hizala'ya basın.",
                "Choose a grid. Use Align now separately to rearrange circles.",
            ),
            Self::Guide => i18n::choose("Kısa kullanım kılavuzunu açın.", "Open the quick guide."),
            Self::Circles => i18n::choose(
                "Çemberlerin dosya ve grup menülerini açın. Oklarla seçin, Enter ile açın.",
                "Browse circle files and groups. Select with arrow keys and press Enter to open.",
            ),
            Self::Visibility => i18n::choose(
                "Çemberleri göster / gizle. Düğmenin üzerinde bekleyin veya tıklayın.",
                "Show / hide circles. Hover briefly over the button or click it.",
            ),
            Self::Files => i18n::choose(
                "Dosya listesini açıp arayın. Yazmak filtreler; Enter veya çift tıklama dosyayı açar.",
                "Open and search files. Typing filters; Enter or double-click opens the selected file.",
            ),
        }
    }
}
#[derive(Clone, Copy)]
struct Pending {
    owner: HWND,
    target: RECT,
    tip: Tip,
}
thread_local! {
    static PENDING: Cell<Option<Pending>> = const { Cell::new(None) };
    static WINDOW: Cell<HWND> = const { Cell::new(std::ptr::null_mut()) };
}

pub fn hide() {
    if let Some(p) = PENDING.take() {
        unsafe { KillTimer(p.owner, TIMER) };
    }
    let h = WINDOW.replace(std::ptr::null_mut());
    if !h.is_null() {
        unsafe { DestroyWindow(h) };
    }
}

pub fn hover(owner: HWND, target: Option<(RECT, Tip)>) {
    let enabled = app().board.try_borrow().map(|b| b.hints).unwrap_or(false);
    let Some((target, tip)) = target.filter(|_| enabled) else {
        hide();
        return;
    };
    if PENDING.get().is_some_and(|p| {
        p.owner == owner
            && p.tip == tip
            && p.target.left == target.left
            && p.target.top == target.top
            && p.target.right == target.right
            && p.target.bottom == target.bottom
    }) {
        return;
    }
    hide();
    PENDING.set(Some(Pending { owner, target, tip }));
    if unsafe { SetTimer(owner, TIMER, 550, None) } == 0 {
        PENDING.set(None);
    }
}

pub fn fire(owner: HWND) {
    unsafe { KillTimer(owner, TIMER) };
    if !WINDOW.get().is_null() {
        return;
    }
    let Some(p) = PENDING.get().filter(|p| p.owner == owner) else { return };
    let mut cursor = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut cursor) };
    let enabled = app().board.try_borrow().map(|b| b.hints).unwrap_or(false);
    if !enabled || !win::rect_contains(&p.target, cursor) {
        hide();
        return;
    }
    let dpi = unsafe { GetDpiForWindow(owner) }.max(96);
    let sc = |v| win::scale(v, dpi);
    let w = sc(280);
    let h = sc(88);
    let center = POINT { x: (p.target.left + p.target.right) / 2, y: (p.target.top + p.target.bottom) / 2 };
    let mut mi: MONITORINFO = unsafe { std::mem::zeroed() };
    mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    if unsafe { GetMonitorInfoW(MonitorFromPoint(center, MONITOR_DEFAULTTONEAREST), &mut mi) } == 0 {
        hide();
        return;
    }
    // Keep the whole bubble outside the dock, even for its inset centre tile.
    let panel = win::window_rect(owner);
    let left_of_target = panel.left - w - sc(4) >= mi.rcWork.left;
    let x = if left_of_target { panel.left - w - sc(4) } else { panel.right + sc(4) }
        .clamp(mi.rcWork.left, (mi.rcWork.right - w).max(mi.rcWork.left));
    let y = (center.y - h / 2).clamp(mi.rcWork.top, (mi.rcWork.bottom - h).max(mi.rcWork.top));
    let Some(mut surface) = Surface::new(w, h) else { return };
    let arrow = sc(9) as f32;
    let radius = sc(8) as f32;
    let stroke = sc(1).max(1) as f32;
    let (box_left, box_right) = if left_of_target { (0.0, w as f32 - arrow) } else { (arrow, w as f32) };
    let arrow_y = (center.y - y).clamp(sc(16), h - sc(16)) as f32;
    let pixels = surface.pixels();
    let bg = theme::BG;
    let border = if p.tip == Tip::Locked { win::rgb(185, 99, 111) } else { theme::MINT };
    for py in 0..h {
        for px in 0..w {
            let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
            let cx = (box_left + box_right) / 2.0;
            let qx = (fx - cx).abs() - ((box_right - box_left) / 2.0 - radius);
            let qy = (fy - h as f32 / 2.0).abs() - (h as f32 / 2.0 - radius);
            let distance = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - radius;
            let body_alpha = (0.5 - distance).clamp(0.0, 1.0);
            let body_edge = (distance + stroke + 0.5).clamp(0.0, 1.0);
            let ax = if left_of_target { fx - box_right } else { box_left - fx };
            let triangle = ax >= -stroke && ax <= arrow && (fy - arrow_y).abs() <= arrow - ax;
            let a = if triangle { 1.0 } else { body_alpha };
            let edge = if triangle { f32::from(ax >= arrow - stroke || (fy - arrow_y).abs() >= arrow - ax - stroke) } else { body_edge };
            let channel =
                |shift: u32| ((((bg >> shift) & 255) as f32 * (1.0 - edge) + ((border >> shift) & 255) as f32 * edge) * a).round() as u32;
            pixels[(py * w + px) as usize] = ((a * 255.0).round() as u32) << 24 | channel(0) << 16 | channel(8) << 8 | channel(16);
        }
    }
    let mut text_rect = win::rect(box_left as i32 + sc(12), sc(10), box_right as i32 - sc(12), h - sc(10));
    let restore_rect = text_rect;
    let font = theme::font(dpi, false, 100);
    unsafe {
        let old = SelectObject(surface.dc(), font);
        SetBkMode(surface.dc(), TRANSPARENT as i32);
        SetTextColor(surface.dc(), theme::TEXT);
        let mut text = win::wide(p.tip.text());
        DrawTextW(surface.dc(), text.as_mut_ptr(), -1, &mut text_rect, DT_LEFT | DT_WORDBREAK);
        SelectObject(surface.dc(), old);
        DeleteObject(font);
    }
    // GDI clears text alpha; the text rectangle is fully within the opaque body.
    let pixels = surface.pixels();
    for row in restore_rect.top..restore_rect.bottom {
        for col in restore_rect.left..restore_rect.right {
            pixels[(row * w + col) as usize] |= 0xff00_0000;
        }
    }
    unsafe {
        let name = win::wide(CLASS);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(DefWindowProcW),
            hInstance: GetModuleHandleW(std::ptr::null()),
            lpszClassName: name.as_ptr(),
            ..std::mem::zeroed()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT,
            name.as_ptr(),
            std::ptr::null(),
            WS_POPUP,
            x,
            y,
            w,
            h,
            owner,
            std::ptr::null_mut(),
            wc.hInstance,
            std::ptr::null(),
        );
        if !hwnd.is_null() {
            WINDOW.set(hwnd);
            surface.present(hwnd, POINT { x, y });
            SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW);
        }
    }
}
