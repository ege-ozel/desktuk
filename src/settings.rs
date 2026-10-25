//! DeskTuk's compact, custom-painted Settings and Quick Guide panel.

use std::cell::Cell;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreatePen, CreateRoundRectRgn, CreateSolidBrush, DeleteObject, DrawTextW, Ellipse, EndPaint, FillRect, GetMonitorInfoW,
    InvalidateRect, MonitorFromPoint, RoundRect, ScreenToClient, SelectObject, SetBkMode, SetTextColor, SetWindowRgn, DT_CENTER, DT_LEFT,
    DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, MONITORINFO, MONITOR_DEFAULTTONEAREST, PAINTSTRUCT, PS_SOLID, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::{app::app, i18n, win};

pub const MENU_ID: usize = 70;
pub const HELP_ID: usize = 71;

const CLASS: &str = "DeskTuk.SettingsPanel";
const SETTINGS: u8 = 0;
const GUIDE: u8 = 1;

thread_local! {
    static PANEL: Cell<HWND> = const { Cell::new(std::ptr::null_mut()) };
    static PAGE: Cell<u8> = const { Cell::new(SETTINGS) };
    static STARTUP_ENABLED: Cell<Option<bool>> = const { Cell::new(None) };
    static CLASS_READY: Cell<bool> = const { Cell::new(false) };
}

pub fn label() -> &'static str {
    i18n::choose("Ayarlar", "Settings")
}

pub fn help_label() -> &'static str {
    i18n::choose("Hızlı kılavuz", "Quick guide")
}

pub fn show(owner: HWND, at: POINT) {
    crate::hint::hide();
    open(owner, at, SETTINGS);
}

pub fn guide(owner: HWND) {
    crate::hint::hide();
    let mut at = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut at) };
    open(owner, at, GUIDE);
}

fn open(owner: HWND, at: POINT, page: u8) {
    let startup = crate::autostart::is_enabled().ok();
    STARTUP_ENABLED.set(startup);
    PAGE.set(page);
    register_class();
    if !PANEL.get().is_null() && unsafe { IsWindow(PANEL.get()) } != 0 {
        let hwnd = PANEL.get();
        let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
        let (w, h) = size(page, dpi);
        let at = reposition_for_size(hwnd, w, h);
        unsafe {
            SetWindowPos(hwnd, HWND_TOPMOST, at.x, at.y, w, h, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            shape(hwnd, dpi, w, h);
            InvalidateRect(hwnd, std::ptr::null(), 1);
            ShowWindow(hwnd, SW_SHOWNORMAL);
            SetForegroundWindow(hwnd);
        }
        return;
    }

    let dpi = unsafe { GetDpiForWindow(owner) }.max(96);
    let (w, h) = size(page, dpi);
    let (x, y) = clamp_to_work_area(at, w, h);
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            win::wide(CLASS).as_ptr(),
            win::wide("DeskTuk").as_ptr(),
            WS_POPUP,
            x,
            y,
            w,
            h,
            owner,
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        crate::dlog!("Settings panel window creation failed: {}", unsafe { windows_sys::Win32::Foundation::GetLastError() });
        return;
    }
    PANEL.set(hwnd);
    shape(hwnd, dpi, w, h);
    unsafe {
        crate::host::exclude_from_peek(hwnd);
        ShowWindow(hwnd, SW_SHOWNORMAL);
        SetForegroundWindow(hwnd);
        InvalidateRect(hwnd, std::ptr::null(), 1);
    }
}

fn register_class() {
    if CLASS_READY.get() {
        return;
    }
    let name = win::wide(CLASS);
    let class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(panel_proc),
        hInstance: unsafe { GetModuleHandleW(std::ptr::null()) },
        hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) },
        lpszClassName: name.as_ptr(),
        ..unsafe { std::mem::zeroed() }
    };
    if unsafe { RegisterClassExW(&class) } != 0 {
        CLASS_READY.set(true);
    }
}

fn size(page: u8, dpi: u32) -> (i32, i32) {
    if page == GUIDE {
        (scale(540, dpi), scale(710, dpi))
    } else {
        (scale(380, dpi), scale(480, dpi))
    }
}

fn scale(value: i32, dpi: u32) -> i32 {
    (value * dpi as i32 + 48) / 96
}

fn clamp_to_work_area(at: POINT, width: i32, height: i32) -> (i32, i32) {
    unsafe {
        let monitor = MonitorFromPoint(at, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) != 0 {
            let work = info.rcWork;
            return (
                at.x.clamp(work.left, (work.right - width).max(work.left)),
                at.y.clamp(work.top, (work.bottom - height).max(work.top)),
            );
        }
    }
    (at.x, at.y)
}

fn reposition_for_size(hwnd: HWND, width: i32, height: i32) -> POINT {
    let rect = win::window_rect(hwnd);
    let (x, y) = clamp_to_work_area(POINT { x: rect.left, y: rect.top }, width, height);
    POINT { x, y }
}

fn shape(hwnd: HWND, dpi: u32, width: i32, height: i32) {
    let radius = scale(16, dpi);
    let region = unsafe { CreateRoundRectRgn(0, 0, width + 1, height + 1, radius, radius) };
    if !region.is_null() && unsafe { SetWindowRgn(hwnd, region, 1) } == 0 {
        unsafe { DeleteObject(region) };
    }
}

fn settings_state() -> (bool, bool) {
    app().board.try_borrow().map(|b| (b.english, b.hints)).unwrap_or((i18n::english(), true))
}

fn text_scale() -> i32 {
    app().board.try_borrow().map(|b| i32::from(b.menu_text_scale)).unwrap_or(90)
}

#[allow(clippy::too_many_arguments)] // GDI text needs independent geometry, font and color attributes.
fn color_text(
    dc: windows_sys::Win32::Graphics::Gdi::HDC,
    text: &str,
    mut rect: RECT,
    dpi: u32,
    color: u32,
    bold: bool,
    percent: i32,
    flags: u32,
) {
    let font = crate::theme::font(dpi, bold, percent * text_scale() / 90);
    let old_font = unsafe { SelectObject(dc, font) };
    let mut text = win::wide(text);
    unsafe {
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, color);
        DrawTextW(dc, text.as_mut_ptr(), -1, &mut rect, flags | DT_NOPREFIX);
        if !old_font.is_null() {
            SelectObject(dc, old_font);
        }
        if !font.is_null() {
            DeleteObject(font);
        }
    }
}

fn rounded(dc: windows_sys::Win32::Graphics::Gdi::HDC, rect: RECT, dpi: u32, fill: u32, border: u32) {
    let brush = unsafe { CreateSolidBrush(fill) };
    let pen = unsafe { CreatePen(PS_SOLID, scale(1, dpi).max(1), border) };
    if brush.is_null() || pen.is_null() {
        if !brush.is_null() {
            unsafe {
                DeleteObject(brush);
            }
        }
        if !pen.is_null() {
            unsafe {
                DeleteObject(pen);
            }
        }
        return;
    }
    let old_brush = unsafe { SelectObject(dc, brush) };
    let old_pen = unsafe { SelectObject(dc, pen) };
    unsafe {
        RoundRect(dc, rect.left, rect.top, rect.right, rect.bottom, scale(12, dpi), scale(12, dpi));
        SelectObject(dc, old_brush);
        SelectObject(dc, old_pen);
        DeleteObject(brush);
        DeleteObject(pen);
    }
}

fn text_rect(x: i32, y: i32, w: i32, h: i32, dpi: u32) -> RECT {
    win::rect(scale(x, dpi), scale(y, dpi), scale(x + w, dpi), scale(y + h, dpi))
}

fn draw_toggle(dc: windows_sys::Win32::Graphics::Gdi::HDC, dpi: u32, x: i32, y: i32, on: bool) {
    let track = text_rect(x, y, 44, 26, dpi);
    rounded(dc, track, dpi, if on { crate::theme::MINT } else { crate::theme::BORDER }, crate::theme::BORDER);
    let side = scale(18, dpi);
    let inset = scale(4, dpi);
    let left = if on { track.right - side - inset } else { track.left + inset };
    let top = track.top + (track.bottom - track.top - side) / 2;
    let brush = unsafe { CreateSolidBrush(crate::theme::TEXT) };
    let old = unsafe { SelectObject(dc, brush) };
    unsafe {
        Ellipse(dc, left, top, left + side, top + side);
        SelectObject(dc, old);
        if !brush.is_null() {
            DeleteObject(brush);
        }
    }
}

fn paint_settings(dc: windows_sys::Win32::Graphics::Gdi::HDC, dpi: u32) {
    let (english, hints) = settings_state();
    let startup = STARTUP_ENABLED.get();
    let t = |tr: &'static str, en: &'static str| if english { en } else { tr };
    color_text(dc, "DeskTuk", text_rect(22, 18, 250, 34, dpi), dpi, crate::theme::TEXT, true, 145, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
    color_text(
        dc,
        t("AYARLAR", "SETTINGS"),
        text_rect(24, 51, 240, 18, dpi),
        dpi,
        crate::theme::MUTED,
        true,
        92,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE,
    );
    color_text(dc, "×", text_rect(334, 17, 28, 28, dpi), dpi, crate::theme::MUTED, true, 140, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

    rounded(dc, text_rect(18, 78, 344, 84, dpi), dpi, crate::theme::SURFACE, crate::theme::BORDER);
    color_text(
        dc,
        t("Dil / Language", "Language / Dil"),
        text_rect(32, 87, 310, 20, dpi),
        dpi,
        crate::theme::MUTED,
        true,
        95,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE,
    );
    let tr = text_rect(30, 115, 151, 34, dpi);
    let en = text_rect(195, 115, 151, 34, dpi);
    rounded(
        dc,
        tr,
        dpi,
        if !english { crate::theme::RAISED } else { crate::theme::SURFACE },
        if !english { crate::theme::ACCENT } else { crate::theme::BORDER },
    );
    rounded(
        dc,
        en,
        dpi,
        if english { crate::theme::RAISED } else { crate::theme::SURFACE },
        if english { crate::theme::ACCENT } else { crate::theme::BORDER },
    );
    color_text(dc, "Türkçe", tr, dpi, crate::theme::TEXT, !english, 100, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    color_text(dc, "English", en, dpi, crate::theme::TEXT, english, 100, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

    rounded(dc, text_rect(18, 174, 344, 68, dpi), dpi, crate::theme::SURFACE, crate::theme::BORDER);
    color_text(
        dc,
        t("Kullanım ipuçları", "Usage tips"),
        text_rect(32, 183, 256, 24, dpi),
        dpi,
        crate::theme::TEXT,
        true,
        100,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE,
    );
    color_text(
        dc,
        t("Çemberlerde kısa yönlendirmeler", "Small hints around the circles"),
        text_rect(32, 207, 286, 20, dpi),
        dpi,
        crate::theme::MUTED,
        false,
        90,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE,
    );
    draw_toggle(dc, dpi, 304, 195, hints);

    rounded(dc, text_rect(18, 252, 344, 68, dpi), dpi, crate::theme::SURFACE, crate::theme::BORDER);
    color_text(
        dc,
        t("Windows ile başlat", "Start with Windows"),
        text_rect(32, 261, 256, 24, dpi),
        dpi,
        crate::theme::TEXT,
        true,
        100,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE,
    );
    color_text(
        dc,
        t("Oturum açınca DeskTuk'u aç", "Open DeskTuk when you sign in"),
        text_rect(32, 285, 270, 20, dpi),
        dpi,
        crate::theme::MUTED,
        false,
        90,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE,
    );
    if let Some(enabled) = startup {
        draw_toggle(dc, dpi, 304, 273, enabled);
    } else {
        color_text(
            dc,
            t("Kullanılamıyor", "Unavailable"),
            text_rect(286, 273, 62, 26, dpi),
            dpi,
            crate::theme::MUTED,
            false,
            82,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
    }

    rounded(dc, text_rect(18, 330, 344, 68, dpi), dpi, crate::theme::SURFACE, crate::theme::BORDER);
    color_text(
        dc,
        t("Yazı boyutu", "Text size"),
        text_rect(32, 333, 115, 25, dpi),
        dpi,
        crate::theme::TEXT,
        true,
        100,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE,
    );
    let scale_value = text_scale();
    for (index, (x, label)) in [(152, "A"), (218, "A"), (284, "A")].into_iter().enumerate() {
        let value = [90, 100, 110][index];
        let rect = text_rect(x, 337, 56, 42, dpi);
        rounded(
            dc,
            rect,
            dpi,
            if scale_value == value { crate::theme::RAISED } else { crate::theme::SURFACE },
            if scale_value == value { crate::theme::ACCENT } else { crate::theme::BORDER },
        );
        color_text(
            dc,
            label,
            rect,
            dpi,
            if scale_value == value { crate::theme::MINT } else { crate::theme::TEXT },
            true,
            80 + index as i32 * 20,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
    }
    color_text(
        dc,
        t("Küçük     Varsayılan     Büyük", "Small     Default     Large"),
        text_rect(148, 380, 203, 16, dpi),
        dpi,
        crate::theme::MUTED,
        false,
        76,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );

    rounded(dc, text_rect(18, 400, 344, 54, dpi), dpi, crate::theme::RAISED, crate::theme::BORDER);
    color_text(
        dc,
        t("Hızlı kılavuzu aç", "Open the quick guide"),
        text_rect(32, 410, 280, 30, dpi),
        dpi,
        crate::theme::ACCENT,
        true,
        105,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE,
    );
    color_text(dc, "›", text_rect(328, 407, 22, 34, dpi), dpi, crate::theme::MINT, true, 150, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    color_text(
        dc,
        t("DeskTuk görünümüne uygun ayarlar", "Settings styled for DeskTuk"),
        text_rect(24, 458, 330, 18, dpi),
        dpi,
        crate::theme::MUTED,
        false,
        85,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
}

fn guide_topics(english: bool) -> [(&'static str, &'static str); 9] {
    if english {
        [
            ("Circles", "Hover over a circle to reveal its contents."),
            ("Move", "Drag a centre or blank area with the left button."),
            ("Files", "Click to open. Dragging to another circle moves its reference."),
            ("Drag out", "Hold Ctrl while dragging to another app. The source stays put."),
            ("Search", "Type over an open circle; name prefixes appear first."),
            ("Pages", "Scroll the wheel or click the centre to change pages."),
            ("Lock", "Stops hover opening and show/hide. Panel search still works."),
            ("Grid", "Choose a grid for future drops or align existing circles."),
            ("Dock", "Search a circle row, open files, or drag the panel along an edge."),
        ]
    } else {
        [
            ("Çemberler", "İçeriğini görmek için çemberin üzerine gelin."),
            ("Taşıma", "Merkezden veya boş alandan sol tuşla sürükleyin."),
            ("Dosyalar", "Açmak için tıklayın; başka çembere sürüklemek başvuruyu taşır."),
            ("Dışarı sürükleme", "Başka uygulamaya sürüklerken Ctrl basılı tutun; kaynak kalır."),
            ("Arama", "Açık çemberin üzerinde yazın; ad başlangıçları önce eşleşir."),
            ("Sayfalar", "Sayfayı değiştirmek için tekerleği veya merkezi kullanın."),
            ("Kilit", "Üzerine gelince açılmayı ve göster/gizleyi durdurur."),
            ("Izgara", "Yeni bırakmalar için ızgara seçin veya çemberleri hizalayın."),
            ("Panel", "Çember satırında arayın, dosya açın veya paneli kenardan sürükleyin."),
        ]
    }
}

fn paint_guide(dc: windows_sys::Win32::Graphics::Gdi::HDC, dpi: u32) {
    let english = i18n::english();
    let t = |tr: &'static str, en: &'static str| if english { en } else { tr };
    color_text(
        dc,
        t("Hızlı kılavuz", "Quick guide"),
        text_rect(22, 18, 430, 34, dpi),
        dpi,
        crate::theme::TEXT,
        true,
        140,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE,
    );
    color_text(
        dc,
        t("DeskTuk'u kullanmanın kısa yolları", "A few ways to use DeskTuk"),
        text_rect(24, 51, 470, 20, dpi),
        dpi,
        crate::theme::MUTED,
        false,
        95,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE,
    );
    color_text(dc, "×", text_rect(492, 17, 28, 28, dpi), dpi, crate::theme::MUTED, true, 140, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    for (index, (heading, body)) in guide_topics(english).into_iter().enumerate() {
        let y = 82 + index as i32 * 62;
        rounded(dc, text_rect(18, y, 504, 56, dpi), dpi, crate::theme::SURFACE, crate::theme::BORDER);
        color_text(
            dc,
            heading,
            text_rect(30, y + 5, 480, 21, dpi),
            dpi,
            crate::theme::MINT,
            true,
            96,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE,
        );
        color_text(dc, body, text_rect(30, y + 27, 480, 23, dpi), dpi, crate::theme::TEXT, false, 91, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
    }
    rounded(dc, text_rect(18, 652, 504, 40, dpi), dpi, crate::theme::RAISED, crate::theme::BORDER);
    color_text(
        dc,
        t("← Ayarlara dön", "← Back to settings"),
        text_rect(30, 656, 480, 31, dpi),
        dpi,
        crate::theme::ACCENT,
        true,
        100,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
}

unsafe extern "system" fn panel_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut paint: PAINTSTRUCT = std::mem::zeroed();
            let dc = BeginPaint(hwnd, &mut paint);
            let mut client: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut client);
            let background = CreateSolidBrush(crate::theme::BG);
            FillRect(dc, &client, background);
            DeleteObject(background);
            let dpi = GetDpiForWindow(hwnd).max(96);
            if PAGE.get() == GUIDE {
                paint_guide(dc, dpi);
            } else {
                paint_settings(dc, dpi);
            }
            EndPaint(hwnd, &paint);
            0
        }
        WM_LBUTTONUP => {
            click(hwnd, win::x_lparam(lparam), win::y_lparam(lparam));
            0
        }
        WM_KEYDOWN if wparam == windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE as usize => {
            DestroyWindow(hwnd);
            0
        }
        WM_ACTIVATE if (wparam & 0xffff) as u32 == WA_INACTIVE => {
            DestroyWindow(hwnd);
            0
        }
        WM_NCHITTEST => {
            let mut point = POINT { x: win::x_lparam(lparam), y: win::y_lparam(lparam) };
            ScreenToClient(hwnd, &mut point);
            let dpi = GetDpiForWindow(hwnd).max(96);
            let panel_width = if PAGE.get() == GUIDE { 540 } else { 380 };
            if point.y < scale(62, dpi) && point.x < scale(panel_width - 52, dpi) {
                HTCAPTION as LRESULT
            } else {
                HTCLIENT as LRESULT
            }
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            if PANEL.get() == hwnd {
                PANEL.set(std::ptr::null_mut());
            }
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

fn click(hwnd: HWND, x: i32, y: i32) {
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let x = x * 96 / dpi as i32;
    let y = y * 96 / dpi as i32;
    if x >= 330 && y <= 48 {
        unsafe { DestroyWindow(hwnd) };
        return;
    }
    if PAGE.get() == GUIDE {
        if (652..=700).contains(&y) {
            PAGE.set(SETTINGS);
            let (w, h) = size(SETTINGS, dpi);
            let at = reposition_for_size(hwnd, w, h);
            unsafe {
                SetWindowPos(hwnd, HWND_TOPMOST, at.x, at.y, w, h, SWP_NOACTIVATE);
                shape(hwnd, dpi, w, h);
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
        }
        return;
    }
    if (115..=150).contains(&y) {
        if (30..=181).contains(&x) {
            set_language(false);
        } else if (195..=346).contains(&x) {
            set_language(true);
        }
    } else if (174..=242).contains(&y) {
        if let Ok(mut board) = app().board.try_borrow_mut() {
            board.hints = !board.hints;
        }
        app().save_config();
        crate::circles::refresh_all();
        crate::dock::refresh();
    } else if (252..=320).contains(&y) {
        if let Some(enabled) = STARTUP_ENABLED.get() {
            match crate::autostart::set_enabled(!enabled) {
                Ok(()) => STARTUP_ENABLED.set(Some(!enabled)),
                Err(error) => {
                    crate::dlog!("startup change failed in Settings panel: {error}");
                    STARTUP_ENABLED.set(None);
                }
            }
        }
    } else if (337..=379).contains(&y) {
        let value = match x {
            152..=207 => Some(90),
            218..=273 => Some(100),
            284..=340 => Some(110),
            _ => None,
        };
        if let Some(value) = value {
            if let Ok(mut board) = app().board.try_borrow_mut() {
                board.menu_text_scale = value;
            }
            app().save_config();
        }
    } else if (400..=454).contains(&y) {
        PAGE.set(GUIDE);
        let (w, h) = size(GUIDE, dpi);
        let at = reposition_for_size(hwnd, w, h);
        unsafe {
            SetWindowPos(hwnd, HWND_TOPMOST, at.x, at.y, w, h, SWP_NOACTIVATE);
            shape(hwnd, dpi, w, h);
            InvalidateRect(hwnd, std::ptr::null(), 1);
        }
    }
    unsafe { InvalidateRect(hwnd, std::ptr::null(), 1) };
}

fn set_language(english: bool) {
    if let Ok(mut board) = app().board.try_borrow_mut() {
        board.english = english;
        i18n::set_english(english);
    }
    app().save_config();
    crate::circles::refresh_all();
    crate::dock::refresh();
}
