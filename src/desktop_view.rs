//! On-demand access to Explorer's desktop view. Never retain a COM interface or query
//! it at startup: closed circles keep their small idle footprint. Call outside borrows.

use windows::core::{Interface, Result};
use windows::Win32::System::Com::{CoCreateInstance, IServiceProvider, CLSCTX_LOCAL_SERVER};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Shell::{
    IFolderView2, IShellBrowser, IShellWindows, SID_STopLevelBrowser, ShellWindows, FOLDERVIEWMODE, SWC_DESKTOP, SWFO_NEEDDISPATCH,
};

fn folder_view() -> Result<IFolderView2> {
    unsafe {
        let windows: IShellWindows = CoCreateInstance(&ShellWindows, None, CLSCTX_LOCAL_SERVER)?;
        // VT_EMPTY owns no resources. Avoid importing VariantClear just for empty
        // input values (and loading oleaut32 before a ring is ever opened).
        let empty = std::mem::ManuallyDrop::new(VARIANT::default());
        let mut hwnd = 0;
        let desktop = windows.FindWindowSW(&*empty, &*empty, SWC_DESKTOP, &mut hwnd, SWFO_NEEDDISPATCH)?;
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
