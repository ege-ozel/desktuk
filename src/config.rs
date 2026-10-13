//! Command line and the persisted node configuration (%APPDATA%\DeskTuk\desktuk.cfg).
//!
//! The file is a plain `key=value` text file so users can inspect and edit it:
//!
//! ```text
//! host=window
//! x=120
//! y=120
//! item=C:\Users\me\Desktop\Notes.txt
//! item=C:\Users\me\Desktop\Visual Studio Code.lnk
//! ```

use std::path::PathBuf;

use crate::host::HostKind;

#[derive(Clone, Debug)]
pub struct Config {
    pub host: HostKind,
    pub pos: Option<(i32, i32)>,
    pub items: Vec<PathBuf>,
}

#[derive(Debug, Default)]
pub struct Args {
    pub host: Option<HostKind>,
    pub pos: Option<(i32, i32)>,
    pub items: Vec<PathBuf>,
    pub log: Option<PathBuf>,
    pub selftest: bool,
    pub selftest_explorer: bool,
    pub shots: Option<PathBuf>,
    pub exit_after_ms: Option<u32>,
    pub no_save: bool,
    pub allow_multiple: bool,
    pub diag_only: bool,
    pub help: bool,
}

pub const USAGE: &str = "DeskTuk.exe [options] [item paths...]

  --host <window|owned|child>  Desktop hosting strategy (default: window).
                               owned/child are EXPERIMENTAL (undocumented Explorer windows).
  --pos <x,y>                  Node position in screen pixels.
  --log <file>                 Write a diagnostic log.
  --exit-after <ms>            Quit after the given time (measurements).
  --selftest                   Run the automated integration self-test, then exit.
  --selftest-explorer-restart  Self-test also kills and restarts explorer.exe (CI only!).
  --shots <dir>                Self-test saves screenshots (BMP) into <dir>.
  --diag                       Log desktop/shell diagnostics and exit.
  --no-save                    Do not write the configuration file.
  --allow-multiple             Skip the single-instance check.

Item paths given on the command line replace the configured items for this run.";

pub fn parse_args() -> Result<Args, String> {
    let mut a = Args::default();
    let mut it = std::env::args_os().skip(1);
    while let Some(arg) = it.next() {
        let s = arg.to_string_lossy().into_owned();
        let mut value = |name: &str| it.next().map(|v| v.to_string_lossy().into_owned()).ok_or(format!("{name} needs a value"));
        match s.as_str() {
            "--host" => {
                let v = value("--host")?;
                a.host = Some(HostKind::parse(&v).ok_or(format!("unknown host '{v}'"))?);
            }
            "--pos" => {
                let v = value("--pos")?;
                a.pos = Some(parse_pos(&v).ok_or(format!("bad --pos '{v}'"))?);
            }
            "--log" => a.log = Some(PathBuf::from(value("--log")?)),
            "--shots" => a.shots = Some(PathBuf::from(value("--shots")?)),
            "--exit-after" => {
                let v = value("--exit-after")?;
                a.exit_after_ms = Some(v.parse().map_err(|_| format!("bad --exit-after '{v}'"))?);
            }
            "--selftest" => a.selftest = true,
            "--selftest-explorer-restart" => {
                a.selftest = true;
                a.selftest_explorer = true;
            }
            "--diag" => a.diag_only = true,
            "--no-save" => a.no_save = true,
            "--allow-multiple" => a.allow_multiple = true,
            "-h" | "--help" | "/?" => a.help = true,
            _ if s.starts_with("--") => return Err(format!("unknown option '{s}'")),
            _ => a.items.push(PathBuf::from(arg)),
        }
    }
    if a.selftest {
        a.no_save = true;
        a.allow_multiple = true;
    }
    Ok(a)
}

fn parse_pos(s: &str) -> Option<(i32, i32)> {
    let (x, y) = s.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

pub fn config_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("DeskTuk").join("desktuk.cfg"))
}

pub fn load() -> Option<Config> {
    let text = std::fs::read_to_string(config_path()?).ok()?;
    let mut c = Config { host: HostKind::Window, pos: None, items: Vec::new() };
    let (mut x, mut y) = (None, None);
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let v = v.trim();
        match k.trim() {
            "host" => c.host = HostKind::parse(v).unwrap_or(HostKind::Window),
            "x" => x = v.parse().ok(),
            "y" => y = v.parse().ok(),
            "item" if !v.is_empty() => c.items.push(PathBuf::from(v)),
            _ => {}
        }
    }
    if let (Some(x), Some(y)) = (x, y) {
        c.pos = Some((x, y));
    }
    Some(c)
}

pub fn save(c: &Config) -> std::io::Result<()> {
    let Some(path) = config_path() else { return Ok(()) };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut s = String::from("# DeskTuk configuration. Items are references; the files themselves are never moved or changed.\n");
    s += &format!("host={}\n", c.host.name());
    if let Some((x, y)) = c.pos {
        s += &format!("x={x}\ny={y}\n");
    }
    for p in &c.items {
        s += &format!("item={}\n", p.display());
    }
    // Write-then-rename so a crash never leaves a truncated file.
    let tmp = path.with_extension("cfg.tmp");
    std::fs::write(&tmp, s)?;
    std::fs::rename(&tmp, &path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_positions() {
        assert_eq!(parse_pos("10,20"), Some((10, 20)));
        assert_eq!(parse_pos(" -5 , 7 "), Some((-5, 7)));
        assert_eq!(parse_pos("10"), None);
        assert_eq!(parse_pos("a,b"), None);
    }

    #[test]
    fn host_names_round_trip() {
        for k in [HostKind::Window, HostKind::Owned, HostKind::Child] {
            assert_eq!(HostKind::parse(k.name()), Some(k));
        }
        assert_eq!(HostKind::parse("auto"), Some(HostKind::Window));
        assert_eq!(HostKind::parse("CHILD"), Some(HostKind::Child));
        assert_eq!(HostKind::parse("workerw"), None);
        assert!(!HostKind::Window.experimental());
        assert!(HostKind::Child.experimental());
    }
}
