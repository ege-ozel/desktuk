//! On-demand native settings menu and short bilingual guide. No retained dialog.
use crate::{app::app, i18n, win};
use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub const MENU_ID: usize = 70;
pub const HELP_ID: usize = 71;

pub fn label() -> &'static str {
    i18n::choose("Ayarlar", "Settings")
}
pub fn help_label() -> &'static str {
    i18n::choose("Kullanım kılavuzu", "Quick guide")
}

pub fn show(owner: HWND, at: POINT) {
    crate::hint::hide();
    let (en, hints) = app().board.try_borrow().map(|b| (b.english, b.hints)).unwrap_or((false, true));
    let command = unsafe {
        let menu = CreatePopupMenu();
        let languages = CreatePopupMenu();
        if menu.is_null() || languages.is_null() {
            if !menu.is_null() {
                DestroyMenu(menu);
            }
            if !languages.is_null() {
                DestroyMenu(languages);
            }
            return;
        }
        AppendMenuW(languages, MF_STRING | if !en { MF_CHECKED } else { 0 }, 1, win::wide("Türkçe").as_ptr());
        AppendMenuW(languages, MF_STRING | if en { MF_CHECKED } else { 0 }, 2, win::wide("English").as_ptr());
        if AppendMenuW(menu, MF_POPUP, languages as usize, win::wide("Dil / Language").as_ptr()) == 0 {
            DestroyMenu(languages);
        }
        AppendMenuW(
            menu,
            MF_STRING | if hints { MF_CHECKED } else { 0 },
            3,
            win::wide(i18n::choose("Kullanım ipuçları", "Usage tips")).as_ptr(),
        );
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(menu, MF_STRING, 4, win::wide(help_label()).as_ptr());
        SetForegroundWindow(owner);
        let command = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, at.x, at.y, 0, owner, std::ptr::null());
        PostMessageW(owner, WM_NULL, 0, 0);
        DestroyMenu(menu);
        command
    };
    if command == 4 {
        guide(owner);
        return;
    }
    if !(1..=3).contains(&command) {
        return;
    }
    if let Ok(mut board) = app().board.try_borrow_mut() {
        match command {
            1 => board.english = false,
            2 => board.english = true,
            3 => board.hints = !board.hints,
            _ => {}
        }
        i18n::set_english(board.english);
    }
    app().save_config();
    crate::circles::refresh_all();
    crate::dock::refresh();
}

pub fn guide(owner: HWND) {
    crate::hint::hide();
    let text = i18n::choose(
        "Çemberin üzerine gelerek içeriğini açın.\n\n• Taşıma: merkezden veya boş alandan sol tuşla sürükleyin.\n• Dosya açma: simgeye tıklayın. Dosyayı başka çembere sürüklemek yalnızca başvurusunu taşır.\n• Arama: açık çemberin üzerinde yazın. Başlangıcı eşleşen adlar önce gelir. Backspace siler, Esc temizler.\n• Sayfalama: tekerleği kaydırın veya merkeze tıklayın.\n• Kilit: otomatik açılmayı ve panel eylemlerini durdurur; çemberler taşınabilir.\n• Izgara: seçim sonraki bırakmaları hizalar. Şimdi hizala mevcut çemberleri uygun boş hücrelere yerleştirir.\n• Panelin sağ tık menüsünden Çemberler ile dosyalara ulaşabilirsiniz.\n• Dil ve ipuçları Ayarlar menüsündedir.",
        "Hover over a circle to reveal its contents.\n\n• Move: drag its centre or empty background with the left button.\n• Open: click a file icon. Dragging a file to another circle moves only its reference.\n• Search: type over an open circle. Prefix matches appear first. Backspace deletes; Esc clears.\n• Pages: scroll the wheel or click the centre.\n• Lock: stops hover opening and panel actions; circles can still be moved.\n• Grid: selection affects future drops. Align now places existing circles in suitable empty cells.\n• Right-click the panel and choose Circles to launch files.\n• Language and usage tips are in Settings.");
    unsafe { MessageBoxW(owner, win::wide(text).as_ptr(), win::wide(help_label()).as_ptr(), MB_OK | MB_ICONINFORMATION) };
}
