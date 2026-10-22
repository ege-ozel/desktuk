//! Static UI text only. User names and file paths never pass through translation.
use std::cell::Cell;
thread_local! { static ENGLISH: Cell<bool> = const { Cell::new(false) }; }
pub fn set_english(value: bool) {
    ENGLISH.set(value);
}
pub fn english() -> bool {
    ENGLISH.get()
}
pub fn choose<'a>(tr: &'a str, en: &'a str) -> &'a str {
    if english() {
        en
    } else {
        tr
    }
}

pub fn text(source: &str) -> &str {
    let pair = match source {
        "Recent" => ("Son öğeler", "Recent"),
        "&New circle" | "New circle" => ("Yeni çember", "New circle"),
        "New &group" | "New group" => ("Yeni grup", "New group"),
        "&Rename..." | "Rename..." => ("Yeniden adlandır...", "Rename..."),
        "&Delete circle" | "Delete circle" => ("Çemberi sil", "Delete circle"),
        "Sırala" => ("Sırala", "Sort"),
        "Ada göre" | "Ada göre sırala" => ("Ada göre sırala", "Sort by name"),
        "Türe göre" | "Türe göre sırala" => ("Türe göre sırala", "Sort by type"),
        "Eklenme sırası" => ("Eklenme sırası", "Date added"),
        "Kapal\u{131}" => ("Kapalı", "Off"),
        "&Izgara" => ("Izgara", "Grid"),
        "E&xit DeskTuk" | "&Çıkış" => ("Çıkış", "Exit DeskTuk"),
        "Çemberler" => ("Çemberler", "Circles"),
        "Çember işlemleri" => ("Çember işlemleri", "Circle actions"),
        "Menüyü gizle" => ("Menüyü gizle", "Hide menu"),
        "Menüyü göster" => ("Menüyü göster", "Show menu"),
        "Kilidi çöz" => ("Kilidi çöz", "Unlock"),
        "Kilitle" => ("Kilitle", "Lock"),
        "&Masaüstü öğelerini göster" => ("Masaüstü öğelerini göster", "Show desktop icons"),
        "&Desktuk'u göster" => ("DeskTuk'u göster", "Show DeskTuk"),
        "&Windows ile başlat" => ("Windows ile başlat", "Start with Windows"),
        "Arkasına bırak" => ("Arkasına bırak", "Drop after"),
        "Önüne bırak" => ("Önüne bırak", "Drop before"),
        "Taşı / sırala" => ("Taşı / sırala", "Move / reorder"),
        "Right-click: New circle" => ("Sağ tık: Yeni çember", "Right-click: New circle"),
        "Drop files here" => ("Dosyaları buraya bırakın", "Drop files here"),
        "Aramak için yazın" => ("Aramak için yazın", "Type to search"),
        "Back" => ("Geri", "Back"),
        "&Rename group…" => ("Grubu yeniden adlandır...", "Rename group..."),
        "&Delete group" => ("Grubu sil", "Delete group"),
        "&Not now" => ("Şimdi değil", "Not now"),
        "Remove from circle" | "&Remove from circle" => ("Çemberden çıkar", "Remove from circle"),
        "&Open" => ("Aç", "Open"),
        "Boş" => ("Boş", "Empty"),
        _ => return source,
    };
    choose(pair.0, pair.1)
}
