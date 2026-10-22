//! On-demand access to Explorer's desktop view. Never retain a COM interface or query
//! it at startup: closed circles keep their small idle footprint. Call outside borrows.

use windows::core::{Interface, Result};
use windows::Win32::System::Com::{CoCreateInstance, IServiceProvider, CLSCTX_LOCAL_SERVER};
use windows::Win32::System::Variant::{VARIANT, VT_I4};
use windows::Win32::UI::Shell::{
    IFolderView2, IShellBrowser, IShellWindows, SID_STopLevelBrowser, ShellWindows, CSIDL_DESKTOP, FOLDERVIEWMODE, FWF_NOICONS,
    SWC_DESKTOP, SWFO_NEEDDISPATCH,
};

fn folder_view() -> Result<IFolderView2> {
    unsafe {
        let windows: IShellWindows = CoCreateInstance(&ShellWindows, None, CLSCTX_LOCAL_SERVER)?;
        // VT_EMPTY owns no resources. Avoid importing VariantClear just for empty
        // input values (and loading oleaut32 before a ring is ever opened).
        let empty = std::mem::ManuallyDrop::new(VARIANT::default());
        let mut location = VARIANT::default();
        (*location.Anonymous.Anonymous).vt = VT_I4;
        (*location.Anonymous.Anonymous).Anonymous.lVal = CSIDL_DESKTOP as i32;
        let mut hwnd = 0;
        let desktop = windows.FindWindowSW(&location, &*empty, SWC_DESKTOP, &mut hwnd, SWFO_NEEDDISPATCH)?;
        let services: IServiceProvider = desktop.cast()?;
        let browser: IShellBrowser = services.QueryService(&SID_STopLevelBrowser)?;
        browser.QueryActiveShellView()?.cast()
    }
}

/// Explorer reports physical pixels. Do not DPI-scale this value a second time.
pub fn icon_size() -> Result<i32> {
    let view = folder_view()?;
    let mut mode = FOLDERVIEWMODE::default();
    let mut px = 0;
    unsafe { view.GetViewModeAndIconSize(&mut mode, &mut px)? };
    if !(16..=512).contains(&px) {
        return Err(windows::core::Error::from_hresult(windows::core::HRESULT(0x80070057u32 as i32)));
    }
    Ok(px)
}

// Above the shell's reserved command range; also used in file context-menu extras.
pub const MENU_ID: usize = 0x8020;
pub const MENU_LABEL: &str = "&Masaüstü öğelerini göster";

/// Read the real view whenever a menu opens; do not keep a second preference in
/// DeskTuk's config. Query outside RefCell borrows, including those of the ring.
pub fn menu_flags() -> u32 {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MF_CHECKED, MF_GRAYED, MF_STRING};
    let result = folder_view().and_then(|view| unsafe { view.GetCurrentFolderFlags() });
    match result {
        Ok(flags) if flags & FWF_NOICONS.0 as u32 == 0 => MF_STRING | MF_CHECKED,
        Ok(_) => MF_STRING,
        Err(e) => {
            crate::dlog!("desktop visibility unavailable: {e}");
            MF_STRING | MF_GRAYED
        }
    }
}

fn toggle_visibility() -> Result<()> {
    let view = folder_view()?;
    let mask = FWF_NOICONS.0 as u32;
    unsafe {
        let flags = view.GetCurrentFolderFlags()?;
        let hide = flags & mask == 0;
        view.SetCurrentFolderFlags(mask, if hide { mask } else { 0 })?;
        if (view.GetCurrentFolderFlags()? & mask != 0) != hide {
            return Err(windows::core::Error::from_hresult(windows::core::HRESULT(0x80070005u32 as i32)));
        }
        // Keep Explorer's own Show desktop icons preference consistent and persistent.
        // Mask only fHideIcons; never change unrelated shell settings or file attributes.
        use windows_sys::Win32::UI::Shell::{SHGetSetSettings, SHELLSTATEA, SSF_HIDEICONS};
        let mut state: SHELLSTATEA = std::mem::zeroed();
        SHGetSetSettings(&mut state, SSF_HIDEICONS, 0);
        state._bitfield1 = (state._bitfield1 & !(1 << 12)) | (i32::from(hide) << 12);
        SHGetSetSettings(&mut state, SSF_HIDEICONS, 1);
        crate::dlog!("desktop items visible={}", !hide);
    }
    Ok(())
}

/// Only invoked by an explicit menu selection, never during startup or shutdown.
pub fn toggle_from_menu() {
    if let Err(e) = toggle_visibility() {
        crate::dlog!("desktop visibility change failed: {e}");
        unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
            MessageBoxW(
                crate::app::app().controller.get(),
                crate::win::wide(crate::i18n::choose(
                    "Masaüstü öğelerinin görünürlüğü değiştirilemedi. Windows masaüstü kullanılabilir olduğunda tekrar deneyin.",
                    "Could not change desktop icon visibility. Try again when the Windows desktop is available.",
                ))
                .as_ptr(),
                crate::win::wide("DeskTuk").as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
    }
}
