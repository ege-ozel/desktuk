//! Transient hover typing, also routing edit keys when Windows denies popup activation.
use std::cell::Cell;
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub const WM_SEARCH_CHAR: u32 = WM_APP + 44;
pub const WM_SEARCH_KEY: u32 = WM_APP + 77;
thread_local! {
    static HOOK: Cell<HHOOK> = const { Cell::new(std::ptr::null_mut()) };
    static TAKEN: Cell<[bool; 256]> = const { Cell::new([false; 256]) };
}
pub fn install() {
    if HOOK.get().is_null() {
        HOOK.set(unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard), GetModuleHandleW(std::ptr::null()), 0) });
    }
}
pub fn uninstall() {
    let hook = HOOK.replace(std::ptr::null_mut());
    if !hook.is_null() {
        unsafe { UnhookWindowsHookEx(hook) };
    }
    TAKEN.set([false; 256]);
}
unsafe extern "system" fn keyboard(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    if code >= 0 {
        let key = unsafe { &*(data as *const KBDLLHOOKSTRUCT) };
        let vk = key.vkCode as usize;
        if vk < 256 {
            let mut taken = TAKEN.get();
            if matches!(message as u32, WM_KEYUP | WM_SYSKEYUP) {
                if taken[vk] {
                    taken[vk] = false;
                    TAKEN.set(taken);
                    return 1;
                }
            } else if message as u32 == WM_KEYDOWN {
                if let Some(hwnd) = super::ring::search_window() {
                    let down = |v: u16| unsafe { GetAsyncKeyState(v as i32) } < 0;
                    if vk == 0x41 && down(VK_CONTROL) && !down(VK_MENU) && !down(VK_LWIN) && !down(VK_RWIN) {
                        unsafe {
                            PostMessageW(hwnd, WM_SEARCH_CHAR, 1, 0);
                        }
                        taken[vk] = true;
                        TAKEN.set(taken);
                        return 1;
                    }
                    let editor = super::launcher::hover_editor();
                    let edit_shortcut = down(VK_CONTROL) && matches!(vk, 0x43 | 0x58 | 0x56 | 0x5a);
                    if editor
                        && !down(VK_LWIN)
                        && !down(VK_RWIN)
                        && !down(VK_MENU)
                        && (matches!(vk as u16, VK_UP | VK_DOWN | VK_RETURN | VK_LEFT | VK_RIGHT | VK_HOME | VK_END | VK_DELETE)
                            || edit_shortcut)
                    {
                        let modifiers = usize::from(down(VK_SHIFT)) | (usize::from(down(VK_CONTROL)) << 1);
                        unsafe {
                            PostMessageW(hwnd, WM_SEARCH_KEY, vk, modifiers as isize);
                        }
                        taken[vk] = true;
                        TAKEN.set(taken);
                        return 1;
                    }
                    // AltGr is Ctrl+RightAlt; ordinary Ctrl/Alt/Win shortcuts pass through.
                    if !down(VK_LWIN) && !down(VK_RWIN) && (!down(VK_CONTROL) || down(VK_RMENU)) && (!down(VK_MENU) || down(VK_RMENU)) {
                        let mut chars = Vec::new();
                        if matches!(vk as u16, VK_BACK | VK_ESCAPE) {
                            chars.push(vk as u32);
                        } else {
                            let mut state = [0u8; 256];
                            unsafe { GetKeyboardState(state.as_mut_ptr()) };
                            // A low-level hook runs before queued key state updates. Only
                            // modifiers need async correction; avoid 256 system calls/key.
                            for modifier in
                                [VK_SHIFT, VK_LSHIFT, VK_RSHIFT, VK_CONTROL, VK_LCONTROL, VK_RCONTROL, VK_MENU, VK_LMENU, VK_RMENU]
                            {
                                state[modifier as usize] = if down(modifier) { 0x80 } else { 0 };
                            }
                            state[VK_CAPITAL as usize] |= (unsafe { GetKeyState(VK_CAPITAL as i32) } & 1) as u8;
                            state[vk] |= 0x80;
                            let mut text = [0u16; 8];
                            let count = unsafe {
                                ToUnicodeEx(
                                    key.vkCode,
                                    key.scanCode,
                                    state.as_ptr(),
                                    text.as_mut_ptr(),
                                    text.len() as i32,
                                    4,
                                    GetKeyboardLayout(GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut())),
                                )
                            };
                            if count > 0 {
                                chars.extend(
                                    char::decode_utf16(text[..(count as usize).min(text.len())].iter().copied())
                                        .filter_map(Result::ok)
                                        .filter(|c| !c.is_control())
                                        .map(|c| c as u32),
                                );
                            }
                        }
                        if !chars.is_empty() {
                            for c in chars {
                                unsafe { PostMessageW(hwnd, WM_SEARCH_CHAR, c as usize, 0) };
                            }
                            taken[vk] = true;
                            TAKEN.set(taken);
                            return 1;
                        }
                    }
                }
            }
        }
    }
    unsafe { CallNextHookEx(HOOK.get(), code, message, data) }
}
