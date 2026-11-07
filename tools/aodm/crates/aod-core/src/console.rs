//! Emulator console client (`127.0.0.1:<console port>`), independent of adb.
//! Protocol (checked on emulator 37.2.13): a banner ending in `OK`, then `auth <token>`
//! with the token from `~/.emulator_console_auth_token` (no auth if that file is empty);
//! each command answers with lines ending in `OK` or `KO: <reason>`.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

use crate::error::{Error, Result};

pub struct Console {
    reader: BufReader<TcpStream>,
    port: u16,
}

/// The console auth token; `None` when the file is missing or empty (no auth needed).
pub fn token(home: &Path) -> Option<String> {
    let t = std::fs::read_to_string(home.join(".emulator_console_auth_token")).ok()?;
    let t = t.trim();
    // A token with control characters can't be sent as one line (and would end up in errors).
    (!t.is_empty() && !t.chars().any(char::is_control)).then(|| t.to_string())
}

impl Console {
    pub fn connect(port: u16, token: Option<&str>, timeout: Duration) -> Result<Console> {
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let io = |e| Error::Io {
            path: format!("console 127.0.0.1:{port}").into(),
            source: e,
        };
        let stream = TcpStream::connect_timeout(&addr, timeout).map_err(io)?;
        stream.set_read_timeout(Some(timeout)).map_err(io)?;
        stream.set_write_timeout(Some(timeout)).map_err(io)?;
        let mut c = Console {
            reader: BufReader::new(stream),
            port,
        };
        c.reply()?; // banner
        if let Some(t) = token {
            c.command(&format!("auth {t}"))
                .map_err(|e| Error::Failed(format!("console :{port}: auth failed: {e}")))?;
        }
        Ok(c)
    }

    /// Sends one command; returns the reply lines before `OK` (an `OK: text` line is
    /// returned as `text`), or the `KO:` reason as an error.
    pub fn command(&mut self, cmd: &str) -> Result<String> {
        if cmd.contains(['\n', '\r']) {
            // Never echo the command: it may be `auth <token>`.
            return Err(Error::Invalid {
                what: "console command",
                detail: "contains a line break".into(),
            });
        }
        let port = self.port;
        self.reader
            .get_mut()
            .write_all(format!("{cmd}\n").as_bytes())
            .map_err(|e| Error::Io {
                path: format!("console 127.0.0.1:{port}").into(),
                source: e,
            })?;
        self.reply()
    }

    fn reply(&mut self) -> Result<String> {
        let mut lines = Vec::new();
        loop {
            let mut line = String::new();
            let n = self.reader.read_line(&mut line).map_err(|e| Error::Io {
                path: format!("console 127.0.0.1:{}", self.port).into(),
                source: e,
            })?;
            let l = line.trim_end();
            if n == 0 || l == "OK" {
                // EOF counts as done: `kill` answers `OK: killing emulator, bye bye` and hangs up.
                return Ok(lines.join("\n"));
            }
            if let Some(rest) = l.strip_prefix("OK: ") {
                lines.push(rest.to_string());
                return Ok(lines.join("\n"));
            }
            if let Some(reason) = l.strip_prefix("KO:") {
                return Err(Error::Failed(format!(
                    "console :{}: {}",
                    self.port,
                    reason.trim()
                )));
            }
            lines.push(l.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /// A fake console speaking the real emulator's banner and replies.
    fn fake(script: &'static [(&'static str, &'static str)]) -> u16 {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            s.write_all(b"Android Console: Authentication required\r\nAndroid Console: type 'auth <auth_token>' to authenticate\r\nOK\r\n").unwrap();
            let mut r = BufReader::new(s.try_clone().unwrap());
            for (want, answer) in script {
                let mut line = String::new();
                r.read_line(&mut line).unwrap();
                assert_eq!(line.trim_end(), *want);
                s.write_all(answer.as_bytes()).unwrap();
            }
        });
        port
    }

    #[test]
    fn auth_and_commands() {
        let port = fake(&[
            (
                "auth sekrit",
                "Android Console: type 'help' for a list of commands\r\nOK\r\n",
            ),
            ("avd name", "adx_api37\r\nOK\r\n"),
            ("bogus", "KO: unknown command, try 'help'\r\n"),
            ("kill", "OK: killing emulator, bye bye\r\n"),
        ]);
        let mut c = Console::connect(port, Some("sekrit"), Duration::from_secs(5)).unwrap();
        assert_eq!(c.command("avd name").unwrap(), "adx_api37");
        assert!(
            c.command("bogus")
                .unwrap_err()
                .to_string()
                .contains("unknown command")
        );
        assert_eq!(c.command("kill").unwrap(), "killing emulator, bye bye");
        assert!(c.command("a\nkill").is_err(), "no command injection");
    }

    #[test]
    fn bad_token_is_refused() {
        let port = fake(&[(
            "auth wrong",
            "KO: authentication token does not match ~/.emulator_console_auth_token\r\n",
        )]);
        let e = Console::connect(port, Some("wrong"), Duration::from_secs(5))
            .err()
            .unwrap();
        assert!(e.to_string().contains("auth failed"), "{e}");
    }

    #[test]
    fn token_file() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(token(d.path()), None);
        std::fs::write(d.path().join(".emulator_console_auth_token"), "\n").unwrap();
        assert_eq!(token(d.path()), None, "empty file: no auth");
        std::fs::write(d.path().join(".emulator_console_auth_token"), "a\nb").unwrap();
        assert_eq!(token(d.path()), None, "inner line break rejected");
        std::fs::write(d.path().join(".emulator_console_auth_token"), "abc\n").unwrap();
        assert_eq!(token(d.path()).as_deref(), Some("abc"));
    }
}
