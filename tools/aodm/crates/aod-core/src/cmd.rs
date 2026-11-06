//! Running external tools (adb) with a deadline: a hung adb never hangs aodm.

use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct Output {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Runs `cmd` to completion, or kills it (and anything it started) after `timeout`.
/// The deadline also covers the output: a grandchild still holding the pipes after the
/// child exited (e.g. a daemon forked by adb) can't make aodm wait either.
pub fn run(cmd: &mut Command, timeout: Duration) -> Result<Output> {
    let what = format!("{:?}", cmd.get_program());
    let mut child = cmd
        .process_group(0) // own group: a timeout kills grandchildren holding the pipes too
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(Error::io(cmd.get_program()))?;
    let deadline = Instant::now() + timeout;
    let out = reader(child.stdout.take());
    let err = reader(child.stderr.take());
    let pgid = rustix::process::Pid::from_raw(child.id() as i32);
    let kill_group = || {
        if let Some(pg) = pgid {
            let _ = rustix::process::kill_process_group(pg, rustix::process::Signal::KILL);
        }
    };
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Some(st),
            Ok(None) if Instant::now() >= deadline => {
                kill_group();
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => break None,
        }
    };
    if let Some(st) = status {
        let left = || deadline.saturating_duration_since(Instant::now());
        match (out.recv_timeout(left()), err.recv_timeout(left())) {
            (Ok(stdout), Ok(stderr)) => {
                return Ok(Output {
                    ok: st.success(),
                    stdout,
                    stderr,
                });
            }
            // A grandchild kept the pipes open past the deadline: kill its group.
            // shortcut: the pgid could be reused only after every member exited; harmless then.
            _ => kill_group(),
        }
    }
    Err(Error::Timeout {
        what,
        secs: timeout.as_secs(),
    })
}

/// Reads a pipe to the end on a detached thread, so an abandoned pipe never blocks the caller.
fn reader(r: Option<impl Read + Send + 'static>) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut r) = r {
            let _ = r.read_to_end(&mut buf);
        }
        let _ = tx.send(String::from_utf8_lossy(&buf).into_owned());
    });
    rx
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
    fn grandchild_holding_pipes_times_out() {
        let t = Instant::now();
        let e = run(
            Command::new("sh").args(["-c", "sleep 6 & echo hi"]),
            Duration::from_millis(300),
        )
        .unwrap_err();
        assert!(matches!(e, Error::Timeout { .. }), "{e}");
        assert!(t.elapsed() < Duration::from_secs(3), "{:?}", t.elapsed());
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
