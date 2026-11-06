//! Starting and stopping one emulator. Same flags, environment and log file as
//! `adx emulator start`, so both tools see the same thing.

use std::fs::{self, File};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::adb::Adb;
use crate::console::Console;
use crate::error::{Error, Result};
use crate::proc::{self, EmuProc};

/// How to launch (from config and command-line flags).
#[derive(Debug, Clone, Default)]
pub struct Launch {
    pub gpu: String,
    /// No window and no audio (ADX_EMU_HEADLESS, unless `--window`).
    pub headless: bool,
    /// `--cold`: ignore the quick-boot snapshot.
    pub cold: bool,
    /// `--wipe`: factory reset (destroys the image's user data).
    pub wipe: bool,
    /// Environment for the emulator process: the SDK and AVD locations aodm resolved
    /// (adx exports its whole config; aodm passes what the emulator reads), then ADX_EMU_ENV.
    pub env: Vec<(String, String)>,
    /// Previous emulator logs kept (ADX_LOG_KEEP, as adx).
    pub log_keep: u64,
}

/// `ADX_EMU_ENV`: space-separated `VAR=value` pairs (no spaces inside values), as in adx.
pub fn parse_env(s: &str) -> Result<Vec<(String, String)>> {
    s.split_whitespace()
        .map(|kv| {
            let (k, v) = kv.split_once('=').unwrap_or(("", ""));
            let ok = k.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            if ok {
                Ok((k.to_string(), v.to_string()))
            } else {
                Err(Error::Config(format!(
                    "ADX_EMU_ENV: '{kv}' is not VAR=value"
                )))
            }
        })
        .collect()
}

pub fn args(image: &str, port: u16, l: &Launch) -> Vec<String> {
    let mut a: Vec<String> = [
        "-avd",
        image,
        "-port",
        &port.to_string(),
        "-gpu",
        &l.gpu,
        "-no-boot-anim",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if l.headless {
        a.extend(["-no-window".into(), "-no-audio".into()]);
    }
    if l.cold {
        a.push("-no-snapshot-load".into());
    }
    if l.wipe {
        a.push("-wipe-data".into());
    }
    a
}

/// `$ADX_LOG_DIR/emulator-<image>.log`, the file `adx logs emulator` reads.
pub fn log_path(log_dir: &Path, image: &str) -> PathBuf {
    log_dir.join(format!("emulator-{image}.log"))
}

/// Starts the emulator detached (own process group: Ctrl-C or closing aodm's
/// terminal doesn't kill it). Previous runs' logs are kept as `.1`… like adx.
pub fn spawn(emulator: &Path, image: &str, port: u16, l: &Launch, log: &Path) -> Result<Child> {
    if let Some(d) = log.parent() {
        fs::create_dir_all(d).map_err(Error::io(d))?;
    }
    crate::log::shift(log, l.log_keep).map_err(Error::io(log))?;
    let out = File::create(log).map_err(Error::io(log))?;
    let err = out.try_clone().map_err(Error::io(log))?;
    Command::new(emulator)
        .args(args(image, port, l))
        .envs(l.env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err)
        .process_group(0)
        .spawn()
        .map_err(Error::io(emulator))
}

/// Waits until Android has booted; fails fast if the emulator exits, the device
/// stays `unauthorized` (user data from a crashed boot) or the timeout passes.
pub fn wait_boot(
    child: &mut Child,
    adb: &Adb,
    serial: &str,
    timeout: Duration,
    log: &Path,
) -> Result<Duration> {
    let t0 = Instant::now();
    loop {
        if adb.booted(serial) {
            return Ok(t0.elapsed());
        }
        if let Ok(Some(st)) = child.try_wait() {
            return Err(Error::Refused(format!(
                "emulator exited during boot ({st}); see {} (retry, or --wipe if it keeps failing)",
                log.display()
            )));
        }
        if t0.elapsed() >= Duration::from_secs(60) && adb.state(serial) == "unauthorized" {
            return Err(Error::Refused(format!(
                "{serial} is 'unauthorized' (user data from a crashed boot?): restart it with --wipe"
            )));
        }
        if t0.elapsed() >= timeout {
            return Err(Error::Timeout {
                what: format!("boot of {serial} (see {})", log.display()),
                secs: timeout.as_secs(),
            });
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

/// How an emulator went down.
#[derive(Debug, PartialEq, Eq)]
pub enum Stopped {
    Console,
    /// SIGTERM after the console path failed, with why.
    Sigterm(String),
}

/// Stops one running emulator: console `kill` (the emulator saves its quick-boot
/// snapshot) once the console confirms it is `p.avd`, then SIGTERM if it is still
/// alive after `grace`. Never SIGKILL: that can corrupt the image; a stuck emulator
/// is reported instead.
pub fn stop(p: &EmuProc, token: Option<&str>, grace: Duration) -> Result<Stopped> {
    let why = match p.port {
        None => "no console port known".to_string(),
        Some(port) => match Console::connect(port, token, Duration::from_secs(5))
            .and_then(|mut c| Ok((c.command("avd name")?, c)))
        {
            // Another emulator may hold the port by now: only kill the one we mean.
            Ok((name, _)) if name.trim() != p.avd => {
                format!("console :{port} answers as '{}'", name.trim())
            }
            Ok((_, mut c)) => match c.command("kill") {
                Ok(_) | Err(Error::Io { .. }) if wait_gone(p, grace) => {
                    return Ok(Stopped::Console);
                }
                Ok(_) => format!("still running {} s after console kill", grace.as_secs()),
                Err(e) => format!("console kill: {e}"),
            },
            Err(e) => format!("console: {e}"),
        },
    };
    if !still_running(p) {
        return Ok(Stopped::Sigterm(format!("{why} (it exited anyway)")));
    }
    let pid = rustix::process::Pid::from_raw(p.pid as i32)
        .ok_or_else(|| Error::Refused(format!("bad pid {}", p.pid)))?;
    rustix::process::kill_process(pid, rustix::process::Signal::TERM).map_err(|e| Error::Io {
        path: format!("pid {}", p.pid).into(),
        source: e.into(),
    })?;
    if wait_gone(p, Duration::from_secs(15)) {
        Ok(Stopped::Sigterm(why))
    } else {
        Err(Error::Refused(format!(
            "{} (pid {}) did not exit after SIGTERM ({why}); check it, then `kill -9 {}` as a last resort",
            p.avd, p.pid, p.pid
        )))
    }
}

/// The running emulator of this image: a same-named emulator only counts if it holds
/// this image's `multiinstance.lock` (one from another AVD home, e.g. a sandbox, doesn't).
pub fn running<'a>(procs: &'a [EmuProc], image: &crate::avd::Image) -> Option<&'a EmuProc> {
    proc::running(procs, &image.name).filter(|_| in_use(&image.dir))
}

/// The same emulator still holds this pid (guards against pid reuse).
fn still_running(p: &EmuProc) -> bool {
    fs::read(format!("/proc/{}/cmdline", p.pid))
        .ok()
        .and_then(|raw| proc::parse_cmdline(p.pid, &raw))
        .is_some_and(|q| q.avd == p.avd)
}

fn wait_gone(p: &EmuProc, within: Duration) -> bool {
    let t0 = Instant::now();
    while still_running(p) {
        if t0.elapsed() >= within {
            return false;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    true
}

/// Some process holds the image's `multiinstance.lock` (spike: authoritative while an
/// emulator runs it, even one the `/proc` scan can't attribute).
pub fn in_use(avd_dir: &Path) -> bool {
    File::open(avd_dir.join("multiinstance.lock")).is_ok_and(|f| f.try_lock_shared().is_err())
}

/// Error lines in an emulator log (what `adx emulator status` looks for).
pub fn log_errors(log: &Path) -> Vec<String> {
    fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter(|l| {
            ["FATAL", "Segmentation", "context was preempted", "null ctx"]
                .iter()
                .any(|k| l.contains(k))
        })
        .map(|l| l.trim().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_args_match_adx() {
        let l = Launch {
            gpu: "software".into(),
            headless: true,
            ..Launch::default()
        };
        assert_eq!(
            args("adx_api37", 5554, &l).join(" "),
            "-avd adx_api37 -port 5554 -gpu software -no-boot-anim -no-window -no-audio"
        );
        let l = Launch {
            gpu: "host".into(),
            cold: true,
            wipe: true,
            ..Launch::default()
        };
        assert_eq!(
            args("p", 5556, &l).join(" "),
            "-avd p -port 5556 -gpu host -no-boot-anim -no-snapshot-load -wipe-data"
        );
    }

    #[test]
    fn emu_env() {
        assert_eq!(
            parse_env(" __NV_PRIME_RENDER_OFFLOAD=1  A_B=x=y ").unwrap(),
            [
                ("__NV_PRIME_RENDER_OFFLOAD".into(), "1".into()),
                ("A_B".into(), "x=y".into())
            ]
        );
        assert!(parse_env("").unwrap().is_empty());
        for bad in ["novalue", "1X=2", "A-B=1", "=1"] {
            assert!(parse_env(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn multiinstance_lock() {
        let d = tempfile::tempdir().unwrap();
        assert!(!in_use(d.path()), "no lock file");
        let f = File::create(d.path().join("multiinstance.lock")).unwrap();
        assert!(!in_use(d.path()));
        f.lock().unwrap();
        assert!(in_use(d.path()));
    }

    #[test]
    fn finds_log_errors() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("emulator-x.log");
        fs::write(&p, "INFO boot\nERROR | null ctx\nFATAL: oops\n").unwrap();
        assert_eq!(log_errors(&p).len(), 2);
        assert!(log_errors(&d.path().join("missing")).is_empty());
    }
}
