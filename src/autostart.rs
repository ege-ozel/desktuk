//! The per-user Run value. Startup is opt-in; never change it during normal startup.
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegGetValueW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE,
    REG_OPTION_NON_VOLATILE, REG_SZ, RRF_RT_REG_SZ,
};

use crate::win;

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE: &str = "DeskTuk";

/// Quoted UTF-16 including the terminator; preserve even non-Unicode Windows paths.
fn run_value(exe: &Path) -> io::Result<Vec<u16>> {
    let mut value = vec![b'"' as u16];
    value.extend(exe.as_os_str().encode_wide());
    value.push(b'"' as u16);
    if value.len() > 260 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Windows startup commands cannot exceed 260 characters"));
    }
    value.push(0);
    Ok(value)
}

fn read_value(key: &str) -> io::Result<Option<Vec<u16>>> {
    let key = win::wide(key);
    let name = win::wide(VALUE);
    let mut data = vec![0u16; 261];
    for _ in 0..3 {
        let mut bytes = (data.len() * 2) as u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                data.as_mut_ptr().cast(),
                &mut bytes,
            )
        };
        match status {
            0 => {
                data.truncate(bytes as usize / 2);
                return Ok(Some(data));
            }
            ERROR_FILE_NOT_FOUND => return Ok(None),
            ERROR_MORE_DATA => data.resize((bytes as usize).div_ceil(2), 0),
            _ => return Err(io::Error::from_raw_os_error(status as i32)),
        }
    }
    Err(io::Error::new(io::ErrorKind::WouldBlock, "Startup value changed while reading it"))
}

fn write_value(key: &str, exe: Option<&Path>) -> io::Result<()> {
    // Validate before opening/creating the registry key.
    let data = exe.map(run_value).transpose()?;
    let path = win::wide(key);
    let name = win::wide(VALUE);
    let mut handle: HKEY = std::ptr::null_mut();
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            path.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut handle,
            std::ptr::null_mut(),
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let status = unsafe {
        let result = match data {
            Some(data) => RegSetValueExW(handle, name.as_ptr(), 0, REG_SZ, data.as_ptr().cast(), (data.len() * 2) as u32),
            None => RegDeleteValueW(handle, name.as_ptr()),
        };
        RegCloseKey(handle);
        result
    };
    if status == 0 || (exe.is_none() && status == ERROR_FILE_NOT_FOUND) {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(status as i32))
    }
}

/// Read the registry whenever the menu opens, including changes made outside DeskTuk.
pub fn is_enabled() -> io::Result<bool> {
    Ok(read_value(RUN_KEY)?.is_some())
}

pub fn set_enabled(on: bool) -> io::Result<()> {
    let exe = if on { Some(std::env::current_exe()?) } else { None };
    write_value(RUN_KEY, exe.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_value_quotes_spaces_and_preserves_unicode() {
        for path in [r"C:\Program Files\DeskTuk\DeskTuk.exe", r"C:\Üser\Müzik\DeskTuk.exe"] {
            let data = run_value(Path::new(path)).unwrap();
            assert_eq!(String::from_utf16(&data[..data.len() - 1]).unwrap(), format!("\"{path}\""));
            assert_eq!(data.last(), Some(&0));
        }
    }

    #[test]
    fn run_value_rejects_commands_over_the_windows_limit() {
        assert!(run_value(Path::new(&format!("C:\\{}\\DeskTuk.exe", "a".repeat(260)))).is_err());
    }

    #[test]
    fn registry_toggle_round_trips_without_touching_run() {
        use windows_sys::Win32::System::Registry::{RegDeleteKeyW, HKEY_CURRENT_USER};
        let key = format!("Software\\DeskTuk-test-{}-{}", std::process::id(), unsafe {
            windows_sys::Win32::System::Threading::GetCurrentThreadId()
        });
        let exe = Path::new(r"C:\Program Files\Üser\DeskTuk.exe");
        assert_eq!(read_value(&key).unwrap(), None);
        write_value(&key, Some(exe)).unwrap();
        assert_eq!(read_value(&key).unwrap(), Some(run_value(exe).unwrap()));
        write_value(&key, None).unwrap();
        assert_eq!(read_value(&key).unwrap(), None);
        write_value(&key, None).unwrap(); // disabling an absent value is idempotent
        assert_eq!(unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, win::wide(&key).as_ptr()) }, 0);
    }
}
