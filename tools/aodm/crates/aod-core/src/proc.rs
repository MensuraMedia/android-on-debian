//! Running emulators, found by reading `/proc/*/cmdline` and matching the qemu process
//! itself (a shell running `pgrep -f '...-avd NAME'` would match its own command line).

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmuProc {
    pub pid: u32,
    pub avd: String,
    /// Console port from `-port`; `None` when the emulator picked one itself.
    pub port: Option<u16>,
}

impl EmuProc {
    /// adb serial, when the port is known.
    pub fn serial(&self) -> Option<String> {
        self.port.map(|p| format!("emulator-{p}"))
    }
}

/// Parses one NUL-separated command line; `Some` only for a qemu emulator process.
pub fn parse_cmdline(pid: u32, raw: &[u8]) -> Option<EmuProc> {
    let args: Vec<&str> = raw
        .split(|b| *b == 0)
        .filter(|a| !a.is_empty())
        .filter_map(|a| std::str::from_utf8(a).ok())
        .collect();
    let exe = Path::new(args.first()?).file_name()?.to_str()?;
    if !exe.starts_with("qemu-system") {
        return None;
    }
    let after = |flag: &str| {
        args.iter()
            .position(|a| *a == flag)
            .and_then(|i| args.get(i + 1))
            .copied()
    };
    // `-avd NAME`, or the launcher's shorthand `@NAME` (kept in the qemu argv).
    let avd = after("-avd").or_else(|| args.iter().skip(1).find_map(|a| a.strip_prefix('@')))?;
    // `-port N`, or `-ports CONSOLE,ADB`.
    let port = after("-port")
        .or_else(|| after("-ports").and_then(|p| p.split(',').next()))
        .and_then(|p| p.parse().ok());
    Some(EmuProc {
        pid,
        avd: avd.to_string(),
        port,
    })
}

/// Running emulators owned by the current user (another user's emulator with the
/// same AVD name is not "our image running").
pub fn emulators() -> Vec<EmuProc> {
    let me = fs::metadata("/proc/self").map(|m| m.uid()).ok();
    emulators_in(Path::new("/proc"), me)
}

/// Emulators under `proc_root`, optionally only those owned by `uid`.
pub fn emulators_in(proc_root: &Path, uid: Option<u32>) -> Vec<EmuProc> {
    let Ok(dir) = fs::read_dir(proc_root) else {
        return Vec::new();
    };
    let mut found: Vec<EmuProc> = dir
        .flatten()
        .filter_map(|e| {
            let pid: u32 = e.file_name().to_str()?.parse().ok()?;
            if let Some(want) = uid
                && e.metadata().ok()?.uid() != want
            {
                return None;
            }
            parse_cmdline(pid, &fs::read(e.path().join("cmdline")).ok()?)
        })
        .collect();
    found.sort_by(|a, b| a.avd.cmp(&b.avd).then(a.pid.cmp(&b.pid)));
    found
}

/// The running emulator for an image, if any.
pub fn running<'a>(procs: &'a [EmuProc], image: &str) -> Option<&'a EmuProc> {
    procs.iter().find(|p| p.avd == image)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(args: &[&str]) -> Vec<u8> {
        args.iter().flat_map(|a| a.bytes().chain([0])).collect()
    }

    #[test]
    fn parses_real_emulator_cmdline() {
        let line = raw(&[
            "/home/u/Android/Sdk/emulator/qemu/linux-x86_64/qemu-system-x86_64",
            "-avd",
            "adx_api37",
            "-gpu",
            "software",
            "-no-boot-anim",
            "-port",
            "5554",
        ]);
        let p = parse_cmdline(97018, &line).unwrap();
        assert_eq!(
            p,
            EmuProc {
                pid: 97018,
                avd: "adx_api37".into(),
                port: Some(5554)
            }
        );
        assert_eq!(p.serial().as_deref(), Some("emulator-5554"));
    }

    #[test]
    fn ignores_shells_and_other_programs() {
        assert!(
            parse_cmdline(
                1,
                &raw(&["/bin/bash", "-c", "pgrep -f 'qemu-system.*-avd adx_api37'"])
            )
            .is_none()
        );
        assert!(
            parse_cmdline(2, &raw(&["emulator", "-avd", "x"])).is_none(),
            "the launcher is not the VM"
        );
        assert!(
            parse_cmdline(3, &raw(&["qemu-system-x86_64", "-m", "2048"])).is_none(),
            "qemu without -avd"
        );
        let p = parse_cmdline(4, &raw(&["qemu-system-x86_64-headless", "-avd", "a.b"])).unwrap();
        assert_eq!((p.avd.as_str(), p.port), ("a.b", None));
    }

    #[test]
    fn launcher_shorthand_and_ports() {
        let p = parse_cmdline(
            5,
            &raw(&["qemu-system-x86_64", "@pixel", "-ports", "5556,5557"]),
        )
        .unwrap();
        assert_eq!((p.avd.as_str(), p.port), ("pixel", Some(5556)));
    }

    #[test]
    fn scans_a_fake_proc_tree() {
        let root = tempfile::tempdir().unwrap();
        for (pid, args) in [
            (
                "100",
                vec!["qemu-system-x86_64", "-avd", "b", "-port", "5556"],
            ),
            ("42", vec!["qemu-system-x86_64", "-avd", "a"]),
            ("7", vec!["bash"]),
        ] {
            fs::create_dir(root.path().join(pid)).unwrap();
            fs::write(root.path().join(pid).join("cmdline"), raw(&args)).unwrap();
        }
        fs::create_dir(root.path().join("self")).unwrap();
        let found = emulators_in(root.path(), None);
        let mine = fs::metadata(root.path().join("100")).unwrap().uid();
        assert_eq!(
            emulators_in(root.path(), Some(mine)).len(),
            2,
            "own processes kept"
        );
        assert!(
            emulators_in(root.path(), Some(mine + 1)).is_empty(),
            "other users' processes dropped"
        );
        assert_eq!(
            found.iter().map(|p| p.avd.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(running(&found, "b").and_then(|p| p.port), Some(5556));
        assert!(running(&found, "c").is_none());
    }
}
