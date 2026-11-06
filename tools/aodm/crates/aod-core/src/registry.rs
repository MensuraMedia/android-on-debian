//! `$ADX_AOD_HOME/images.toml`: what aodm knows about images beyond the AVD files
//! (groups, pinned port, template, origin). Keys it doesn't know are kept on save,
//! so newer and older aodm versions can share one file. Comments are not kept.
//! shortcut: no lock and no `schema` key yet; the first writer (P3/P4) adds both.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Registry {
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

    pub fn entry(&self, image: &str) -> Option<&Entry> {
        self.images.get(image)
    }

    pub fn groups(&self, image: &str) -> &[String] {
        self.entry(image)
            .map(|e| e.groups.as_slice())
            .unwrap_or(&[])
    }
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
