//! Phone images on disk: `$ANDROID_AVD_HOME/<name>.ini` + the `<name>.avd/` folder it
//! points to (with `config.ini`).

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
    /// The `<name>.avd` folder.
    pub dir: PathBuf,
    pub ini: IniDoc,
    /// `config.ini` of the image; `None` if it is missing or unreadable (broken image).
    pub config: Option<IniDoc>,
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

    /// Bytes actually used on disk (sparse files count their allocated blocks).
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

/// All images in `avd_home`, sorted by name. An image whose folder or config is
/// missing is still listed (with `config: None`) so it can be repaired or removed.
pub fn scan(avd_home: &Path) -> Result<Vec<Image>> {
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
        let ini = IniDoc::load(&path)?;
        // `path=` is absolute; `path.rel=` is relative to the avd home's parent (~/.android).
        let dir = ini
            .get("path")
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
            .or_else(|| {
                ini.get("path.rel")
                    .map(|r| avd_home.parent().unwrap_or(avd_home).join(r))
            })
            .unwrap_or_else(|| avd_home.join(format!("{name}.avd")));
        let config = IniDoc::load(&dir.join("config.ini")).ok();
        images.push(Image {
            name,
            ini_path: path,
            dir,
            ini,
            config,
        });
    }
    images.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(images)
}

/// Value of `key` in a Java-style `.properties` file (`source.properties`, JDK `release`).
pub fn props(path: &Path, key: &str) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    text.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == key).then(|| v.trim().trim_matches('"').to_string())
    })
}

/// Allocated size of a directory tree (like `du -s`), without following symlinks.
pub fn du(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    let own = meta.blocks() * 512;
    if !meta.is_dir() {
        return own;
    }
    own + fs::read_dir(path)
        .map(|it| it.flatten().map(|e| du(&e.path())).sum::<u64>())
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

    fn fake_home() -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        let avd = home.path().join("avd");
        fs::create_dir_all(avd.join("phone-a.avd")).unwrap();
        fs::write(avd.join("phone-a.avd/config.ini"), CONFIG).unwrap();
        fs::write(
            avd.join("phone-a.ini"),
            format!(
                "avd.ini.encoding=UTF-8\npath={}\npath.rel=avd/phone-a.avd\ntarget=android-37.0\n",
                avd.join("phone-a.avd").display()
            ),
        )
        .unwrap();
        // Moved home: absolute path is stale, path.rel still resolves.
        fs::create_dir_all(avd.join("moved.avd")).unwrap();
        fs::write(avd.join("moved.avd/config.ini"), "abi.type=x86_64\n").unwrap();
        fs::write(
            avd.join("moved.ini"),
            "path=/gone/moved.avd\npath.rel=avd/moved.avd\ntarget=android-36\n",
        )
        .unwrap();
        // Broken: no folder at all.
        fs::write(avd.join("broken.ini"), "target=android-30\n").unwrap();
        fs::write(avd.join("notes.txt"), "ignored").unwrap();
        home
    }

    #[test]
    fn scans_sorted_with_values_and_broken_images() {
        let home = fake_home();
        let imgs = scan(&home.path().join("avd")).unwrap();
        let names: Vec<_> = imgs.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["broken", "moved", "phone-a"]);
        let a = &imgs[2];
        assert_eq!(
            (a.api(), a.abi(), a.tag(), a.device(), a.ram()),
            ("37.0", "x86_64", "google_apis_playstore", "pixel_8", "2G")
        );
        assert_eq!(imgs[1].abi(), "x86_64", "path.rel used when path= is stale");
        assert!(imgs[0].config.is_none());
        assert_eq!(imgs[0].device(), "?");
    }

    #[test]
    fn missing_avd_home_is_empty() {
        assert!(scan(Path::new("/nonexistent/avd")).unwrap().is_empty());
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
        fs::write(dir.path().join("blob"), vec![1u8; 10_000]).unwrap();
        assert!(du(dir.path()) >= 10_000);
        assert_eq!(human(512), "512B");
        assert_eq!(human(6_871_947_674), "6.4G");
    }
}
