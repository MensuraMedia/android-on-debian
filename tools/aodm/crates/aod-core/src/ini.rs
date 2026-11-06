//! Lossless editor for AVD `config.ini` / `<name>.ini` files.
//!
//! Every line is kept byte for byte; `set` rewrites only the value of one key (or
//! appends it), so unknown keys, comments, order, line endings (LF or CRLF), a
//! UTF-8 BOM and the trailing newline survive. Keys and values that would corrupt
//! the file (newlines, NUL, `=` in a key) are rejected.

use std::fs;
use std::io::Write;
use std::path::Path;

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IniDoc {
    lines: Vec<String>,
    trailing_newline: bool,
    crlf: bool,
    bom: bool,
}

/// `key=value` split of one line: (key, byte offset where the value starts).
fn split(line: &str) -> Option<(&str, usize)> {
    let t = line.trim_start();
    if t.is_empty() || t.starts_with('#') || t.starts_with(';') {
        return None;
    }
    let eq = line.find('=')?;
    let rest = &line[eq + 1..];
    let value_start = eq + 1 + (rest.len() - rest.trim_start().len());
    Some((line[..eq].trim(), value_start))
}

fn validate(key: &str, value: &str) -> Result<()> {
    let bad = |c: char| c == '\n' || c == '\r' || c == '\0';
    if key.is_empty() || key.contains('=') || key.contains(bad) || key.trim() != key {
        return Err(Error::Invalid {
            what: "ini key",
            detail: format!("{key:?}"),
        });
    }
    if value.contains(bad) {
        return Err(Error::Invalid {
            what: "ini value",
            detail: format!("{key}={value:?}"),
        });
    }
    Ok(())
}

impl IniDoc {
    pub fn parse(text: &str) -> IniDoc {
        let (bom, text) = match text.strip_prefix('\u{feff}') {
            Some(t) => (true, t),
            None => (false, text),
        };
        let crlf = text.contains("\r\n");
        let eol = if crlf { "\r\n" } else { "\n" };
        let trailing_newline = text.ends_with(eol);
        let body = text.strip_suffix(eol).unwrap_or(text);
        let lines = if text.is_empty() {
            Vec::new()
        } else {
            body.split(eol).map(str::to_string).collect()
        };
        IniDoc {
            lines,
            trailing_newline,
            crlf,
            bom,
        }
    }

    pub fn load(path: &Path) -> Result<IniDoc> {
        let text = fs::read_to_string(path).map_err(Error::io(path))?;
        Ok(IniDoc::parse(&text))
    }

    /// Writes durably and atomically: unique temp file in the same directory, fsync,
    /// keep the original's permissions, rename over the target (through a symlink,
    /// to the real file), fsync the directory.
    pub fn save(&self, path: &Path) -> Result<()> {
        let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let dir = target.parent().unwrap_or(Path::new("."));
        let name = target.file_name().and_then(|n| n.to_str()).unwrap_or("ini");
        let tmp = dir.join(format!(".{name}.aodm-{}.tmp", std::process::id()));
        let write = || -> std::io::Result<()> {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(self.to_string().as_bytes())?;
            if let Ok(meta) = fs::metadata(&target) {
                f.set_permissions(meta.permissions())?;
            }
            f.sync_all()?;
            fs::rename(&tmp, &target)?;
            fs::File::open(dir)?.sync_all()
        };
        write().map_err(|e| {
            let _ = fs::remove_file(&tmp);
            Error::Io {
                path: target.clone(),
                source: e,
            }
        })
    }

    /// Value of the last occurrence of `key` (the emulator's own rule), trimmed.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.lines.iter().rev().find_map(|l| {
            split(l)
                .filter(|(k, _)| *k == key)
                .map(|(_, v)| l[v..].trim_end())
        })
    }

    /// Sets `key` in place (keeping the original key spelling and spacing), or appends it.
    /// Returns the previous value. Rejects keys/values that would corrupt the file.
    pub fn set(&mut self, key: &str, value: &str) -> Result<Option<String>> {
        validate(key, value)?;
        let found = self
            .lines
            .iter()
            .rposition(|l| split(l).is_some_and(|(k, _)| k == key));
        Ok(match found {
            Some(i) => {
                let (_, v) = split(&self.lines[i]).expect("matched above");
                let old = self.lines[i][v..].trim_end().to_string();
                self.lines[i] = format!("{}{value}", &self.lines[i][..v]);
                Some(old)
            }
            None => {
                if self.lines.is_empty() {
                    self.trailing_newline = true;
                }
                self.lines.push(format!("{key}={value}"));
                None
            }
        })
    }

    /// All `(key, value)` pairs in file order.
    pub fn entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.lines
            .iter()
            .filter_map(|l| split(l).map(|(k, v)| (k, l[v..].trim_end())))
    }
}

impl std::fmt::Display for IniDoc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let eol = if self.crlf { "\r\n" } else { "\n" };
        if self.bom {
            f.write_str("\u{feff}")?;
        }
        f.write_str(&self.lines.join(eol))?;
        if self.trailing_newline {
            f.write_str(eol)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/config.ini");

    #[test]
    fn real_config_round_trips_byte_for_byte() {
        assert_eq!(IniDoc::parse(FIXTURE).to_string(), FIXTURE);
    }

    #[test]
    fn get_reads_real_values() {
        let d = IniDoc::parse(FIXTURE);
        assert_eq!(d.get("hw.device.name"), Some("pixel_8"));
        assert_eq!(d.get("tag.id"), Some("google_apis_playstore"));
        assert_eq!(d.get("avd.name"), Some("<build>"));
        assert_eq!(d.get("no.such.key"), None);
    }

    #[test]
    fn set_replaces_in_place_and_keeps_everything_else() {
        let mut d = IniDoc::parse(FIXTURE);
        assert_eq!(d.set("hw.ramSize", "4096M").unwrap().as_deref(), Some("2G"));
        let out = d.to_string();
        assert_eq!(out.lines().count(), FIXTURE.lines().count());
        let changed: Vec<_> = FIXTURE
            .lines()
            .zip(out.lines())
            .filter(|(a, b)| a != b)
            .collect();
        assert_eq!(changed, vec![("hw.ramSize=2G", "hw.ramSize=4096M")]);
    }

    #[test]
    fn set_appends_missing_and_keeps_spacing_style() {
        let mut d = IniDoc::parse("# c\nkey = old\n");
        assert_eq!(d.set("key", "new").unwrap().as_deref(), Some("old"));
        assert_eq!(d.set("added", "1").unwrap(), None);
        assert_eq!(d.to_string(), "# c\nkey = new\nadded=1\n");
    }

    #[test]
    fn last_occurrence_wins_and_comments_ignored() {
        let d = IniDoc::parse("a=1\n; a=9\n# a=8\na=2");
        assert_eq!(d.get("a"), Some("2"));
        assert_eq!(d.entries().count(), 2);
        assert_eq!(d.to_string(), "a=1\n; a=9\n# a=8\na=2");
    }

    #[test]
    fn rejects_injection_and_bad_keys() {
        let mut d = IniDoc::parse(FIXTURE);
        assert!(d.set("hw.ramSize", "4G\nhw.gpu.mode=swiftshader").is_err());
        assert!(d.set("hw.ramSize", "4G\r").is_err());
        assert!(d.set("a=b", "z").is_err());
        assert!(d.set("", "z").is_err());
        assert!(d.set(" padded", "z").is_err());
        assert_eq!(
            d.to_string(),
            FIXTURE,
            "rejected sets must not change the document"
        );
    }

    #[test]
    fn crlf_bom_and_empty_documents() {
        let mut d = IniDoc::parse("\u{feff}a=1\r\nb=2\r\n");
        assert_eq!(d.get("a"), Some("1"));
        d.set("a", "9").unwrap();
        d.set("c", "3").unwrap();
        assert_eq!(d.to_string(), "\u{feff}a=9\r\nb=2\r\nc=3\r\n");
        let mut e = IniDoc::parse("");
        e.set("k", "v").unwrap();
        assert_eq!(e.to_string(), "k=v\n");
    }

    #[test]
    fn save_is_atomic_keeps_mode_and_follows_symlinks() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("config.ini");
        fs::write(&real, FIXTURE).unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o640)).unwrap();
        let link = dir.path().join("link.ini");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let mut d = IniDoc::load(&link).unwrap();
        d.set("avd.name", "phone-01").unwrap();
        d.save(&link).unwrap();
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink(),
            "symlink kept"
        );
        assert_eq!(
            IniDoc::load(&real).unwrap().get("avd.name"),
            Some("phone-01")
        );
        assert_eq!(
            fs::metadata(&real).unwrap().permissions().mode() & 0o777,
            0o640
        );
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }
}
