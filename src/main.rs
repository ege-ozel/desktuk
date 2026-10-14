//! DeskTuk - native Win32 desktop Nodes. Phase 0/1 proof of concept.
//!
//! No GUI framework: plain Win32 windows, GDI painting and Shell APIs via `windows-sys`.
#![windows_subsystem = "windows"]

#[cfg(not(windows))]
compile_error!("DeskTuk is a Windows application: build with --target x86_64-pc-windows-gnu or on Windows (msvc).");

mod app;
mod autostart;
mod circles;
mod config;
mod desktop_view;
mod diag;
mod host;
mod items;
mod node;
mod selftest;
mod shell;
mod shell_menu;
mod tray;
mod watch;
mod win;

fn main() {
    std::process::exit(app::run());
}
