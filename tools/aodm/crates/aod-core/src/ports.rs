//! Console ports. adb finds emulators by itself only on console ports 5554..5584
//! (even; adb port = console + 1), so there are at most 16 slots.
//! `ports.lock` serialises allocation between concurrent aodm runs.

use std::fs::{self, File};
use std::net::{Ipv4Addr, TcpListener};
use std::path::Path;

use crate::error::{Error, Result};

pub const FIRST: u16 = 5554;
pub const LAST: u16 = 5584;

/// The usable console ports from `base` on.
pub fn slots(base: u16) -> Result<Vec<u16>> {
    if !(FIRST..=LAST).contains(&base) || !base.is_multiple_of(2) {
        return Err(Error::Config(format!(
            "ADX_AOD_BASE_PORT={base}: must be even and within {FIRST}..{LAST} (adb's range)"
        )));
    }
    Ok((base..=LAST).step_by(2).collect())
}

/// Both the console and the adb port are free on localhost right now.
pub fn bindable(port: u16) -> bool {
    [port, port + 1]
        .iter()
        .all(|p| TcpListener::bind((Ipv4Addr::LOCALHOST, *p)).is_ok())
}

/// Picks the console port for `image`.
/// - `reserved`: images with a fixed port (registry pin, adx's own AVD on ADX_EMU_PORT);
///   other images never take those ports, so the owner can always start.
/// - `used`: ports of running emulators (and ones started earlier in this run).
pub fn pick(
    image: &str,
    slots: &[u16],
    reserved: &[(String, u16)],
    used: &[u16],
    free: impl Fn(u16) -> bool,
) -> Result<u16> {
    if let Some((_, p)) = reserved.iter().find(|(n, _)| n == image) {
        if used.contains(p) || !free(*p) {
            return Err(Error::Refused(format!(
                "{image}: its fixed port {p} is in use (another emulator or program)"
            )));
        }
        return Ok(*p);
    }
    slots
        .iter()
        .copied()
        .find(|p| !used.contains(p) && !reserved.iter().any(|(_, r)| r == p) && free(*p))
        .ok_or_else(|| {
            Error::Refused(format!(
                "{image}: no free console port in {}..{} (adb sees at most 16 emulators)",
                slots.first().unwrap_or(&FIRST),
                LAST
            ))
        })
}

/// Exclusive lock on `$ADX_AOD_HOME/ports.lock`, released on drop.
pub struct PortLock(#[allow(dead_code)] File);

impl PortLock {
    pub fn acquire(aod_home: &Path) -> Result<PortLock> {
        fs::create_dir_all(aod_home).map_err(Error::io(aod_home))?;
        let path = aod_home.join("ports.lock");
        let f = File::create(&path).map_err(Error::io(&path))?;
        f.lock().map_err(Error::io(&path))?; // waits for a concurrent aodm start
        Ok(PortLock(f))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sixteen_slots() {
        assert_eq!(slots(5554).unwrap().len(), 16);
        assert_eq!(slots(5580).unwrap(), [5580, 5582, 5584]);
        assert!(slots(5555).is_err());
        assert!(slots(5586).is_err());
        assert!(slots(5000).is_err());
    }

    #[test]
    fn picks_around_used_and_reserved() {
        let s = slots(5554).unwrap();
        let res = vec![
            ("adx_api37".to_string(), 5554),
            ("pinned".to_string(), 5558),
        ];
        let all = |_| true;
        assert_eq!(pick("adx_api37", &s, &res, &[], all).unwrap(), 5554);
        assert_eq!(pick("pinned", &s, &res, &[], all).unwrap(), 5558);
        assert_eq!(pick("other", &s, &res, &[], all).unwrap(), 5556);
        assert_eq!(pick("other", &s, &res, &[5556], all).unwrap(), 5560);
        assert_eq!(
            pick("other", &s, &res, &[], |p| p != 5556).unwrap(),
            5560,
            "port taken by another program"
        );
        assert!(
            pick("adx_api37", &s, &res, &[5554], all).is_err(),
            "fixed port busy"
        );
        let full: Vec<u16> = s.clone();
        assert!(
            pick("x", &s, &[], &full, all)
                .unwrap_err()
                .to_string()
                .contains("16")
        );
    }

    #[test]
    fn lock_is_exclusive() {
        let d = tempfile::tempdir().unwrap();
        let _held = PortLock::acquire(d.path()).unwrap();
        let other = File::open(d.path().join("ports.lock")).unwrap();
        assert!(other.try_lock().is_err());
    }
}
