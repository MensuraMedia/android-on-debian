//! `$ADX_AOD_HOME/images.toml`: what aodm knows about images beyond the AVD files
//! (groups, pinned port, template, origin). Keys it doesn't know are kept on save,
//! so newer and older aodm versions can share one file. Comments are not kept.
//! Writers go through `Registry::update` (flock on `images.lock`, `schema` checked).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
    /// Free-form labels (`aodm tag set`, selected with `--tag K=V`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tags: BTreeMap<String, String>,
    /// Pinned console port (otherwise allocated at start).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Template it was built from, `name@version`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    /// Where it came from, e.g. `adx_api37@20261106-120000-clean` for an opened save.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    /// Unknown keys, preserved.
    #[serde(flatten)]
    pub extra: toml::Table,
}

/// File format version this aodm writes; a newer file is never overwritten.
pub const SCHEMA: u32 = 1;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub schema: u32,
    #[serde(default)]
    pub images: BTreeMap<String, Entry>,
    #[serde(flatten)]
    pub extra: toml::Table,
}

impl Registry {
    pub fn path(aod_home: &Path) -> PathBuf {
        aod_home.join("images.toml")
    }

    /// Loads the registry; a missing file is an empty registry.
    pub fn load(aod_home: &Path) -> Result<Registry> {
        let path = Registry::path(aod_home);
        match fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).map_err(|e| Error::Parse {
                path: path.clone(),
                line: e
                    .span()
                    .map(|s| text[..s.start].matches('\n').count() + 1)
                    .unwrap_or(0),
                msg: e.message().to_string(),
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Registry::default()),
            Err(e) => Err(Error::Io { path, source: e }),
        }
    }

    /// Saves atomically (temp file + fsync + rename).
    pub fn save(&self, aod_home: &Path) -> Result<()> {
        let path = Registry::path(aod_home);
        fs::create_dir_all(aod_home).map_err(Error::io(aod_home))?;
        let text =
            toml::to_string_pretty(self).map_err(|e| Error::Config(format!("images.toml: {e}")))?;
        let tmp = aod_home.join(format!(".images.toml.{}.tmp", std::process::id()));
        let write = || -> std::io::Result<()> {
            use std::io::Write;
            let mut f = fs::File::create(&tmp)?;
            f.write_all(format!("# aodm image registry (managed by aodm; safe to edit while aodm is not running)\n{text}").as_bytes())?;
            f.sync_all()?;
            fs::rename(&tmp, &path)?;
            fs::File::open(aod_home)?.sync_all()
        };
        write().map_err(|e| {
            let _ = fs::remove_file(&tmp);
            Error::Io { path, source: e }
        })
    }

    /// Load, change, save under an exclusive lock, so concurrent aodm runs don't lose
    /// each other's changes.
    pub fn update(aod_home: &Path, change: impl FnOnce(&mut Registry)) -> Result<()> {
        fs::create_dir_all(aod_home).map_err(Error::io(aod_home))?;
        let lock_path = aod_home.join("images.lock");
        let lock = fs::File::create(&lock_path).map_err(Error::io(&lock_path))?;
        lock.lock().map_err(Error::io(&lock_path))?;
        let mut reg = Registry::load(aod_home)?;
        reg.check(aod_home)?;
        change(&mut reg);
        reg.schema = SCHEMA;
        reg.save(aod_home)
    }

    /// Errors now if a later `update` would fail (unreadable file, newer schema), so
    /// commands can refuse before they change anything on disk.
    pub fn writable(aod_home: &Path) -> Result<()> {
        Registry::load(aod_home)?.check(aod_home)
    }

    fn check(&self, aod_home: &Path) -> Result<()> {
        if self.schema > SCHEMA {
            return Err(Error::Refused(format!(
                "{} has schema {} (this aodm writes {SCHEMA}): update aodm before changing images",
                Registry::path(aod_home).display(),
                self.schema
            )));
        }
        Ok(())
    }

    pub fn entry(&self, image: &str) -> Option<&Entry> {
        self.images.get(image)
    }

    pub fn groups(&self, image: &str) -> &[String] {
        self.entry(image)
            .map(|e| e.groups.as_slice())
            .unwrap_or(&[])
    }

    pub fn tag(&self, image: &str, key: &str) -> Option<&str> {
        self.entry(image)
            .and_then(|e| e.tags.get(key))
            .map(String::as_str)
    }

    /// Every group with its members, sorted.
    pub fn all_groups(&self) -> BTreeMap<&str, Vec<&str>> {
        let mut g: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (name, e) in &self.images {
            for x in &e.groups {
                g.entry(x).or_default().push(name);
            }
        }
        g
    }

    /// Drops entries that hold nothing (after removing a group or tag).
    pub fn compact(&mut self) {
        self.images.retain(|_, e| *e != Entry::default());
    }
}

/// Group names and tag keys: the same rule as image names.
pub fn validate_label(what: &'static str, s: &str) -> Result<()> {
    crate::avd::validate_name(s).map_err(|_| Error::Invalid {
        what,
        detail: format!("{s:?} (use letters, digits, . _ -; max 64)"),
    })
}

/// Tag values: one line of printable text, max 128 characters.
pub fn validate_value(s: &str) -> Result<()> {
    if s.len() > 128 || s.chars().any(char::is_control) {
        return Err(Error::Invalid {
            what: "tag value",
            detail: format!("{s:?} (one line, max 128 characters)"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Registry::load(dir.path()).unwrap(), Registry::default());
    }

    #[test]
    fn round_trip_keeps_unknown_keys() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            Registry::path(dir.path()),
            "future_top = 1\n[images.phone-a]\ngroups = [\"qa\"]\nport = 5556\nlocks = [\"camera\"]\n[images.phone-b]\ntemplate = \"barebones@1\"\n",
        )
        .unwrap();
        let mut reg = Registry::load(dir.path()).unwrap();
        assert_eq!(reg.groups("phone-a"), ["qa"]);
        assert_eq!(reg.entry("phone-a").unwrap().port, Some(5556));
        assert!(reg.entry("phone-a").unwrap().extra.contains_key("locks"));
        reg.images
            .get_mut("phone-b")
            .unwrap()
            .groups
            .push("demo".into());
        reg.save(dir.path()).unwrap();
        let back = Registry::load(dir.path()).unwrap();
        assert_eq!(back, reg);
        assert!(back.extra.contains_key("future_top"));
    }

    #[test]
    fn update_sets_schema_and_refuses_newer() {
        let dir = tempfile::tempdir().unwrap();
        Registry::update(dir.path(), |r| {
            r.images.entry("a".into()).or_default().origin = Some("b".into());
        })
        .unwrap();
        let r = Registry::load(dir.path()).unwrap();
        assert_eq!(
            (r.schema, r.entry("a").unwrap().origin.as_deref()),
            (1, Some("b"))
        );
        fs::write(Registry::path(dir.path()), "schema = 2\n").unwrap();
        assert!(Registry::update(dir.path(), |_| {}).is_err());
    }

    /// The P4 binary knows no `tags`: its flattened `extra` table must carry them
    /// through a rewrite (rollback, or mixed versions on one machine).
    #[test]
    fn older_shape_keeps_tags_table() {
        #[derive(Serialize, Deserialize)]
        struct P4Entry {
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            groups: Vec<String>,
            #[serde(flatten)]
            extra: toml::Table,
        }
        #[derive(Serialize, Deserialize)]
        struct P4 {
            #[serde(default)]
            images: BTreeMap<String, P4Entry>,
            #[serde(flatten)]
            extra: toml::Table,
        }
        let text = "schema = 1\n[images.a]\ngroups = [\"qa\"]\n[images.a.tags]\nowner = \"x\"\n";
        let old: P4 = toml::from_str(text).unwrap();
        let rewritten = toml::to_string_pretty(&old).unwrap();
        let back: Registry = toml::from_str(&rewritten).unwrap();
        assert_eq!(back.tag("a", "owner"), Some("x"));
        assert_eq!(back.groups("a"), ["qa"]);
    }

    #[test]
    fn parse_error_has_line() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(Registry::path(dir.path()), "[images.a]\nport = \"x\"\n").unwrap();
        let err = Registry::load(dir.path()).unwrap_err().to_string();
        assert!(err.contains("images.toml:2"), "{err}");
        fs::write(Registry::path(dir.path()), "a=1\nb=2\n=3\n").unwrap();
        let err = Registry::load(dir.path()).unwrap_err().to_string();
        assert!(err.contains("images.toml:3"), "error at column 0: {err}");
    }
}
