//! Start budget: how many emulators may run, and whether host RAM allows more.
//! Running emulators are already counted in `MemAvailable`, so only new ones are added.
//! shortcut: an emulator started seconds ago hasn't touched most of its RAM yet, so
//! back-to-back separate `aodm start` runs can overcommit; start them in one run instead.

use crate::error::{Error, Result};

/// One image about to start: name, guest RAM in MiB, cores.
pub struct Want<'a> {
    pub name: &'a str,
    pub ram_mb: u64,
    pub cores: u64,
}

pub struct Host {
    pub running: usize,
    pub max_instances: u64,
    pub avail_mb: u64,
    pub reserve_mb: u64,
    pub threads: u64,
    /// Cores of the emulators already running.
    pub running_cores: u64,
}

/// `Err` when the start would exceed the instance limit or eat the RAM reserve;
/// `Ok(warnings)` otherwise (CPU oversubscription only warns: it slows, it doesn't fail).
pub fn check(host: &Host, want: &[Want]) -> Result<Vec<String>> {
    let total = host.running + want.len();
    if total as u64 > host.max_instances {
        return Err(Error::Refused(format!(
            "{} running + {} to start = {total}, above ADX_AOD_MAX_INSTANCES={} (stop one, or raise it in config/local.env)",
            host.running,
            want.len(),
            host.max_instances
        )));
    }
    let need: u64 = want.iter().map(|w| w.ram_mb).sum();
    if need + host.reserve_mb > host.avail_mb {
        return Err(Error::Refused(format!(
            "starting {} needs {need} MiB guest RAM; {} MiB available minus ADX_AOD_RAM_RESERVE_MB={} leaves {} MiB",
            names(want),
            host.avail_mb,
            host.reserve_mb,
            host.avail_mb.saturating_sub(host.reserve_mb)
        )));
    }
    let cores = host.running_cores + want.iter().map(|w| w.cores).sum::<u64>();
    Ok(if cores > host.threads {
        vec![format!(
            "{cores} emulator cores on {} host threads: expect slow boots",
            host.threads
        )]
    } else {
        Vec::new()
    })
}

fn names(want: &[Want]) -> String {
    want.iter().map(|w| w.name).collect::<Vec<_>>().join(", ")
}

/// `hw.ramSize` as MiB: `2048`, `2048M`, `2048MB`, `2G`, `2GB` (emulator syntax).
pub fn ram_mb(v: &str) -> Option<u64> {
    let v = v.trim().to_ascii_uppercase();
    let v = v.strip_suffix('B').unwrap_or(&v);
    if let Some(g) = v.strip_suffix('G') {
        return g.trim().parse::<u64>().ok().map(|g| g * 1024);
    }
    v.strip_suffix('M').unwrap_or(v).trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> Host {
        Host {
            running: 1,
            max_instances: 3,
            avail_mb: 8000,
            reserve_mb: 2048,
            threads: 8,
            running_cores: 4,
        }
    }

    fn w(name: &str) -> Want<'_> {
        Want {
            name,
            ram_mb: 2048,
            cores: 2,
        }
    }

    #[test]
    fn instance_limit() {
        assert!(check(&host(), &[w("a"), w("b")]).is_ok());
        let e = check(&host(), &[w("a"), w("b"), w("c")]).unwrap_err();
        assert!(e.to_string().contains("MAX_INSTANCES=3"), "{e}");
    }

    #[test]
    fn ram_reserve() {
        let h = Host {
            avail_mb: 5000,
            ..host()
        };
        assert!(check(&h, &[w("a")]).is_ok());
        let e = check(&h, &[w("a"), w("b")]).unwrap_err().to_string();
        assert!(e.contains("4096 MiB") && e.contains("leaves 2952"), "{e}");
    }

    #[test]
    fn cpu_only_warns() {
        let h = Host {
            running_cores: 7,
            ..host()
        };
        assert_eq!(check(&h, &[w("a")]).unwrap().len(), 1);
        assert!(check(&host(), &[w("a")]).unwrap().is_empty());
    }

    #[test]
    fn ram_sizes() {
        for (s, mb) in [
            ("2048", 2048),
            ("2048M", 2048),
            ("2048MB", 2048),
            ("2G", 2048),
            ("3gb", 3072),
        ] {
            assert_eq!(ram_mb(s), Some(mb), "{s}");
        }
        assert_eq!(ram_mb("lots"), None);
    }
}
