//! Populate native launch menus on demand using only existing file references.
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use windows_sys::Win32::UI::WindowsAndMessaging::*;

use super::view::{SlotRef, Target};
use crate::{app::app, items, win};

const FILE_BASE: usize = 0x2000_0000;
const PAGE_SIZE: usize = 32;

enum Pending {
    Target(Target),
    Files(Rc<Vec<PathBuf>>, usize),
}

#[derive(Default)]
struct Session {
    pending: HashMap<usize, Pending>,
    files: Vec<PathBuf>,
}

thread_local! {
    static SESSION: RefCell<Session> = RefCell::new(Session::default());
}

pub fn reset() {
    SESSION.with(|s| *s.borrow_mut() = Session::default());
}

fn append_pending(parent: HMENU, label: &str, pending: Pending) {
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, win::wide("...").as_ptr());
        if AppendMenuW(parent, MF_POPUP, menu as usize, win::wide(label.replace('&', "&&")).as_ptr()) == 0 {
            DestroyMenu(menu);
            return;
        }
        SESSION.with(|s| s.borrow_mut().pending.insert(menu as usize, pending));
    }
}

pub fn append_target(parent: HMENU, label: &str, target: Target) {
    append_pending(parent, label, Pending::Target(target));
}

fn append_files(menu: HMENU, paths: Rc<Vec<PathBuf>>, offset: usize) {
    for path in paths.iter().skip(offset).take(PAGE_SIZE) {
        let name = path.file_name().unwrap_or(path.as_os_str()).to_string_lossy().replace('&', "&&");
        let command = SESSION.with(|s| {
            let mut s = s.borrow_mut();
            let id = FILE_BASE + s.files.len();
            s.files.push(path.clone());
            id
        });
        unsafe { AppendMenuW(menu, MF_STRING, command, win::wide(&name).as_ptr()) };
    }
    let next = offset + PAGE_SIZE;
    if next < paths.len() {
        let label = format!("{} ({}-{})", crate::i18n::choose("Devamı", "More"), next + 1, paths.len());
        append_pending(menu, &label, Pending::Files(paths, next));
    }
}

pub fn populate(menu: HMENU) {
    // Copy state before any native operation can pump messages.
    let pending = SESSION.with(|s| s.borrow_mut().pending.remove(&(menu as usize)));
    let Some(pending) = pending else { return };
    unsafe { DeleteMenu(menu, 0, MF_BYPOSITION) };
    match pending {
        Pending::Files(paths, offset) => append_files(menu, paths, offset),
        Pending::Target(target) => {
            let refs = if target == Target::Recent {
                super::recent_paths().into_iter().map(SlotRef::Item).collect()
            } else {
                app().board.try_borrow().map(|b| super::view::slot_refs(&b, target, &[], &[])).unwrap_or_default()
            };
            let groups: Vec<(usize, String)> = if let Target::Circle(ci) = target {
                app()
                    .board
                    .try_borrow()
                    .ok()
                    .and_then(|b| b.circles.get(ci).map(|c| c.groups.iter().enumerate().map(|(gi, g)| (gi, g.name.clone())).collect()))
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            for (gi, name) in groups {
                if let Target::Circle(ci) = target {
                    append_target(menu, &name, Target::Group(ci, gi));
                }
            }
            let paths: Vec<_> = refs.into_iter().filter_map(|r| if let SlotRef::Item(p) = r { Some(p) } else { None }).collect();
            append_files(menu, Rc::new(paths), 0);
            if let Target::Circle(ci) = target {
                unsafe {
                    AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
                    let actions = CreatePopupMenu();
                    if !actions.is_null() {
                        for (offset, label) in [
                            "New group",
                            "Rename...",
                            "Delete circle",
                            "Ada g\u{f6}re s\u{131}rala",
                            "T\u{fc}re g\u{f6}re s\u{131}rala",
                            "Eklenme s\u{131}ras\u{131}",
                        ]
                        .iter()
                        .enumerate()
                        {
                            AppendMenuW(
                                actions,
                                MF_STRING,
                                super::CIRCLE_ACTION_BASE + ci * 8 + offset,
                                win::wide(crate::i18n::text(label)).as_ptr(),
                            );
                        }
                        if AppendMenuW(
                            menu,
                            MF_POPUP,
                            actions as usize,
                            win::wide(crate::i18n::text("\u{c7}ember i\u{15f}lemleri")).as_ptr(),
                        ) == 0
                        {
                            DestroyMenu(actions);
                        }
                    }
                }
            }
            unsafe {
                if GetMenuItemCount(menu) == 0 {
                    AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, win::wide(crate::i18n::text("Bo\u{15f}")).as_ptr());
                }
            }
        }
    }
}

pub fn finish(command: usize) -> bool {
    let session = SESSION.with(|s| std::mem::take(&mut *s.borrow_mut()));
    let path = command.checked_sub(FILE_BASE).and_then(|i| session.files.get(i)).cloned();
    if let Some(path) = path {
        let _ = items::open(&path, app().controller.get());
        true
    } else {
        false
    }
}

/// Native mnemonic handling must select a file, never execute a unique match.
pub fn select_character(menu: HMENU, value: u16) -> Option<isize> {
    let len = SESSION.with(|s| s.borrow().files.len());
    if len == 0 {
        return None;
    }
    let needle = char::from_u32(value as u32)?.to_lowercase().collect::<String>();
    let count = unsafe { GetMenuItemCount(menu) }.max(0) as u32;
    let mut choices = Vec::new();
    let mut highlighted = None;
    for index in 0..count {
        let id = unsafe { GetMenuItemID(menu, index as i32) } as usize;
        if unsafe { GetMenuState(menu, index, MF_BYPOSITION) } & MF_HILITE != 0 {
            highlighted = Some(index);
        }
        if id < FILE_BASE || id >= FILE_BASE + len {
            continue;
        }
        let mut buf = [0u16; 512];
        let n = unsafe { GetMenuStringW(menu, index, buf.as_mut_ptr(), buf.len() as i32, MF_BYPOSITION) }.max(0) as usize;
        let label = String::from_utf16_lossy(&buf[..n]);
        if label.chars().next().is_some_and(|c| c.to_lowercase().collect::<String>() == needle) {
            choices.push(index);
        }
    }
    let index = choices.iter().copied().find(|i| highlighted.is_none_or(|h| *i > h)).or_else(|| choices.first().copied());
    index.map(|i| i as isize | (3 << 16)) // MNC_SELECT, deliberately never MNC_EXECUTE.
}
