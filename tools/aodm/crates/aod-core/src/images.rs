//! Clone, rename and delete images, and the copy-in step saves reuse. Every change
//! holds the image's `multiinstance.lock`: the emulator takes the same flock, so it
//! cannot start (or be running) while aodm copies, moves or removes the folder.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::avd::{self, Image};
use crate::cmd;
use crate::env::Env;
use crate::error::{Error, Result};
use crate::ini::IniDoc;
use crate::lifecycle::Ctx;
use crate::registry::Registry;

/// Free space kept on top of a copy's size.
pub const MARGIN: u64 = 1 << 30;
/// Copying or hashing a 10 GB image takes seconds to minutes; this only catches a hang.
pub const SLOW: Duration = Duration::from_secs(3600);

/// Files the emulator writes per run (locks, paths of the current run): never copied.
const TRANSIENT: &[&str] = &[
    "multiinstance.lock",
    "hardware-qemu.ini.lock",
    "hardware-qemu.ini",
    "emu-launch-params.txt",
    "bootcompleted.ini",
    "read-snapshot.txt",
    "snapshot.trace",
    "tmpAdbCmds",
];

/// The quick-boot snapshot: holds the folder's absolute path, so it only survives in place.
const SNAPSHOT: &str = "snapshots/default_boot";

/// Exclusive hold on an image folder, released on drop.
pub struct ImageLock(#[allow(dead_code)] File);

/// Takes `<dir>/multiinstance.lock`; refuses if the emulator (or another aodm) holds it.
pub fn lock(name: &str, dir: &Path) -> Result<ImageLock> {
    let path = dir.join("multiinstance.lock");
    let f = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(Error::io(&path))?;
    match f.try_lock() {
        Ok(()) => Ok(ImageLock(f)),
        Err(fs::TryLockError::WouldBlock) => Err(Error::Refused(format!(
            "{name} is running or busy (its multiinstance.lock is held): stop it first (aodm stop {name})"
        ))),
        Err(fs::TryLockError::Error(e)) => Err(Error::Io { path, source: e }),
    }
}

/// Everything a change to `img` must hold: its folder is a real directory (not a symlink:
/// copies would copy the link and edit the live folder through it), no emulator process
/// runs it (also one started with `-read-only`, which skips the lock), and its lock.
pub fn hold(img: &Image) -> Result<ImageLock> {
    if let Some(p) = &img.problem {
        return Err(Error::Refused(format!("{} is broken: {p}", img.name)));
    }
    if fs::symlink_metadata(&img.dir).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(Error::Refused(format!(
            "{}: {} is a symlink ({}); point path= in {} at the real folder first",
            img.name,
            img.dir.display(),
            fs::read_link(&img.dir).unwrap_or_default().display(),
            img.ini_path.display()
        )));
    }
    if let Some(p) = crate::proc::running(&crate::proc::emulators(), &img.name) {
        return Err(Error::Refused(format!(
            "{} is running (pid {}): stop it first (aodm stop {})",
            img.name, p.pid, img.name
        )));
    }
    lock(&img.name, &img.dir)
}

/// A configured directory aodm writes to: must be absolute (a relative or quoted `~`
/// value would put gigabytes under the current directory).
pub fn cfg_dir(env: &Env, key: &str) -> Result<PathBuf> {
    let p = env.path(key);
    if !p.is_absolute() {
        return Err(Error::Config(format!(
            "{key}={:?} must be an absolute path",
            env.get(key)
        )));
    }
    Ok(p)
}

/// Fails before any disk change if the registry can't be written afterwards.
fn registry_ok(env: &Env) -> Result<()> {
    Registry::writable(&cfg_dir(env, "ADX_AOD_HOME")?)
}

/// Leftovers of interrupted aodm runs in `dir` (hidden `.…partial-PID`, `.restore-PID`,
/// `.old-PID`, `.deleting-…-PID`) whose process is gone: removed and logged.
pub fn sweep(ctx: &Ctx, dir: &Path) {
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        let f = e.file_name().to_string_lossy().to_string();
        let ours = f.starts_with('.')
            && [".partial-", ".restore-", ".old-", ".deleting-"]
                .iter()
                .any(|m| f.contains(m));
        let pid = f.rsplit_once('-').map(|(_, p)| p).unwrap_or("");
        if ours
            && !pid.is_empty()
            && pid.chars().all(|c| c.is_ascii_digit())
            && !Path::new(&format!("/proc/{pid}")).exists()
        {
            let p = e.path();
            let size = avd::human(avd::du(&p));
            match fs::remove_dir_all(&p) {
                Ok(()) => ctx.log.warn(&format!(
                    "removed leftover of an interrupted run: {} ({size})",
                    p.display()
                )),
                Err(err) => ctx
                    .log
                    .warn(&format!("cannot remove leftover {}: {err}", p.display())),
            }
        }
    }
}

/// Sweeps the AVD home and every image's saves folder.
pub fn sweep_all(ctx: &Ctx) {
    if let Ok(home) = cfg_dir(ctx.env, "ANDROID_AVD_HOME") {
        sweep(ctx, &home);
    }
    if let Ok(saves) = cfg_dir(ctx.env, "ADX_AOD_SAVES_DIR") {
        for e in fs::read_dir(&saves).into_iter().flatten().flatten() {
            sweep(ctx, &e.path());
        }
    }
}

/// Flushes the filesystem holding `p`, so a copy is on disk before the original goes.
pub fn sync(p: &Path) -> Result<()> {
    let f = File::open(p).map_err(Error::io(p))?;
    rustix::fs::syncfs(&f).map_err(|e| Error::Io {
        path: p.to_path_buf(),
        source: e.into(),
    })
}

/// Bytes free for an unprivileged user on the filesystem holding `path` (or its parent).
pub fn free(path: &Path) -> Result<u64> {
    let p = path
        .ancestors()
        .find(|p| p.exists())
        .unwrap_or(Path::new("/"));
    rustix::fs::statvfs(p)
        .map(|s| s.f_bavail * s.f_frsize)
        .map_err(|e| Error::Io {
            path: p.to_path_buf(),
            source: e.into(),
        })
}

/// Refuses unless `need` bytes (+ margin) fit where `dest` will be.
pub fn ensure_space(dest: &Path, need: u64) -> Result<()> {
    let have = free(dest)?;
    if have < need + MARGIN {
        return Err(Error::Refused(format!(
            "not enough space for {}: need {} + 1.0G margin, {} free",
            dest.display(),
            avd::human(need),
            avd::human(have)
        )));
    }
    Ok(())
}

/// `cp --archive --sparse=always --reflink=auto SRC DST` (DST must not exist), synced.
/// SRC must be a real folder: `cp -a` would copy a symlink as a link.
pub fn copy_tree(ctx: &Ctx, src: &Path, dst: &Path) -> Result<()> {
    if !fs::symlink_metadata(src).is_ok_and(|m| m.is_dir()) {
        return Err(Error::Refused(format!(
            "{} is not a real folder (symlink?): not copied",
            src.display()
        )));
    }
    let t0 = Instant::now();
    let out = cmd::run_attached(
        Command::new("cp")
            .args(["--archive", "--sparse=always", "--reflink=auto", "--"])
            .arg(src)
            .arg(dst),
        SLOW,
    )?;
    if !out.ok {
        return Err(Error::Refused(format!(
            "copy {} -> {} failed: {}",
            src.display(),
            dst.display(),
            out.stderr.trim()
        )));
    }
    sync(dst)?;
    ctx.log.info(&format!(
        "copied {} -> {} ({}) in {:.1} s",
        src.display(),
        dst.display(),
        avd::human(avd::du(dst)),
        t0.elapsed().as_secs_f64()
    ));
    Ok(())
}

/// Removes the per-run files from a copy, and the snapshot when the folder moved.
pub fn tidy(dir: &Path, drop_snapshot: bool) -> Result<()> {
    let snap = drop_snapshot.then_some(SNAPSHOT);
    for rel in TRANSIENT.iter().copied().chain(snap) {
        let p = dir.join(rel);
        let r = match fs::symlink_metadata(&p) {
            Ok(m) if m.is_dir() => fs::remove_dir_all(&p),
            Ok(_) => fs::remove_file(&p),
            Err(_) => continue,
        };
        r.map_err(Error::io(&p))?;
    }
    Ok(())
}

/// Points `config.ini` at the image's (new) name.
fn rename_config(dir: &Path, name: &str) -> Result<()> {
    let path = dir.join("config.ini");
    let mut c = IniDoc::load(&path)?;
    c.set("avd.id", name)?;
    c.set("avd.name", name)?;
    c.save(&path)
}

/// The `<name>.ini` the emulator reads: absolute `path=`, `path.rel=` when the folder
/// sits under the Android user home (as avdmanager writes it), `target=`.
fn write_ini(env: &Env, ini_path: &Path, dir: &Path, target: &str) -> Result<()> {
    let mut ini = IniDoc::parse("");
    ini.set("avd.ini.encoding", "UTF-8")?;
    ini.set("path", &dir.to_string_lossy())?;
    if let Ok(rel) = dir.strip_prefix(env.android_user_home()) {
        ini.set("path.rel", &rel.to_string_lossy())?;
    }
    ini.set("target", target)?;
    ini.save(ini_path)
}

/// A new image name must be valid and free (no `.ini`, no folder).
fn check_new(env: &Env, name: &str, folder: &Path) -> Result<()> {
    avd::validate_name(name)?;
    let ini = cfg_dir(env, "ANDROID_AVD_HOME")?.join(format!("{name}.ini"));
    if ini.exists() || fs::symlink_metadata(folder).is_ok() {
        return Err(Error::Refused(format!(
            "an image named {name} already exists ({})",
            if ini.exists() {
                ini
            } else {
                folder.to_path_buf()
            }
            .display()
        )));
    }
    Ok(())
}

/// Copies an image folder (`src`, nothing may run it) in as a new image `name`:
/// fresh `.ini`, `config.ini` renamed, per-run files and, if the path changes, the
/// quick-boot snapshot dropped (first boot is cold). Nothing is left behind on failure.
pub fn install(
    ctx: &Ctx,
    src: &Path,
    name: &str,
    target: &str,
    keep_snapshot: bool,
) -> Result<PathBuf> {
    let home = cfg_dir(ctx.env, "ANDROID_AVD_HOME")?;
    let dir = home.join(format!("{name}.avd"));
    check_new(ctx.env, name, &dir)?;
    registry_ok(ctx.env)?;
    fs::create_dir_all(&home).map_err(Error::io(&home))?;
    sweep(ctx, &home);
    ensure_space(&home, avd::du(src))?;
    let partial = home.join(format!(".{name}.avd.partial-{}", std::process::id()));
    let ini_path = home.join(format!("{name}.ini"));
    let result = (|| {
        (ctx.say)(&format!(
            "copying {} into {name}…",
            avd::human(avd::du(src))
        ));
        copy_tree(ctx, src, &partial)?;
        tidy(&partial, !keep_snapshot)?;
        rename_config(&partial, name)?;
        // Fails if the name was taken meanwhile (a non-empty folder is never replaced).
        fs::rename(&partial, &dir).map_err(Error::io(&dir))?;
        write_ini(ctx.env, &ini_path, &dir, target).inspect_err(|_| {
            let _ = fs::remove_dir_all(&dir);
        })
    })();
    if let Err(e) = &result {
        ctx.log.error(&format!("copy into {name} failed: {e}"));
        if let Err(c) = fs::remove_dir_all(&partial)
            && partial.exists()
        {
            ctx.log
                .warn(&format!("cannot remove {}: {c}", partial.display()));
        }
    }
    result.map(|()| dir)
}

fn refuse_adx_image(env: &Env, name: &str, what: &str) -> Result<()> {
    if name == env.get("ADX_AVD_NAME") {
        return Err(Error::Refused(format!(
            "{name} is adx's own image (ADX_AVD_NAME): {what} it would break `adx emulator`; clone it instead"
        )));
    }
    Ok(())
}

/// `aodm clone SRC NEW`: a full copy with a new name; SRC must be stopped.
pub fn clone(ctx: &Ctx, src: &Image, name: &str) -> Result<String> {
    let _held = hold(src)?;
    let target = src.ini.get("target").unwrap_or("").to_string();
    let t0 = Instant::now();
    install(ctx, &src.dir, name, &target, false)?;
    Registry::update(&cfg_dir(ctx.env, "ADX_AOD_HOME")?, |r| {
        r.images.entry(name.to_string()).or_default().origin = Some(src.name.clone());
    })?;
    let msg = format!(
        "cloned from {} in {:.0} s (first boot is cold; same Android ID as {} until identity policies, P5)",
        src.name,
        t0.elapsed().as_secs_f64(),
        src.name
    );
    ctx.log.info(&format!("clone {} -> {name}: ok", src.name));
    Ok(msg)
}

/// `aodm rename OLD NEW`: moves the folder, rewrites the `.ini` and `config.ini`, and
/// carries the registry entry and saves along. The snapshot is dropped (paths change).
pub fn rename(ctx: &Ctx, img: &Image, new: &str) -> Result<String> {
    refuse_adx_image(ctx.env, &img.name, "renaming")?;
    let parent = img.dir.parent().unwrap_or(Path::new("/"));
    let new_dir = parent.join(format!("{new}.avd"));
    check_new(ctx.env, new, &new_dir)?;
    registry_ok(ctx.env)?;
    let saves = cfg_dir(ctx.env, "ADX_AOD_SAVES_DIR")?;
    if saves.join(new).exists() {
        return Err(Error::Refused(format!(
            "saves for a former image {new} exist in {}: delete or move them first",
            saves.join(new).display()
        )));
    }
    let _held = hold(img)?;
    let target = img.ini.get("target").unwrap_or("").to_string();
    let new_ini = cfg_dir(ctx.env, "ANDROID_AVD_HOME")?.join(format!("{new}.ini"));
    let dir = new_dir;
    fs::rename(&img.dir, &dir).map_err(Error::io(&dir))?;
    let rest = (|| {
        // Not `tidy`: that would unlink the lock file this rename holds.
        let snap = dir.join(SNAPSHOT);
        if snap.exists() {
            fs::remove_dir_all(&snap).map_err(Error::io(&snap))?;
        }
        rename_config(&dir, new)?;
        write_ini(ctx.env, &new_ini, &dir, &target)?;
        fs::remove_file(&img.ini_path).map_err(Error::io(&img.ini_path))
    })();
    if let Err(e) = rest {
        // Put the folder back so the old name keeps working.
        let _ = fs::remove_file(&new_ini);
        let back = fs::rename(&dir, &img.dir).and_then(|()| {
            rename_config(&img.dir, &img.name).map_err(|e| std::io::Error::other(e.to_string()))
        });
        ctx.log.error(&format!("rename {} -> {new}: {e}", img.name));
        if let Err(b) = back {
            ctx.log.error(&format!(
                "rename rollback failed ({b}): move {} back to {} by hand",
                dir.display(),
                img.dir.display()
            ));
            return Err(Error::Refused(format!(
                "{e}; putting it back also failed ({b}): move {} back to {} by hand",
                dir.display(),
                img.dir.display()
            )));
        }
        return Err(e);
    }
    // The image is renamed now: later failures say what is left to do by hand.
    Registry::update(&cfg_dir(ctx.env, "ADX_AOD_HOME")?, |r| {
        if let Some(e) = r.images.remove(&img.name) {
            r.images.insert(new.to_string(), e);
        }
    })
    .map_err(|e| {
        Error::Refused(format!(
            "renamed, but the registry entry stayed under {}: {e}",
            img.name
        ))
    })?;
    if saves.join(&img.name).exists() {
        fs::rename(saves.join(&img.name), saves.join(new)).map_err(|e| {
            Error::Refused(format!(
                "renamed, but its saves stayed in {} ({e}): move them to {} by hand",
                saves.join(&img.name).display(),
                saves.join(new).display()
            ))
        })?;
    }
    ctx.log.info(&format!("rename {} -> {new}: ok", img.name));
    Ok(format!("renamed to {new} (next boot is cold)"))
}

/// `aodm delete NAME`: removes the `.ini`, the folder and the registry entry. Saves
/// are kept. Works on broken images too (that is how they get cleaned up).
pub fn delete(ctx: &Ctx, img: &Image, all: &[Image]) -> Result<String> {
    refuse_adx_image(ctx.env, &img.name, "deleting")?;
    registry_ok(ctx.env)?;
    if let Some(p) = crate::proc::running(&crate::proc::emulators(), &img.name) {
        return Err(Error::Refused(format!(
            "{} is running (pid {}): stop it first (aodm stop {})",
            img.name, p.pid, img.name
        )));
    }
    let link = fs::symlink_metadata(&img.dir)
        .is_ok_and(|m| m.file_type().is_symlink())
        .then(|| fs::read_link(&img.dir).unwrap_or_default());
    let real = |p: &Path| fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    // A corrupt `.ini` may point at another image's folder: never remove that one.
    let shared = all
        .iter()
        .any(|o| o.name != img.name && o.dir.exists() && real(&o.dir) == real(&img.dir));
    let folder = fs::symlink_metadata(&img.dir).is_ok_and(|m| m.is_dir()) && !shared;
    let _held = if folder {
        Some(lock(&img.name, &img.dir)?)
    } else {
        None
    };
    if img.ini_path.exists() || fs::symlink_metadata(&img.ini_path).is_ok() {
        fs::remove_file(&img.ini_path).map_err(Error::io(&img.ini_path))?;
    }
    let mut size = 0;
    if folder {
        size = avd::du(&img.dir);
        // Rename first: an interrupted removal leaves a hidden leftover the next run sweeps,
        // not a half-deleted folder that looks like an image.
        let parent = img.dir.parent().unwrap_or(Path::new("/"));
        let gone = parent.join(format!(".{}.avd.deleting-{}", img.name, std::process::id()));
        fs::rename(&img.dir, &gone).map_err(Error::io(&img.dir))?;
        fs::remove_dir_all(&gone).map_err(Error::io(&gone))?;
    } else if link.is_some() {
        // Only the link: the folder it points at may be on purpose elsewhere.
        fs::remove_file(&img.dir).map_err(Error::io(&img.dir))?;
    }
    Registry::update(&cfg_dir(ctx.env, "ADX_AOD_HOME")?, |r| {
        r.images.remove(&img.name);
    })?;
    let saves = crate::saves::list(&cfg_dir(ctx.env, "ADX_AOD_SAVES_DIR")?, Some(&img.name))
        .map(|l| l.len())
        .unwrap_or(0);
    ctx.log.info(&format!(
        "delete {}: ok ({} freed, folder {}, {saves} save(s) kept)",
        img.name,
        avd::human(size),
        if folder { "removed" } else { "left alone" }
    ));
    let mut msg = format!("deleted ({} freed)", avd::human(size));
    if shared {
        msg.push_str("; its folder belongs to another image and was left alone");
    }
    if let Some(t) = &link {
        msg.push_str(&format!(
            "; its folder was a symlink: removed the link, left {} alone",
            t.display()
        ));
    }
    if saves > 0 {
        msg.push_str(&format!(
            "; {saves} save(s) kept (aodm save open {} ID --as NAME)",
            img.name
        ));
    }
    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_refuses_while_held() {
        let d = tempfile::tempdir().unwrap();
        let held = lock("a", d.path()).unwrap();
        let e = lock("a", d.path()).err().unwrap();
        assert!(e.to_string().contains("aodm stop a"), "{e}");
        drop(held);
        assert!(lock("a", d.path()).is_ok());
    }

    #[test]
    fn tidy_drops_per_run_files() {
        let d = tempfile::tempdir().unwrap();
        for f in ["multiinstance.lock", "hardware-qemu.ini", "config.ini"] {
            fs::write(d.path().join(f), "x").unwrap();
        }
        fs::create_dir_all(d.path().join("tmpAdbCmds")).unwrap();
        fs::create_dir_all(d.path().join(SNAPSHOT)).unwrap();
        fs::write(d.path().join(SNAPSHOT).join("ram.bin"), "x").unwrap();
        tidy(d.path(), false).unwrap();
        assert!(d.path().join("config.ini").exists());
        assert!(!d.path().join("hardware-qemu.ini").exists());
        assert!(!d.path().join("tmpAdbCmds").exists());
        assert!(d.path().join(SNAPSHOT).exists(), "kept in place");
        tidy(d.path(), true).unwrap();
        assert!(!d.path().join(SNAPSHOT).exists());
    }

    #[test]
    fn space_check() {
        let d = tempfile::tempdir().unwrap();
        assert!(ensure_space(d.path(), 1).is_ok());
        assert!(ensure_space(d.path(), u64::MAX / 2).is_err());
    }
}
