//! Writes to the framework log `$ADX_LOG_DIR/adx.log` in adx's exact line format:
//! `YYYY-mm-dd HH:MM:SS LEVEL [aodm #run] message`.
//! One `write_all` per line on an O_APPEND file, so lines interleave safely with adx's
//! bash writers. Logging never fails a command.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::env::Env;

#[derive(Debug, Clone)]
pub struct Logger {
    path: PathBuf,
    run: String,
}

impl Logger {
    /// Logger for this process; rotates the log first (same rule as adx).
    pub fn new(env: &Env) -> Logger {
        let dir = env.path("ADX_LOG_DIR");
        let path = dir.join("adx.log");
        // shortcut: rotation can race a concurrent adx run, harmless (one extra .N file).
        let _ = rotate(
            &path,
            env.num("ADX_LOG_MAX_KB", 5120) * 1024,
            env.num("ADX_LOG_KEEP", 5),
        );
        let run = std::env::var("ADX_RUN_ID").unwrap_or_else(|_| std::process::id().to_string());
        Logger { path, run }
    }

    /// Logger writing to an explicit file (tests, standalone phones).
    pub fn to_file(path: impl Into<PathBuf>, run: impl Into<String>) -> Logger {
        Logger {
            path: path.into(),
            run: run.into(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn info(&self, msg: &str) {
        self.write("INFO", msg);
    }

    pub fn warn(&self, msg: &str) {
        self.write("WARN", msg);
    }

    pub fn error(&self, msg: &str) {
        self.write("ERROR", msg);
    }

    fn write(&self, level: &str, msg: &str) {
        let ts = jiff::Zoned::now().strftime("%Y-%m-%d %H:%M:%S").to_string();
        // One line per entry: newlines in messages would break the log format.
        let line = format!(
            "{ts} {level:<5} [aodm #{}] {}\n",
            self.run,
            msg.replace('\n', " | ")
        );
        if let Some(dir) = self.path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Ok(mut f) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = f.write_all(line.as_bytes());
        }
    }
}

/// Size-based rotation: `adx.log` -> `.1` ... `.keep` once it reaches `max_bytes`.
pub fn rotate(path: &Path, max_bytes: u64, keep: u64) -> std::io::Result<()> {
    match fs::metadata(path) {
        Ok(m) if m.len() >= max_bytes => shift(path, keep),
        _ => Ok(()),
    }
}

/// Keep previous generations of a log (adx_log_shift): `f` -> `f.1`, `f.1` -> `f.2`, ...
pub fn shift(path: &Path, keep: u64) -> std::io::Result<()> {
    if !path.exists() || keep == 0 {
        return Ok(());
    }
    let generation = |n: u64| PathBuf::from(format!("{}.{n}", path.display()));
    for n in (1..keep).rev() {
        if generation(n).exists() {
            fs::rename(generation(n), generation(n + 1))?;
        }
    }
    fs::rename(path, generation(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_format_matches_adx() {
        let dir = tempfile::tempdir().unwrap();
        let log = Logger::to_file(dir.path().join("adx.log"), "42");
        log.info("saved image\nwith newline");
        log.warn("w");
        let text = fs::read_to_string(log.path()).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        // 2026-11-06 01:02:03 INFO  [aodm #42] saved image | with newline
        let l = lines[0];
        assert_eq!(&l[4..5], "-");
        assert_eq!(&l[10..11], " ");
        assert_eq!(&l[19..], " INFO  [aodm #42] saved image | with newline");
        assert!(lines[1].ends_with(" WARN  [aodm #42] w"));
    }

    #[test]
    fn shift_keeps_generations() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("e.log");
        for i in 0..4 {
            fs::write(&f, format!("run{i}")).unwrap();
            shift(&f, 2).unwrap();
        }
        assert!(!f.exists());
        assert_eq!(
            fs::read_to_string(dir.path().join("e.log.1")).unwrap(),
            "run3"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("e.log.2")).unwrap(),
            "run2"
        );
        assert!(!dir.path().join("e.log.3").exists());
    }

    #[test]
    fn rotate_only_above_limit() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("adx.log");
        fs::write(&f, "small").unwrap();
        rotate(&f, 1024, 5).unwrap();
        assert!(f.exists());
        rotate(&f, 3, 5).unwrap();
        assert!(!f.exists() && dir.path().join("adx.log.1").exists());
    }
}
