//! Diagnostics: optional log file, startup timing and resource measurements.

use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

use windows_sys::Win32::Foundation::FILETIME;
use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX};
use windows_sys::Win32::System::SystemInformation::GetSystemTimePreciseAsFileTime;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetGuiResources, GetProcessTimes, GR_GDIOBJECTS, GR_USEROBJECTS};

use crate::win::filetime_to_u64;

static LOG: Mutex<Option<File>> = Mutex::new(None);
static START: Mutex<Option<Instant>> = Mutex::new(None);

/// Records the time `main` was entered. Call first thing.
pub fn mark_start() {
    *START.lock().unwrap() = Some(Instant::now());
}

/// Milliseconds since `mark_start`.
pub fn since_start_ms() -> f64 {
    START.lock().unwrap().map(|s| s.elapsed().as_secs_f64() * 1000.0).unwrap_or(0.0)
}

/// Milliseconds since the OS created this process (includes loader + CRT init).
pub fn since_process_creation_ms() -> f64 {
    unsafe {
        let mut creation = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let mut exit = creation;
        let mut kernel = creation;
        let mut user = creation;
        if GetProcessTimes(GetCurrentProcess(), &mut creation, &mut exit, &mut kernel, &mut user) == 0 {
            return -1.0;
        }
        let mut now = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        GetSystemTimePreciseAsFileTime(&mut now);
        (filetime_to_u64(&now).saturating_sub(filetime_to_u64(&creation))) as f64 / 10_000.0
    }
}

pub fn open_log(path: &Path) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(f) = File::create(path) {
        *LOG.lock().unwrap() = Some(f);
    }
}

pub fn log_enabled() -> bool {
    LOG.lock().unwrap().is_some()
}

pub fn write_line(s: &str) {
    let t = since_start_ms();
    if let Some(f) = LOG.lock().unwrap().as_mut() {
        let _ = writeln!(f, "[{t:9.1} ms] {s}");
        let _ = f.flush();
    }
}

/// `dlog!("fmt", args)` - writes a timestamped line to the log file if one is open.
#[macro_export]
macro_rules! dlog {
    ($($arg:tt)*) => {
        if $crate::diag::log_enabled() {
            $crate::diag::write_line(&format!($($arg)*));
        }
    };
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Resources {
    pub working_set_kb: usize,
    pub peak_working_set_kb: usize,
    pub private_kb: usize,
    pub gdi_objects: u32,
    pub user_objects: u32,
}

pub fn resources() -> Resources {
    unsafe {
        let mut pmc: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
        pmc.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
        let proc = GetCurrentProcess();
        let ok = K32GetProcessMemoryInfo(proc, &mut pmc as *mut _ as *mut PROCESS_MEMORY_COUNTERS, pmc.cb) != 0;
        Resources {
            working_set_kb: if ok { pmc.WorkingSetSize / 1024 } else { 0 },
            peak_working_set_kb: if ok { pmc.PeakWorkingSetSize / 1024 } else { 0 },
            private_kb: if ok { pmc.PrivateUsage / 1024 } else { 0 },
            gdi_objects: GetGuiResources(proc, GR_GDIOBJECTS),
            user_objects: GetGuiResources(proc, GR_USEROBJECTS),
        }
    }
}

pub fn log_resources(label: &str) {
    let r = resources();
    dlog!(
        "METRIC resources[{label}] working_set_kb={} peak_working_set_kb={} private_kb={} gdi_objects={} user_objects={}",
        r.working_set_kb,
        r.peak_working_set_kb,
        r.private_kb,
        r.gdi_objects,
        r.user_objects
    );
}
