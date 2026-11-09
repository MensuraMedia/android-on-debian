//! Full-copy saves: `$ADX_AOD_SAVES_DIR/<image>/<YYYYmmdd-HHMMSS>[-label]/` with
//! `avd/` (the image folder, quick-boot snapshot included), `image.ini` and
//! `manifest.toml` (versions, size and SHA-256 of every file). A save appears only
//! when complete (built in `.partial-<pid>`, then renamed). Pinned saves carry a
//! `pinned` file and don't count against `ADX_AOD_MAX_SAVES`. Never pruned automatically.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::avd::{self, Image};
use crate::cmd;
use crate::error::{Error, Result};
use crate::images::{self, SLOW};
use crate::lifecycle::Ctx;
use crate::registry::Registry;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileSum {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

/// Manifest format this aodm writes and reads.
pub const FORMAT: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(default)]
    pub format: u32,
    pub aodm: String,
    pub image: String,
    /// The image folder when saved: a restore elsewhere drops the snapshot.
    pub source: PathBuf,
    #[serde(default)]
    pub label: String,
    pub created: String,
    pub target: String,
    pub abi: String,
    pub tag: String,
    pub sysimg_revision: String,
    pub emulator: String,
    pub files: Vec<FileSum>,
}

#[derive(Debug)]
pub struct Save {
    pub image: String,
    pub id: String,
    pub path: PathBuf,
    pub pinned: bool,
    /// Unreadable manifest: the save is listed but can't be restored.
    pub manifest: std::result::Result<Manifest, String>,
}

impl Save {
    pub fn avd(&self) -> PathBuf {
        self.path.join("avd")
    }

    fn manifest(&self) -> Result<&Manifest> {
        let m = self.manifest.as_ref().map_err(|e| {
            Error::Refused(format!("save {}/{} is damaged: {e}", self.image, self.id))
        })?;
        if m.format > FORMAT {
            return Err(Error::Refused(format!(
                "save {}/{} has manifest format {} (this aodm reads {FORMAT}): update aodm",
                self.image, self.id, m.format
            )));
        }
        Ok(m)
    }
}

/// Saves of one image (or all), oldest first. In-progress `.partial-*` folders are skipped.
/// `image` must be a valid image name: never a path (`..`, absolute, empty).
pub fn list(root: &Path, image: Option<&str>) -> Result<Vec<Save>> {
    let names: Vec<String> = match image {
        Some(i) => {
            avd::validate_name(i)?;
            vec![i.to_string()]
        }
        None => dirs(root)
            .into_iter()
            .filter(|n| avd::validate_name(n).is_ok())
            .collect(),
    };
    let mut out = Vec::new();
    for img in names {
        for id in dirs(&root.join(&img)) {
            let path = root.join(&img).join(&id);
            let manifest = fs::read_to_string(path.join("manifest.toml"))
                .map_err(|e| e.to_string())
                .and_then(|t| toml::from_str(&t).map_err(|e| e.to_string()));
            out.push(Save {
                image: img.clone(),
                pinned: path.join("pinned").exists(),
                id,
                path,
                manifest,
            });
        }
    }
    Ok(out)
}

/// Sub-directory names, sorted, without hidden ones.
fn dirs(p: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(p)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| !n.starts_with('.'))
        .collect();
    v.sort();
    v
}

/// A save by id: exact, `latest`, or a unique prefix (e.g. the date).
pub fn find(root: &Path, image: &str, id: &str) -> Result<Save> {
    if id.is_empty() {
        return Err(Error::Invalid {
            what: "save id",
            detail: "empty (give an id, a prefix of one, or `latest`)".into(),
        });
    }
    let mut all = list(root, Some(image))?;
    let hit = if id == "latest" {
        all.pop()
    } else if let Some(i) = all.iter().position(|s| s.id == id) {
        Some(all.swap_remove(i))
    } else {
        let mut m: Vec<Save> = all.into_iter().filter(|s| s.id.starts_with(id)).collect();
        if m.len() > 1 {
            return Err(Error::Failed(format!(
                "'{id}' matches {} saves of {image}: {}",
                m.len(),
                m.iter()
                    .map(|s| s.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        m.pop()
    };
    hit.ok_or_else(|| {
        Error::Failed(format!(
            "{image} has no save '{id}' (aodm save list {image})"
        ))
    })
}

/// Labels: like image names, at most 32 characters.
fn validate_label(label: &str) -> Result<()> {
    if label.len() > 32 || avd::validate_name(label).is_err() {
        return Err(Error::Invalid {
            what: "save label",
            detail: format!("{label:?} (letters, digits, . _ -; max 32)"),
        });
    }
    Ok(())
}

/// Every regular file under `dir` with size and SHA-256 (coreutils `sha256sum`), sorted.
/// Symlinks and special files are refused: a save holds plain files only.
pub fn hash_tree(dir: &Path) -> Result<Vec<FileSum>> {
    let mut files = Vec::new();
    walk(dir, dir, &mut files)?;
    files.sort();
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let out = cmd::run_attached(
        Command::new("sha256sum")
            .args(["--binary", "--zero", "--"])
            .args(&files)
            .current_dir(dir),
        SLOW,
    )?;
    if !out.ok {
        return Err(Error::Failed(format!(
            "sha256sum failed: {}",
            out.stderr.trim()
        )));
    }
    let sums: Vec<&str> = out.stdout.split('\0').filter(|r| !r.is_empty()).collect();
    if sums.len() != files.len() {
        return Err(Error::Failed("sha256sum: unexpected output".into()));
    }
    files
        .into_iter()
        .zip(sums)
        .map(|(path, rec)| {
            let (hash, _) = rec.split_once(" *").unwrap_or((rec, ""));
            if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(Error::Refused(format!(
                    "sha256sum: unexpected output {rec:?} (GNU coreutils needed)"
                )));
            }
            let size = fs::metadata(dir.join(&path))
                .map_err(Error::io(dir.join(&path)))?
                .len();
            Ok(FileSum {
                path,
                size,
                sha256: hash.to_string(),
            })
        })
        .collect()
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
    for e in fs::read_dir(dir).map_err(Error::io(dir))? {
        let e = e.map_err(Error::io(dir))?;
        let p = e.path();
        let t = e.file_type().map_err(Error::io(&p))?;
        if t.is_dir() {
            walk(root, &p, out)?;
        } else if t.is_file() {
            let rel = p
                .strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .to_string();
            out.push(rel);
        } else {
            return Err(Error::Refused(format!(
                "{} is not a plain file (symlink or special file): aodm saves plain files only",
                p.display()
            )));
        }
    }
    Ok(())
}

fn sdk_version(ctx: &Ctx, rel: &str) -> String {
    avd::props(&ctx.env.path("ANDROID_HOME").join(rel), "Pkg.Revision")
        .unwrap_or_else(|| "?".into())
}

/// Non-pinned saves count against the limit.
fn counted(saves: &[Save]) -> usize {
    saves.iter().filter(|s| !s.pinned).count()
}

/// `aodm save create IMAGE [--label L]`: the image must be stopped (its lock is held
/// for the whole copy, so it can't be started meanwhile).
pub fn create(ctx: &Ctx, img: &Image, label: Option<&str>) -> Result<Save> {
    if let Some(p) = &img.problem {
        return Err(Error::Refused(format!("{} is broken: {p}", img.name)));
    }
    if let Some(l) = label {
        validate_label(l)?;
    }
    let root = images::cfg_dir(ctx.env, "ADX_AOD_SAVES_DIR")?;
    let max = ctx.env.num("ADX_AOD_MAX_SAVES", 3) as usize;
    let _held = images::hold(img)?;
    let existing = list(&root, Some(&img.name))?;
    if counted(&existing) >= max {
        return Err(Error::Refused(format!(
            "{} already has {} unpinned save(s) (ADX_AOD_MAX_SAVES={max}): `aodm save prune {}` or delete one first",
            img.name,
            counted(&existing),
            img.name
        )));
    }
    let base = root.join(&img.name);
    fs::create_dir_all(&base).map_err(Error::io(&base))?;
    images::sweep(ctx, &base);
    let size = avd::du(&img.dir);
    images::ensure_space(&base, size)?;
    // UTC: ids sort by time across DST changes and time zones (`latest`, prune order).
    let now = jiff::Timestamp::now();
    let stamp = now.strftime("%Y%m%d-%H%M%SZ").to_string();
    let id = match label {
        Some(l) => format!("{stamp}-{l}"),
        None => stamp,
    };
    let dest = base.join(&id);
    if dest.exists() {
        return Err(Error::Refused(format!(
            "save {id} already exists: wait a second"
        )));
    }
    let partial = base.join(format!(".partial-{}", std::process::id()));
    let t0 = Instant::now();
    let result = (|| {
        fs::create_dir(&partial).map_err(Error::io(&partial))?;
        (ctx.say)(&format!("saving {} ({})…", img.name, avd::human(size)));
        images::copy_tree(ctx, &img.dir, &partial.join("avd"))?;
        images::tidy(&partial.join("avd"), false)?;
        fs::copy(&img.ini_path, partial.join("image.ini")).map_err(Error::io(&img.ini_path))?;
        (ctx.say)("hashing the copy…");
        let files = hash_tree(&partial.join("avd"))?;
        let m = Manifest {
            format: FORMAT,
            aodm: env!("CARGO_PKG_VERSION").into(),
            image: img.name.clone(),
            source: img.dir.clone(),
            label: label.unwrap_or("").into(),
            created: now.to_string(),
            target: img.ini.get("target").unwrap_or("?").into(),
            abi: img.abi().into(),
            tag: img.tag().into(),
            sysimg_revision: img
                .sysimg_revision(&ctx.env.path("ANDROID_HOME"))
                .unwrap_or_else(|| "?".into()),
            emulator: sdk_version(ctx, "emulator/source.properties"),
            files,
        };
        let text =
            toml::to_string_pretty(&m).map_err(|e| Error::Config(format!("manifest: {e}")))?;
        let mp = partial.join("manifest.toml");
        fs::write(&mp, text).map_err(Error::io(&mp))?;
        fs::File::open(&mp)
            .and_then(|f| f.sync_all())
            .map_err(Error::io(&mp))?;
        fs::rename(&partial, &dest).map_err(Error::io(&dest))?;
        fs::File::open(&base)
            .and_then(|f| f.sync_all())
            .map_err(Error::io(&base))
    })();
    if let Err(e) = result {
        let _ = fs::remove_dir_all(&partial);
        ctx.log.error(&format!("save {}: {e}", img.name));
        return Err(e);
    }
    ctx.log.audit(
        "save",
        &img.name,
        &format!(
            "{id} ({}) in {:.1} s",
            avd::human(avd::du(&dest)),
            t0.elapsed().as_secs_f64()
        ),
    );
    find(&root, &img.name, &id)
}

/// Re-hashes the save and compares with its manifest: `Ok(problems)` (empty = intact).
pub fn verify(ctx: &Ctx, save: &Save) -> Result<Vec<String>> {
    let m = save.manifest()?;
    let t0 = Instant::now();
    let now = hash_tree(&save.avd())?;
    let mut problems = Vec::new();
    for f in &m.files {
        match now.iter().find(|n| n.path == f.path) {
            None => problems.push(format!("missing: {}", f.path)),
            Some(n) if n.size != f.size || n.sha256 != f.sha256 => {
                problems.push(format!("changed: {}", f.path))
            }
            Some(_) => {}
        }
    }
    for n in &now {
        if !m.files.iter().any(|f| f.path == n.path) {
            problems.push(format!("unexpected: {}", n.path));
        }
    }
    ctx.log.info(&format!(
        "verify {}/{}: {} in {:.1} s",
        save.image,
        save.id,
        if problems.is_empty() {
            "intact".to_string()
        } else {
            format!("{} problem(s)", problems.len())
        },
        t0.elapsed().as_secs_f64()
    ));
    Ok(problems)
}

fn verified<'s>(ctx: &Ctx, save: &'s Save) -> Result<&'s Manifest> {
    (ctx.say)(&format!("verifying save {}…", save.id));
    let problems = verify(ctx, save)?;
    if !problems.is_empty() {
        return Err(Error::Refused(format!(
            "save {}/{} is damaged ({}); nothing changed",
            save.image,
            save.id,
            problems.join(", ")
        )));
    }
    save.manifest()
}

/// Warns when the system image changed since the save (the snapshot won't load).
fn sysimg_warning(ctx: &Ctx, img_rev: Option<String>, m: &Manifest) -> Option<String> {
    let now = img_rev.unwrap_or_else(|| "?".into());
    (now != m.sysimg_revision).then(|| {
        let w = format!(
            "system image revision {} at save time, {now} now: expect a cold boot",
            m.sysimg_revision
        );
        ctx.log.warn(&w);
        w
    })
}

/// `aodm save restore IMAGE ID`: replaces the image's folder with the save (verified
/// first). The swap is two renames; the old folder is removed only after it worked.
pub fn restore(ctx: &Ctx, save: &Save, img: &Image) -> Result<String> {
    if let Some(p) = &img.problem {
        return Err(Error::Refused(format!(
            "{} is broken ({p}): open the save as a new image instead (aodm save open)",
            img.name
        )));
    }
    let _held = images::hold(img)?;
    let m = verified(ctx, save)?;
    let warn = sysimg_warning(ctx, img.sysimg_revision(&ctx.env.path("ANDROID_HOME")), m);
    let parent = img.dir.parent().unwrap_or(Path::new("/"));
    images::ensure_space(parent, avd::du(&save.avd()))?;
    let stem = img
        .dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    images::sweep(ctx, parent);
    let partial = parent.join(format!(".{stem}.restore-{}", std::process::id()));
    let t0 = Instant::now();
    let staged = (|| {
        images::copy_tree(ctx, &save.avd(), &partial)?;
        // Same folder as when saved: the quick-boot snapshot still fits.
        images::tidy(&partial, m.source != img.dir)?;
        let cfg = partial.join("config.ini");
        let mut c = crate::ini::IniDoc::load(&cfg)?;
        c.set("avd.id", &img.name)?;
        c.set("avd.name", &img.name)?;
        c.save(&cfg)
    })();
    // Hold the restored folder too, then swap both folders in one atomic step
    // (RENAME_EXCHANGE): at no moment is the image without a folder.
    let swapped = staged
        .and_then(|()| images::lock(&img.name, &partial))
        .and_then(|new_lock| {
            rustix::fs::renameat_with(
                rustix::fs::CWD,
                &partial,
                rustix::fs::CWD,
                &img.dir,
                rustix::fs::RenameFlags::EXCHANGE,
            )
            .map_err(|e| Error::Io {
                path: img.dir.clone(),
                source: e.into(),
            })
            .map(|()| new_lock)
        });
    // `partial` now holds the old state (or, on failure, the unused copy): remove it.
    let cleanup = fs::remove_dir_all(&partial);
    if let Err(e) = swapped {
        ctx.log
            .error(&format!("restore {} from {}: {e}", img.name, save.id));
        return Err(e);
    }
    if let Err(e) = cleanup {
        ctx.log.warn(&format!(
            "restore {}: old state left in {} ({e}); delete it by hand",
            img.name,
            partial.display()
        ));
    }
    ctx.log.audit(
        "restore",
        &img.name,
        &format!("from {} in {:.1} s", save.id, t0.elapsed().as_secs_f64()),
    );
    let mut msg = format!("restored from save {}", save.id);
    if m.source != img.dir {
        msg.push_str(" (saved elsewhere: next boot is cold)");
    }
    if let Some(w) = warn {
        msg.push_str(&format!("; {w}"));
    }
    Ok(msg)
}

/// `aodm save open IMAGE ID --as NEW`: the save becomes a new image (verified first).
pub fn open(ctx: &Ctx, save: &Save, new: &str) -> Result<String> {
    let m = verified(ctx, save)?;
    let dir = images::install(ctx, &save.avd(), new, &m.target, false)?;
    Registry::update(&images::cfg_dir(ctx.env, "ADX_AOD_HOME")?, |r| {
        r.images.entry(new.to_string()).or_default().origin =
            Some(format!("{}@{}", save.image, save.id));
    })?;
    ctx.log
        .audit("open-save", &save.image, &format!("{} as {new}", save.id));
    Ok(format!(
        "opened save {}/{} as {new} ({}; first boot is cold; same Android ID as {} until identity policies, P5)",
        save.image,
        save.id,
        dir.display(),
        save.image
    ))
}

pub fn pin(ctx: &Ctx, save: &Save, on: bool) -> Result<()> {
    let p = save.path.join("pinned");
    if on {
        fs::write(&p, "").map_err(Error::io(&p))?;
    } else if p.exists() {
        fs::remove_file(&p).map_err(Error::io(&p))?;
    }
    ctx.log.audit(
        if on { "pin" } else { "unpin" },
        &save.image,
        &format!("save {}", save.id),
    );
    Ok(())
}

/// Removes one save (pinned ones only after unpinning).
pub fn delete(ctx: &Ctx, save: &Save) -> Result<u64> {
    if save.pinned {
        return Err(Error::Refused(format!(
            "save {}/{} is pinned: `aodm save pin {} {} --off` first",
            save.image, save.id, save.image, save.id
        )));
    }
    // Only ever `<saves>/<image>/<id>` as `list` built it: never a path from user input.
    avd::validate_name(&save.image)?;
    let sane = !save.id.is_empty()
        && !save.id.starts_with('.')
        && !save.id.contains('/')
        && save.path.ends_with(Path::new(&save.image).join(&save.id));
    if !sane {
        return Err(Error::Refused(format!(
            "refusing to delete {}: not a save folder",
            save.path.display()
        )));
    }
    let size = avd::du(&save.path);
    // Rename first, so a half-deleted save never looks like a valid one.
    let gone = save
        .path
        .with_file_name(format!(".deleting-{}-{}", save.id, std::process::id()));
    fs::rename(&save.path, &gone).map_err(Error::io(&save.path))?;
    fs::remove_dir_all(&gone).map_err(Error::io(&gone))?;
    // The image's saves folder goes once empty (fails harmlessly otherwise).
    if let Some(parent) = save.path.parent() {
        let _ = fs::remove_dir(parent);
    }
    ctx.log.audit(
        "delete-save",
        &save.image,
        &format!("{} ({} freed)", save.id, avd::human(size)),
    );
    Ok(size)
}

/// The saves `prune --keep N` removes: the oldest unpinned ones beyond the newest N.
pub fn prune_plan(saves: &[Save], keep: usize) -> Vec<&Save> {
    let unpinned: Vec<&Save> = saves.iter().filter(|s| !s.pinned).collect();
    let n = unpinned.len().saturating_sub(keep);
    unpinned.into_iter().take(n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_save(root: &Path, img: &str, id: &str, pinned: bool) {
        let p = root.join(img).join(id);
        fs::create_dir_all(p.join("avd")).unwrap();
        if pinned {
            fs::write(p.join("pinned"), "").unwrap();
        }
    }

    #[test]
    fn find_and_prune() {
        let d = tempfile::tempdir().unwrap();
        for (id, pin) in [
            ("20261101-100000", false),
            ("20261102-100000-clean", true),
            ("20261103-100000", false),
            ("20261104-100000", false),
        ] {
            fake_save(d.path(), "a", id, pin);
        }
        fs::create_dir_all(d.path().join("a/.partial-1")).unwrap();
        for bad in ["", "..", "/etc", "a/b", ".hidden"] {
            assert!(list(d.path(), Some(bad)).is_err(), "{bad:?}");
            assert!(find(d.path(), bad, "latest").is_err(), "{bad:?}");
        }
        assert!(find(d.path(), "a", "").is_err(), "empty id");
        let all = list(d.path(), Some("a")).unwrap();
        assert_eq!(all.len(), 4, "partials are not saves");
        assert!(
            all[0].manifest.is_err(),
            "no manifest: listed, marked damaged"
        );
        assert_eq!(find(d.path(), "a", "latest").unwrap().id, "20261104-100000");
        assert_eq!(
            find(d.path(), "a", "20261102").unwrap().id,
            "20261102-100000-clean"
        );
        assert!(find(d.path(), "a", "2026110").is_err(), "ambiguous prefix");
        assert!(find(d.path(), "a", "nope").is_err());
        let gone: Vec<_> = prune_plan(&all, 1).iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            gone,
            ["20261101-100000", "20261103-100000"],
            "pinned kept, newest kept"
        );
        assert!(prune_plan(&all, 5).is_empty());
        assert_eq!(counted(&all), 3);
    }

    #[test]
    fn hashes_and_refuses_symlinks() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("snapshots/x")).unwrap();
        fs::write(d.path().join("config.ini"), "abc").unwrap();
        fs::write(d.path().join("snapshots/x/ram bin"), "").unwrap();
        let h = hash_tree(d.path()).unwrap();
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].path, "config.ini");
        assert_eq!(
            h[0].sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(h[1].path, "snapshots/x/ram bin");
        std::os::unix::fs::symlink("/etc/passwd", d.path().join("link")).unwrap();
        assert!(hash_tree(d.path()).is_err());
    }

    #[test]
    fn labels() {
        assert!(validate_label("clean-install").is_ok());
        for bad in ["", "a b", "../x", &"x".repeat(33)] {
            assert!(validate_label(bad).is_err(), "{bad}");
        }
    }
}
