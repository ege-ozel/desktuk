//! Desktop hosting strategies.
//!
//! A *host* decides how a Node window is attached to the Windows desktop: which
//! parent/owner and styles it is created with, how it is positioned, how it reacts
//! to z-order changes, "Show desktop" (Win+D) and Explorer restarts. Everything
//! outside this module talks to the host only through [`DesktopHost`], so a strategy
//! can be replaced without touching the Node UI.
//!
//! | kind     | relationship to Explorer                 | status        |
//! |----------|------------------------------------------|---------------|
//! | `window` | none - ordinary top-level tool window    | default       |
//! | `owned`  | top-level window *owned* by `Progman`    | experimental  |
//! | `child`  | `WS_CHILD` of the desktop icon-view host | experimental  |

use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::WINDOWPOS;

mod child;
mod owned;
mod window;

pub use child::ChildHost;
pub use owned::OwnedHost;
pub use window::exclude_from_peek;
pub use window::WindowHost;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostKind {
    Window,
    Owned,
    Child,
}

impl HostKind {
    pub fn parse(s: &str) -> Option<HostKind> {
        match s.to_ascii_lowercase().as_str() {
            "window" | "auto" | "default" => Some(HostKind::Window),
            "owned" => Some(HostKind::Owned),
            "child" => Some(HostKind::Child),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            HostKind::Window => "window",
            HostKind::Owned => "owned",
            HostKind::Child => "child",
        }
    }

    pub fn experimental(self) -> bool {
        self != HostKind::Window
    }
}

pub fn create(kind: HostKind) -> std::rc::Rc<dyn DesktopHost> {
    match kind {
        HostKind::Window => std::rc::Rc::new(WindowHost::default()),
        HostKind::Owned => std::rc::Rc::new(OwnedHost),
        HostKind::Child => std::rc::Rc::new(ChildHost),
    }
}

/// Parameters for `CreateWindowExW`.
pub struct CreateSpec {
    pub ex_style: u32,
    pub style: u32,
    /// Parent (for `WS_CHILD`) or owner (for popups); null for none.
    pub parent: HWND,
}

/// The replaceable desktop integration interface.
///
/// All methods take `&self` and are called on the UI thread; implementations keep any
/// mutable state in `Cell`s because Win32 calls they make (e.g. `SetWindowPos`) can
/// re-enter the window procedure, which calls back into the host.
pub trait DesktopHost {
    fn kind(&self) -> HostKind;

    /// Decides parent/owner and window styles for a new node window.
    /// Fails when the desktop it depends on is not available (e.g. Explorer not running).
    fn prepare(&self) -> Result<CreateSpec, String>;

    /// Called once right after the node window was created, before it is shown.
    fn attached(&self, node: HWND);

    /// Circles can opt into a persistent topmost band; panel hosts keep their policy.
    fn set_always_on_top(&self, _node: HWND, _enabled: bool) {}

    /// Return true when a shell system command should be ignored by this host.
    fn system_command(&self, _command: usize) -> bool {
        false
    }

    /// Explicit recovery from the tray; ordinary foreground changes restore desktop ordering.
    fn reveal(&self, node: HWND) {
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(node, windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNOACTIVATE);
        }
    }

    /// Hide a node on explicit user request, distinct from Shell Show Desktop ordering.
    fn hide(&self, node: HWND) {
        unsafe { windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(node, windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE) };
    }

    /// Restore host ordering after temporary foreground/capture ownership.
    fn capture_finished(&self, node: HWND, foreground: HWND, _was_topmost: bool) {
        self.foreground_changed(node, foreground);
    }

    /// Moves/resizes the node. `r` is in screen (virtual desktop) coordinates.
    fn place(&self, node: HWND, r: &RECT);

    /// `WM_WINDOWPOSCHANGING` hook for the node (z-order policy).
    fn window_pos_changing(&self, _node: HWND, _wp: &mut WINDOWPOS) {}

    /// The foreground window changed (from a WinEvent hook). `fg` may be any window.
    fn foreground_changed(&self, _node: HWND, _fg: HWND) {}

    /// Whether the node is still attached to a live desktop (parent/owner exists).
    fn is_healthy(&self, _node: HWND) -> bool {
        true
    }

    /// Explorer (re)started - the `TaskbarCreated` broadcast was received.
    /// Return `true` if the node (null if it was already destroyed) must be re-created.
    fn shell_restarted(&self, node: HWND) -> bool;
}
