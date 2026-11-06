//! Running external tools (adb) with a deadline: a hung adb never hangs aodm.

use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct Output {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Runs `cmd` to completion, or kills it (and anything it started) after `timeout`.
pub fn run(cmd: &mut Command, timeout: Duration) -> Result<Output> {
    let what = format!("{:?}", cmd.get_program());
    let mut child = cmd
        .process_group(0) // own group: a timeout kills grandchildren holding the pipes too
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(Error::io(cmd.get_program()))?;
    let (mut out, mut err) = (child.stdout.take(), child.stderr.take());
    let deadline = Instant::now() + timeout;
    // Pipes are drained on threads so a chatty tool can't block on a full pipe.
    std::thread::scope(|s| {
        let o = s.spawn(|| drain(out.as_mut()));
        let e = s.spawn(|| drain(err.as_mut()));
        let status = loop {
            match child.try_wait() {
                Ok(Some(st)) => break Some(st),
                Ok(None) if Instant::now() >= deadline => {
                    if let Some(pg) = rustix::process::Pid::from_raw(child.id() as i32) {
                        let _ =
                            rustix::process::kill_process_group(pg, rustix::process::Signal::KILL);
                    }
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                Err(_) => break None,
            }
        };
        let (stdout, stderr) = (o.join().unwrap_or_default(), e.join().unwrap_or_default());
        match status {
            Some(st) => Ok(Output {
                ok: st.success(),
                stdout,
                stderr,
            }),
            None => Err(Error::Timeout {
                what,
                secs: timeout.as_secs(),
            }),
        }
    })
}

fn drain(r: Option<&mut impl Read>) -> String {
    let mut buf = Vec::new();
    if let Some(r) = r {
        let _ = r.read_to_end(&mut buf);
    }
    String::from_utf8_lossy(&buf).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_output_and_status() {
        let o = run(
            Command::new("sh").args(["-c", "echo hi; echo err >&2; exit 3"]),
            Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(
            (o.ok, o.stdout.as_str(), o.stderr.as_str()),
            (false, "hi\n", "err\n")
        );
    }

    #[test]
    fn kills_on_timeout() {
        let t = Instant::now();
        let e = run(
            Command::new("sh").args(["-c", "sleep 30 & sleep 30"]),
            Duration::from_millis(200),
        )
        .unwrap_err();
        assert!(matches!(e, Error::Timeout { .. }), "{e}");
        assert!(t.elapsed() < Duration::from_secs(5));
    }
}
