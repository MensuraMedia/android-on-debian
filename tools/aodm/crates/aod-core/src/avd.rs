//! Phone images on disk: `<avd home>/<name>.ini` + the `<name>.avd/` folder it points
//! to (with `config.ini`). The scan never fails because of one bad entry: unreadable
//! or broken images are listed with a `problem` so they can be repaired or removed.

use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::ini::IniDoc;

#[derive(Debug, Clone)]
pub struct Image {
    pub name: String,
    /// `<avd_home>/<name>.ini`
    pub ini_path: PathBuf,
    /// The `<name>.avd` folder (may not exist for a broken image).
    pub dir: PathBuf,
    pub ini: IniDoc,
    /// `config.ini` of the image; `None` if missing or unreadable.
    pub config: Option<IniDoc>,
    /// Why the image is broken, if it is.
    pub problem: Option<String>,
}

impl Image {
    fn cfg(&self, key: &str) -> Option<&str> {
        self.config.as_ref()?.get(key)
    }

    /// API level, e.g. "37.0" from `target=android-37.0`.
    pub fn api(&self) -> &str {
        self.ini
            .get("target")
            .map(|t| t.trim_start_matches("android-"))
            .unwrap_or("?")
    }

    pub fn abi(&self) -> &str {
        self.cfg("abi.type").unwrap_or("?")
    }

    pub fn tag(&self) -> &str {
        self.cfg("tag.id").unwrap_or("?")
    }

    pub fn device(&self) -> &str {
        self.cfg("hw.device.name").unwrap_or("?")
    }

    pub fn ram(&self) -> &str {
        self.cfg("hw.ramSize").unwrap_or("?")
    }

    /// `image.sysdir.1`, relative to the SDK.
    pub fn sysdir(&self) -> Option<&str> {
        self.cfg("image.sysdir.1")
    }

    /// Revision of the system image this image runs on (`Pkg.Revision`).
    pub fn sysimg_revision(&self, android_home: &Path) -> Option<String> {
        props(
            &android_home.join(self.sysdir()?).join("source.properties"),
            "Pkg.Revision",
        )
    }

    /// Bytes actually used on disk (allocated blocks; hard links counted once).
    pub fn disk_bytes(&self) -> u64 {
        du(&self.dir)
    }
}

/// Image names aodm creates and accepts: letters, digits, `.`, `_`, `-`; 1–64 chars;
/// not starting with `.` or `-`.
pub fn validate_name(name: &str) -> Result<()> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && !name.starts_with(['.', '-'])
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if ok {
        Ok(())
    } else {
        Err(Error::Invalid {
            what: "image name",
            detail: format!("{name:?} (use letters, digits, . _ -; max 64)"),
        })
    }
}

/// A plausible image folder: an existing directory whose name ends in `.avd`
/// (never `/`, `$HOME` or a relative path from a corrupt `.ini`).
fn avd_dir(p: &Path) -> bool {
    p.is_absolute() && p.extension().and_then(|e| e.to_str()) == Some("avd") && p.is_dir()
}

/// All images in `avd_home`, sorted by name. `user_home` is the Android user
/// directory (`~/.android` by default) that `path.rel=` is relative to.
/// Folder lookup per image, first plausible one wins: `path=`, `user_home/path.rel`,
/// `avd_home/<name>.avd`.
pub fn scan(avd_home: &Path, user_home: &Path) -> Result<Vec<Image>> {
    let entries = match fs::read_dir(avd_home) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(Error::Io {
                path: avd_home.to_path_buf(),
                source: e,
            });
        }
    };
    let mut images = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("ini") {
            continue;
        }
        let Some(name) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        // Follows symlinks: a dangling link or a directory named x.ini is reported, not fatal.
        if !path.is_file() {
            images.push(broken(
                name,
                path.clone(),
                avd_home,
                "the .ini is not a readable file (directory or dangling link)",
            ));
            continue;
        }
        let ini = match fs::read(&path) {
            Ok(bytes) => IniDoc::parse(&String::from_utf8_lossy(&bytes)),
            Err(e) => {
                images.push(broken(
                    name,
                    path.clone(),
                    avd_home,
                    &format!("cannot read the .ini: {e}"),
                ));
                continue;
            }
        };
        let candidates = [
            ini.get("path").map(PathBuf::from),
            ini.get("path.rel").map(|r| user_home.join(r)),
            Some(avd_home.join(format!("{name}.avd"))),
        ];
        let dir = candidates.iter().flatten().find(|p| avd_dir(p)).cloned();
        let (dir, config, problem) = match dir {
            Some(d) => match IniDoc::load(&d.join("config.ini")) {
                Ok(c) => (d, Some(c), None),
                Err(_) => (
                    d.clone(),
                    None,
                    Some(format!("no readable config.ini in {}", d.display())),
                ),
            },
            None => (
                avd_home.join(format!("{name}.avd")),
                None,
                Some("image folder not found (path=, path.rel=, <name>.avd)".into()),
            ),
        };
        images.push(Image {
            name,
            ini_path: path,
            dir,
            ini,
            config,
            problem,
        });
    }
    images.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(images)
}

fn broken(name: String, ini_path: PathBuf, avd_home: &Path, why: &str) -> Image {
    Image {
        dir: avd_home.join(format!("{name}.avd")),
        name,
        ini_path,
        ini: IniDoc::parse(""),
        config: None,
        problem: Some(why.to_string()),
    }
}

/// Value of `key` in a Java-style `.properties` file (`source.properties`, JDK `release`).
pub fn props(path: &Path, key: &str) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    text.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == key).then(|| v.trim().trim_matches('"').to_string())
    })
}

/// Allocated size of a directory tree (like `du -sx`): no symlinks followed, stays on
/// the root's filesystem, hard-linked files counted once.
pub fn du(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    let mut seen = HashSet::new();
    du_walk(path, meta.dev(), &mut seen)
}

fn du_walk(path: &Path, dev: u64, seen: &mut HashSet<(u64, u64)>) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.dev() != dev
        || (meta.nlink() > 1 && !meta.is_dir() && !seen.insert((meta.dev(), meta.ino())))
    {
        return 0;
    }
    let own = meta.blocks() * 512;
    if !meta.is_dir() {
        return own;
    }
    own + fs::read_dir(path)
        .map(|it| {
            it.flatten()
                .map(|e| du_walk(&e.path(), dev, seen))
                .sum::<u64>()
        })
        .unwrap_or(0)
}

/// Human size: 6.4G, 812M, 4.0K.
pub fn human(bytes: u64) -> String {
    let units = ["B", "K", "M", "G", "T"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < units.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{bytes}B")
    } else {
        format!("{v:.1}{}", units[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = include_str!("../tests/fixtures/config.ini");

    /// AVD home deliberately NOT named "avd" and not under the user home, as with a
    /// custom ANDROID_AVD_HOME.
    fn fake() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let avds = root.path().join("my-avds");
        let user = root.path().join("dot-android");
        fs::create_dir_all(avds.join("phone-a.avd")).unwrap();
        fs::write(avds.join("phone-a.avd/config.ini"), CONFIG).unwrap();
        fs::write(
            avds.join("phone-a.ini"),
            format!(
                "path={}\ntarget=android-37.0\n",
                avds.join("phone-a.avd").display()
            ),
        )
        .unwrap();
        // Moved with its .ini into the custom home: stale path=, path.rel points into the user home.
        fs::create_dir_all(avds.join("moved.avd")).unwrap();
        fs::write(avds.join("moved.avd/config.ini"), "abi.type=x86_64\n").unwrap();
        fs::write(
            avds.join("moved.ini"),
            "path=/gone/moved.avd\npath.rel=avd/moved.avd\ntarget=android-36\n",
        )
        .unwrap();
        // Only path.rel resolves (folder in the user home).
        fs::create_dir_all(user.join("avd/rel.avd")).unwrap();
        fs::write(
            user.join("avd/rel.avd/config.ini"),
            "hw.device.name=small_phone\n",
        )
        .unwrap();
        fs::write(
            avds.join("rel.ini"),
            "path.rel=avd/rel.avd\ntarget=android-30\n",
        )
        .unwrap();
        // Broken: no folder; corrupt path pointing at a huge non-.avd directory.
        fs::write(
            avds.join("broken.ini"),
            format!("path={}\ntarget=android-30\n", root.path().display()),
        )
        .unwrap();
        // Junk that must not break the scan.
        fs::create_dir(avds.join("dir.ini")).unwrap();
        std::os::unix::fs::symlink("/nonexistent", avds.join("dangling.ini")).unwrap();
        fs::write(avds.join("binary.ini"), b"path=\xff\xfe\n").unwrap();
        fs::write(avds.join("notes.txt"), "ignored").unwrap();
        (root, avds, user)
    }

    #[test]
    fn scan_survives_junk_and_resolves_folders() {
        let (_root, avds, user) = fake();
        let imgs = scan(&avds, &user).unwrap();
        let by = |n: &str| imgs.iter().find(|i| i.name == n).unwrap();
        let names: Vec<_> = imgs.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "binary", "broken", "dangling", "dir", "moved", "phone-a", "rel"
            ]
        );
        let a = by("phone-a");
        assert_eq!(
            (a.api(), a.abi(), a.tag(), a.device(), a.ram()),
            ("37.0", "x86_64", "google_apis_playstore", "pixel_8", "2G")
        );
        assert!(a.problem.is_none());
        assert_eq!(
            by("moved").abi(),
            "x86_64",
            "stale path= falls back to <home>/<name>.avd"
        );
        assert_eq!(
            by("rel").device(),
            "small_phone",
            "path.rel resolves against the user home"
        );
        assert!(
            by("broken").problem.is_some(),
            "a non-.avd path= is never used"
        );
        for junk in ["dir", "dangling", "binary"] {
            assert!(by(junk).problem.is_some(), "{junk}");
        }
    }

    #[test]
    fn missing_avd_home_is_empty() {
        assert!(
            scan(Path::new("/nonexistent/avd"), Path::new("/nonexistent"))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn names() {
        for ok in ["phone-01", "Pixel_8.api37", "a"] {
            assert!(validate_name(ok).is_ok(), "{ok}");
        }
        for bad in ["", ".hidden", "-x", "a/b", "a b", "ä", &"x".repeat(65)] {
            assert!(validate_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn properties_and_sizes() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("source.properties");
        fs::write(&p, "Pkg.Desc=x\nPkg.Revision=37.2.13\n").unwrap();
        assert_eq!(props(&p, "Pkg.Revision").as_deref(), Some("37.2.13"));
        let r = dir.path().join("release");
        fs::write(&r, "JAVA_VERSION=\"21.0.13\"\n").unwrap();
        assert_eq!(props(&r, "JAVA_VERSION").as_deref(), Some("21.0.13"));
        fs::write(dir.path().join("blob"), vec![1u8; 100_000]).unwrap();
        let once = du(dir.path());
        assert!(once >= 100_000);
        fs::hard_link(dir.path().join("blob"), dir.path().join("blob2")).unwrap();
        assert_eq!(du(dir.path()), once, "hard links counted once");
        assert_eq!(human(512), "512B");
        assert_eq!(human(6_871_947_674), "6.4G");
    }
}
