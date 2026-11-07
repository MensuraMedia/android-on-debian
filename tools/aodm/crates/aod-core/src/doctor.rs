//! `aodm doctor`: self-check of the manager and what it depends on. Read-only.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::avd::{self, Image};
use crate::images::{self, cfg_dir};
use crate::lifecycle::Ctx;
use crate::registry::{self, Registry};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Ok,
    Warn,
    Fail,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Ok => "ok",
            Level::Warn => "warn",
            Level::Fail => "FAIL",
        }
    }
}

pub struct Check {
    pub name: &'static str,
    pub level: Level,
    pub detail: String,
}

fn check(name: &'static str, level: Level, detail: impl Into<String>) -> Check {
    Check {
        name,
        level,
        detail: detail.into(),
    }
}

/// Filesystems that lose permissions, symlinks or sparse files (statfs magic numbers).
const NOT_POSIX: &[(i64, &str)] = &[
    (0x4d44, "FAT"),
    (0x2011_BAB0, "exFAT"),
    (0x6573_5546, "FUSE (e.g. NTFS)"),
];

/// Runs every check. `rev` is the git revision this aodm was built from.
pub fn run(ctx: &Ctx, imgs: &[Image], rev: &str) -> Vec<Check> {
    let env = ctx.env;
    let mut out = Vec::new();

    let problems = crate::config::validate(env);
    out.push(if problems.is_empty() {
        check(
            "config",
            Level::Ok,
            "every setting valid (aodm config validate)",
        )
    } else {
        check("config", Level::Fail, problems.join("; "))
    });

    out.push(match env.get("ADX_ROOT") {
        "" => check(
            "framework",
            Level::Warn,
            "not found (ADX_ROOT): using built-in fallbacks",
        ),
        root => {
            let head = Command::new("git")
                .args(["-C", root, "describe", "--always", "--dirty", "--abbrev=7"])
                // Read-only: --dirty would otherwise refresh .git/index under index.lock.
                .env("GIT_OPTIONAL_LOCKS", "0")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
            match head {
                Some(h) if h.trim_end_matches("-dirty") == rev.trim_end_matches("-dirty") => check(
                    "framework",
                    Level::Ok,
                    format!("{root} at {h}, aodm built from it"),
                ),
                Some(_) if rev == "unknown" => check(
                    "framework",
                    Level::Warn,
                    "aodm was built outside git: can't tell if it matches (adx devices install)",
                ),
                Some(h) => check(
                    "framework",
                    Level::Warn,
                    format!("{root} is at {h}, aodm was built from {rev}: run adx devices install"),
                ),
                None => check(
                    "framework",
                    Level::Ok,
                    format!("{root} (not a git checkout)"),
                ),
            }
        }
    });

    let sdk = env.path("ANDROID_HOME");
    let tools = [
        ("emulator", sdk.join("emulator/emulator")),
        ("adb", sdk.join("platform-tools/adb")),
    ];
    let missing: Vec<String> = tools
        .iter()
        .filter(|(_, p)| !is_exec(p))
        .map(|(n, p)| format!("{n} ({})", p.display()))
        .collect();
    out.push(if missing.is_empty() {
        check(
            "sdk",
            Level::Ok,
            format!("emulator and adb in {}", sdk.display()),
        )
    } else {
        check(
            "sdk",
            Level::Fail,
            format!("missing: {} (adx up)", missing.join(", ")),
        )
    });

    out.push(
        match fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/kvm")
        {
            Ok(_) => check("kvm", Level::Ok, "/dev/kvm usable"),
            Err(e) => check(
                "kvm",
                Level::Fail,
                format!("/dev/kvm: {e} (emulators would be very slow or fail; adx doctor)"),
            ),
        },
    );

    out.push(match cfg_dir(env, "ADX_AOD_HOME") {
        Err(e) => check("registry", Level::Fail, e.to_string()),
        Ok(home) => match Registry::load(&home) {
            Ok(r) if r.schema > registry::SCHEMA => check(
                "registry",
                Level::Fail,
                format!(
                    "schema {} is newer than this aodm ({})",
                    r.schema,
                    registry::SCHEMA
                ),
            ),
            Ok(r) => {
                let stale: Vec<&str> = r
                    .images
                    .keys()
                    .filter(|n| !imgs.iter().any(|i| &i.name == *n))
                    .map(String::as_str)
                    .collect();
                if stale.is_empty() {
                    check("registry", Level::Ok, format!("{} entries", r.images.len()))
                } else {
                    check(
                        "registry",
                        Level::Warn,
                        format!("entries without an image: {}", stale.join(", ")),
                    )
                }
            }
            Err(e) => check("registry", Level::Fail, e.to_string()),
        },
    });

    let broken: Vec<String> = imgs
        .iter()
        .filter_map(|i| i.problem.as_ref().map(|p| format!("{}: {p}", i.name)))
        .collect();
    out.push(if broken.is_empty() {
        check(
            "images",
            Level::Ok,
            format!("{} image(s) readable", imgs.len()),
        )
    } else {
        check("images", Level::Warn, broken.join("; "))
    });

    // A held lock with no emulator: another aodm is changing the image right now, or
    // something outside aodm holds it.
    let procs = crate::proc::emulators();
    let busy: Vec<&str> = imgs
        .iter()
        .filter(|i| i.problem.is_none() && crate::proc::running(&procs, &i.name).is_none())
        .filter(|i| locked(&i.dir.join("multiinstance.lock")))
        .map(|i| i.name.as_str())
        .collect();
    out.push(if busy.is_empty() {
        check("locks", Level::Ok, "no image held without an emulator")
    } else {
        check(
            "locks",
            Level::Warn,
            format!(
                "held but not running (another aodm run?): {}",
                busy.join(", ")
            ),
        )
    });

    out.push(match cfg_dir(env, "ADX_AOD_SAVES_DIR") {
        Err(e) => check("saves", Level::Fail, e.to_string()),
        Ok(p) => {
            let at = p.ancestors().find(|a| a.exists()).unwrap_or(Path::new("/"));
            match rustix::fs::statfs(at) {
                Ok(s) => match NOT_POSIX.iter().find(|(m, _)| *m == s.f_type) {
                    Some((_, n)) => check(
                        "saves",
                        Level::Fail,
                        format!(
                            "{} is on {n}: saves need a POSIX filesystem (ext4, xfs, btrfs)",
                            p.display()
                        ),
                    ),
                    None => check(
                        "saves",
                        Level::Ok,
                        format!("{} (POSIX filesystem)", p.display()),
                    ),
                },
                Err(e) => check("saves", Level::Warn, format!("{}: {e}", at.display())),
            }
        }
    });

    let biggest = imgs.iter().map(Image::disk_bytes).max().unwrap_or(0);
    let low: Vec<String> = ["ANDROID_AVD_HOME", "ADX_AOD_SAVES_DIR"]
        .iter()
        .filter_map(|k| {
            let free = images::free(&env.path(k)).ok()?;
            (free < biggest + images::MARGIN).then(|| {
                format!(
                    "{k}: {} free, the largest image needs {}",
                    avd::human(free),
                    avd::human(biggest)
                )
            })
        })
        .collect();
    out.push(if low.is_empty() {
        check(
            "space",
            Level::Ok,
            "room for a save or clone of the largest image",
        )
    } else {
        check("space", Level::Warn, low.join("; "))
    });

    let dirs = [cfg_dir(env, "ANDROID_AVD_HOME").ok()]
        .into_iter()
        .flatten()
        .chain(
            cfg_dir(env, "ADX_AOD_SAVES_DIR")
                .ok()
                .and_then(|s| fs::read_dir(s).ok())
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path()),
        );
    let dead = dirs
        .flat_map(|d| images::leftovers(&d))
        .filter(|(_, alive)| !alive)
        .count();
    out.push(if dead == 0 {
        check(
            "leftovers",
            Level::Ok,
            "no work folders of interrupted runs",
        )
    } else {
        check(
            "leftovers",
            Level::Warn,
            format!("{dead} work folder(s) of interrupted runs (aodm gc)"),
        )
    });

    let worst = out.iter().map(|c| c.level).max().unwrap_or(Level::Ok);
    ctx.log.info(&format!(
        "doctor: {} check(s), worst {}",
        out.len(),
        worst.as_str()
    ));
    out
}

/// Someone holds a flock on `p`, read from /proc/locks (doctor must not take the lock:
/// an emulator starting at that moment would refuse to run).
fn locked(p: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let Ok(m) = fs::metadata(p) else {
        return false;
    };
    // By inode only: on btrfs, stat's device differs from the one /proc/locks prints.
    // A same-inode lock on another filesystem gives a false warning, never a change.
    let ino = format!(":{}", m.ino());
    fs::read_to_string("/proc/locks").is_ok_and(|t| {
        t.lines().any(|l| {
            l.split_whitespace()
                .any(|f| f.ends_with(&ino) && f.matches(':').count() == 2)
        })
    })
}

fn is_exec(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}
