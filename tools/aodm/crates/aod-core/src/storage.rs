//! `aodm disk` (where the space goes) and `aodm gc` (remove what aodm left behind).
//! gc only removes things nobody uses: work folders of dead runs, empty save folders,
//! temp files of dead runs, and adx's rotated logs beyond `ADX_LOG_KEEP`. Never images
//! or saves. Each item is checked again just before it goes.

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::avd::{self, Image};
use crate::error::{Error, Result};
use crate::images::{self, cfg_dir};
use crate::lifecycle::Ctx;
use crate::saves;

pub struct ImageUse {
    pub image: String,
    pub bytes: u64,
    pub saves: usize,
    pub saves_bytes: u64,
}

pub struct FsFree {
    /// The settings whose folders live on this filesystem.
    pub keys: Vec<&'static str>,
    pub path: PathBuf,
    pub free: u64,
}

pub struct Disk {
    pub images: Vec<ImageUse>,
    pub filesystems: Vec<FsFree>,
    /// Hidden work folders: path, size, whether their run is still going.
    pub leftovers: Vec<(PathBuf, u64, bool)>,
}

const DIRS: &[&str] = &[
    "ANDROID_AVD_HOME",
    "ADX_AOD_SAVES_DIR",
    "ADX_AOD_HOME",
    "ADX_LOG_DIR",
];

/// Folders leftovers can be in: the AVD home and each image's saves folder.
fn work_dirs(ctx: &Ctx) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = cfg_dir(ctx.env, "ANDROID_AVD_HOME").into_iter().collect();
    if let Ok(s) = cfg_dir(ctx.env, "ADX_AOD_SAVES_DIR") {
        v.extend(
            fs::read_dir(&s)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir()),
        );
    }
    v
}

pub fn disk(ctx: &Ctx, imgs: &[Image]) -> Result<Disk> {
    let root = cfg_dir(ctx.env, "ADX_AOD_SAVES_DIR")?;
    let all = saves::list(&root, None)?;
    let images = imgs
        .iter()
        .map(|i| {
            let mine: Vec<_> = all.iter().filter(|s| s.image == i.name).collect();
            ImageUse {
                image: i.name.clone(),
                bytes: i.disk_bytes(),
                saves: mine.len(),
                saves_bytes: mine.iter().map(|s| avd::du(&s.path)).sum(),
            }
        })
        .collect();
    let mut filesystems: Vec<(u64, FsFree)> = Vec::new();
    for key in DIRS {
        let p = ctx.env.path(key);
        let Some(existing) = p.ancestors().find(|a| a.exists()) else {
            continue;
        };
        let dev = fs::metadata(existing).map(|m| m.dev()).unwrap_or(0);
        match filesystems.iter_mut().find(|(d, _)| *d == dev) {
            Some((_, f)) => f.keys.push(key),
            None => filesystems.push((
                dev,
                FsFree {
                    keys: vec![key],
                    free: images::free(&p)?,
                    path: existing.to_path_buf(),
                },
            )),
        }
    }
    let leftovers = work_dirs(ctx)
        .iter()
        .flat_map(|d| images::leftovers(d))
        .map(|(p, alive)| {
            let n = avd::du(&p);
            (p, n, alive)
        })
        .collect();
    Ok(Disk {
        images,
        filesystems: filesystems.into_iter().map(|(_, f)| f).collect(),
        leftovers,
    })
}

/// Something gc removes.
pub struct Garbage {
    pub path: PathBuf,
    pub bytes: u64,
    pub why: &'static str,
    pub kind: Kind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    WorkFolder,
    EmptyFolder,
    File,
}

fn dead_pid_suffix(name: &str, marker: &str) -> bool {
    name.strip_suffix(".tmp")
        .and_then(|n| n.rsplit_once(marker))
        .is_some_and(|(_, pid)| {
            !pid.is_empty()
                && pid.chars().all(|c| c.is_ascii_digit())
                && !Path::new(&format!("/proc/{pid}")).exists()
        })
}

/// What `aodm gc` would remove now.
pub fn gc_plan(ctx: &Ctx) -> Result<Vec<Garbage>> {
    let mut out = Vec::new();
    for d in work_dirs(ctx) {
        for (p, alive) in images::leftovers(&d) {
            if !alive {
                let bytes = avd::du(&p);
                out.push(Garbage {
                    path: p,
                    bytes,
                    why: "work folder of an interrupted run",
                    kind: Kind::WorkFolder,
                });
            }
        }
    }
    if let Ok(root) = cfg_dir(ctx.env, "ADX_AOD_SAVES_DIR") {
        for e in fs::read_dir(&root).into_iter().flatten().flatten() {
            let p = e.path();
            // Only folders aodm made (one per image name), and only while empty.
            let ours = avd::validate_name(&e.file_name().to_string_lossy()).is_ok();
            if ours && p.is_dir() && fs::read_dir(&p).is_ok_and(|mut r| r.next().is_none()) {
                out.push(Garbage {
                    path: p,
                    bytes: 0,
                    why: "empty saves folder",
                    kind: Kind::EmptyFolder,
                });
            }
        }
    }
    // Temp files of dead runs: `.images.toml.PID.tmp` (registry), `local.env.env.PID.tmp`.
    let mut temp_dirs: Vec<PathBuf> = cfg_dir(ctx.env, "ADX_AOD_HOME").into_iter().collect();
    // Not the framework's config folder from a sandbox: it is outside it.
    if let Ok(p) = crate::config::local_env(ctx.env)
        && ctx.env.get("ADX_AOD_SANDBOX").is_empty()
        && let Some(d) = p.parent()
    {
        temp_dirs.push(d.to_path_buf());
    }
    for d in temp_dirs {
        for e in fs::read_dir(&d).into_iter().flatten().flatten() {
            let f = e.file_name().to_string_lossy().to_string();
            if (f.starts_with(".images.toml.") || f.starts_with("local.env."))
                && dead_pid_suffix(&f, ".")
            {
                out.push(Garbage {
                    bytes: e.metadata().map(|m| m.len()).unwrap_or(0),
                    path: e.path(),
                    why: "temp file of an interrupted run",
                    kind: Kind::File,
                });
            }
        }
    }
    // The window's thumbnails: temp files of dead runs, and screens of images that are gone.
    if let Ok(home) = cfg_dir(ctx.env, "ADX_AOD_HOME") {
        let names: Vec<String> = crate::avd::scan(
            &ctx.env.path("ANDROID_AVD_HOME"),
            &ctx.env.android_user_home(),
        )
        .map(|v| v.into_iter().map(|i| i.name).collect())
        .unwrap_or_default();
        let scanned = !names.is_empty();
        for e in fs::read_dir(home.join("thumbs"))
            .into_iter()
            .flatten()
            .flatten()
        {
            let f = e.file_name().to_string_lossy().to_string();
            let why = if f.starts_with('.') && f.contains(".thumb.") && dead_pid_suffix(&f, ".") {
                "temp file of an interrupted run"
            } else if scanned
                && f.strip_suffix(".thumb")
                    .is_some_and(|n| !names.iter().any(|x| x == n))
            {
                "screen thumbnail of an image that no longer exists"
            } else {
                continue;
            };
            out.push(Garbage {
                bytes: e.metadata().map(|m| m.len()).unwrap_or(0),
                path: e.path(),
                why,
                kind: Kind::File,
            });
        }
    }
    // Rotated logs beyond ADX_LOG_KEEP (adx.log.N, emulator-*.log.N).
    let keep = ctx.env.num("ADX_LOG_KEEP", 5);
    if let Ok(dir) = cfg_dir(ctx.env, "ADX_LOG_DIR") {
        for e in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let f = e.file_name().to_string_lossy().to_string();
            // Only adx's own logs: adx.log.N and emulator-NAME.log.N.
            let old = f
                .rsplit_once(".log.")
                .filter(|(base, _)| *base == "adx" || base.starts_with("emulator-"))
                .and_then(|(_, n)| n.parse::<u64>().ok())
                .is_some_and(|n| n > keep);
            if old && e.path().is_file() {
                out.push(Garbage {
                    bytes: e.metadata().map(|m| m.len()).unwrap_or(0),
                    path: e.path(),
                    why: "rotated log beyond ADX_LOG_KEEP",
                    kind: Kind::File,
                });
            }
        }
    }
    Ok(out)
}

/// Removes what `gc_plan` listed; returns bytes freed. Stops at the first failure.
pub fn gc(ctx: &Ctx, plan: &[Garbage]) -> Result<u64> {
    let mut freed = 0;
    for g in plan {
        let r = match g.kind {
            // Things may have changed since the plan: an empty folder that filled up
            // (a save started, a rename landed) fails remove_dir and is kept.
            Kind::EmptyFolder => match fs::remove_dir(&g.path) {
                Err(e) if g.path.exists() => {
                    ctx.log
                        .info(&format!("gc: kept {} ({e})", g.path.display()));
                    continue;
                }
                r => r,
            },
            Kind::WorkFolder => {
                let parent = g.path.parent().unwrap_or(Path::new("/"));
                if !images::leftovers(parent).contains(&(g.path.clone(), false)) {
                    continue;
                }
                fs::remove_dir_all(&g.path)
            }
            Kind::File => fs::remove_file(&g.path),
        };
        r.map_err(Error::io(&g.path))?;
        freed += g.bytes;
        ctx.log.info(&format!(
            "gc: removed {} ({}, {})",
            g.path.display(),
            g.why,
            avd::human(g.bytes)
        ));
    }
    if !plan.is_empty() {
        ctx.log.audit(
            "gc",
            "-",
            &format!("{} item(s), {} freed", plan.len(), avd::human(freed)),
        );
    }
    Ok(freed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_suffix_needs_a_dead_pid() {
        assert!(dead_pid_suffix(".images.toml.999999999.tmp", "."));
        let me = format!(".images.toml.{}.tmp", std::process::id());
        assert!(!dead_pid_suffix(&me, "."));
        assert!(!dead_pid_suffix(".images.toml.tmp", "."));
        assert!(!dead_pid_suffix(".images.toml.12x.tmp", "."));
    }
}
