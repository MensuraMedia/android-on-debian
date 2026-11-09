//! adb, always scoped to one serial (`-s emulator-<port>`) and always with a timeout.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::cmd::{self, Output};
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct Adb {
    pub bin: PathBuf,
}

impl Adb {
    pub fn new(android_home: &Path) -> Adb {
        Adb {
            bin: android_home.join("platform-tools/adb"),
        }
    }

    pub fn run(&self, serial: &str, args: &[&str], timeout: Duration) -> Result<Output> {
        cmd::run(
            Command::new(&self.bin).arg("-s").arg(serial).args(args),
            timeout,
        )
    }

    /// `device`, `offline`, `unauthorized`, or why adb can't see it.
    pub fn state(&self, serial: &str) -> String {
        match self.run(serial, &["get-state"], Duration::from_secs(5)) {
            Ok(o) if o.ok => o.stdout.trim().to_string(),
            Ok(o) => state_from_error(&o.stderr),
            Err(e) => e.to_string(),
        }
    }

    /// Android finished booting (`sys.boot_completed` = 1).
    pub fn booted(&self, serial: &str) -> bool {
        self.run(
            serial,
            &["shell", "getprop", "sys.boot_completed"],
            Duration::from_secs(10),
        )
        .is_ok_and(|o| o.ok && o.stdout.trim() == "1")
    }
}

/// adb reports states other than `device` as errors, e.g. `error: device unauthorized.`.
fn state_from_error(stderr: &str) -> String {
    let s = stderr.trim();
    for st in ["unauthorized", "offline", "not found"] {
        if s.contains(st) {
            return st.to_string();
        }
    }
    s.lines().next().unwrap_or("unknown").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn states_from_adb_errors() {
        assert_eq!(
            state_from_error(
                "error: device unauthorized.\nThis adb server's $ADB_VENDOR_KEYS is not set"
            ),
            "unauthorized"
        );
        assert_eq!(
            state_from_error("error: device 'emulator-5556' not found"),
            "not found"
        );
        assert_eq!(state_from_error(""), "unknown");
    }

    #[test]
    fn fake_adb_scoped_to_serial() {
        let d = tempfile::tempdir().unwrap();
        let bin = d.path().join("platform-tools/adb");
        std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
        std::fs::write(
            &bin,
            "#!/bin/sh\n[ \"$1 $2\" = '-s emulator-5556' ] || exit 9\nshift 2\ncase \"$*\" in\n  get-state) echo device ;;\n  'shell getprop sys.boot_completed') echo 1 ;;\n  *) exit 1 ;;\nesac\n",
        )
        .unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let adb = Adb::new(d.path());
        assert_eq!(adb.state("emulator-5556"), "device");
        assert!(adb.booted("emulator-5556"));
        assert!(!adb.booted("emulator-5558"));
    }
}
