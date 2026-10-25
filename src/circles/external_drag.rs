//! OLE file drag source for exporting an existing circle reference to other apps.

use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};

use windows::core::{Interface, GUID, PCWSTR};
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::System::Com::IDataObject;
use windows::Win32::System::Ole::{OleInitialize, OleUninitialize, DROPEFFECT_COPY};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{ILClone, ILFindChild, ILFree, ILRemoveLastID, SHCreateDataObject, SHParseDisplayName};
use windows_sys::Win32::Foundation::HWND;

const DRAGDROP_S_DROP: i32 = 0x0004_0100;
const DRAGDROP_S_CANCEL: i32 = 0x0004_0101;
const DRAGDROP_S_USEDEFAULTCURSORS: i32 = 0x0004_0102;
const IDROP_SOURCE: GUID = GUID::from_u128(0x0000012100000000C000000000000046);
const IUNKNOWN: GUID = GUID::from_u128(0x0000000000000000C000000000000046);

#[repr(C)]
struct DropSourceVtable {
    query_interface: unsafe extern "system" fn(*mut DropSource, *const GUID, *mut *mut std::ffi::c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut DropSource) -> u32,
    release: unsafe extern "system" fn(*mut DropSource) -> u32,
    query_continue_drag: unsafe extern "system" fn(*mut DropSource, i32, u32) -> i32,
    give_feedback: unsafe extern "system" fn(*mut DropSource, u32) -> i32,
}

#[repr(C)]
struct DropSource {
    vtable: *const DropSourceVtable,
    references: AtomicU32,
}

static DROP_SOURCE_VTABLE: DropSourceVtable = DropSourceVtable { query_interface, add_ref, release, query_continue_drag, give_feedback };

unsafe extern "system" fn query_interface(this: *mut DropSource, iid: *const GUID, out: *mut *mut std::ffi::c_void) -> i32 {
    if iid.is_null() || out.is_null() {
        return 0x8000_4003u32 as i32; // E_POINTER
    }
    if *iid == IUNKNOWN || *iid == IDROP_SOURCE {
        *out = this.cast();
        add_ref(this);
        0
    } else {
        *out = std::ptr::null_mut();
        0x8000_4002u32 as i32 // E_NOINTERFACE
    }
}

unsafe extern "system" fn add_ref(this: *mut DropSource) -> u32 {
    (*this).references.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn release(this: *mut DropSource) -> u32 {
    (*this).references.fetch_sub(1, Ordering::Release) - 1
}

unsafe extern "system" fn query_continue_drag(this: *mut DropSource, escape: i32, keys: u32) -> i32 {
    let _ = this;
    if escape != 0 {
        DRAGDROP_S_CANCEL
    } else if keys & 1 == 0 {
        // MK_LBUTTON
        DRAGDROP_S_DROP
    } else {
        0 // S_OK: continue
    }
}

unsafe extern "system" fn give_feedback(_this: *mut DropSource, _effect: u32) -> i32 {
    DRAGDROP_S_USEDEFAULTCURSORS
}

#[link(name = "shell32")]
unsafe extern "system" {
    fn SHDoDragDrop(hwnd: HWND, data: *mut std::ffi::c_void, source: *mut std::ffi::c_void, effect: u32, result: *mut u32) -> i32;
}

/// Starts a copy-style OLE drag. The source file and DeskTuk reference are never moved.
pub fn start(owner: HWND, path: &Path) -> windows::core::Result<()> {
    // The app thread is already a COM STA; OleInitialize enables the OLE drag loop.
    unsafe { OleInitialize(None)? };
    let result = unsafe { start_initialized(owner, path) };
    unsafe { OleUninitialize() };
    result
}

unsafe fn start_initialized(owner: HWND, path: &Path) -> windows::core::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let path_wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let mut absolute: *mut ITEMIDLIST = std::ptr::null_mut();
    SHParseDisplayName(PCWSTR(path_wide.as_ptr()), None, &mut absolute, 0, None)?;
    if absolute.is_null() {
        return Err(windows::core::Error::from_hresult(E_FAIL));
    }

    let parent = ILClone(absolute);
    if parent.is_null() || ILRemoveLastID(Some(parent)) == false {
        ILFree(Some(absolute));
        if !parent.is_null() {
            ILFree(Some(parent));
        }
        return Err(windows::core::Error::from_hresult(E_FAIL));
    }
    let child = ILFindChild(parent, absolute);
    if child.is_null() {
        ILFree(Some(parent));
        ILFree(Some(absolute));
        return Err(windows::core::Error::from_hresult(E_FAIL));
    }

    let children = [child as *const ITEMIDLIST];
    let data_result: windows::core::Result<IDataObject> = SHCreateDataObject(Some(parent), Some(&children), None);
    ILFree(Some(parent));
    ILFree(Some(absolute));
    let data = data_result?;

    let mut source = Box::new(DropSource { vtable: &DROP_SOURCE_VTABLE, references: AtomicU32::new(1) });
    let source_ptr: *mut DropSource = &mut *source;
    let mut effect = 0;
    let status = SHDoDragDrop(owner, data.as_raw(), source_ptr.cast(), DROPEFFECT_COPY.0, &mut effect);
    match status {
        0 | DRAGDROP_S_DROP | DRAGDROP_S_CANCEL => Ok(()),
        error => Err(windows::core::Error::from_hresult(windows::core::HRESULT(error))),
    }
}
