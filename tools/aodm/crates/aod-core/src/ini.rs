//! Lossless editor for AVD `config.ini` / `<name>.ini` files.
//!
//! Every line is kept byte for byte; `set` rewrites only the value of one key (or
//! appends it), so unknown keys, comments, order and the trailing newline survive.

use std::fs;
use std::path::Path;

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IniDoc {
    lines: Vec<String>,
    trailing_newline: bool,
}

/// `key=value` split of one line: (key, byte offset where the value starts).
fn split(line: &str) -> Option<(&str, usize)> {
    let t = line.trim_start();
    if t.is_empty() || t.starts_with('#') || t.starts_with(';') {
        return None;
    }
    let eq = line.find('=')?;
    let value_start = eq + 1 + (line[eq + 1..].len() - line[eq + 1..].trim_start().len());
    Some((line[..eq].trim(), value_start))
}

impl IniDoc {
    pub fn parse(text: &str) -> IniDoc {
        let trailing_newline = text.ends_with('\n');
        let body = text.strip_suffix('\n').unwrap_or(text);
        let lines = if text.is_empty() {
            Vec::new()
        } else {
            body.split('\n').map(str::to_string).collect()
        };
        IniDoc {
            lines,
            trailing_newline,
        }
    }

    pub fn load(path: &Path) -> Result<IniDoc> {
        let text = fs::read_to_string(path).map_err(Error::io(path))?;
        Ok(IniDoc::parse(&text))
    }

    /// Writes atomically (temp file + rename) so a crash never leaves half a config.
    pub fn save(&self, path: &Path) -> Result<()> {
        let tmp = path.with_extension("ini.aodm-tmp");
        fs::write(&tmp, self.to_string()).map_err(Error::io(&tmp))?;
        fs::rename(&tmp, path).map_err(Error::io(path))
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
    /// Returns the previous value.
    pub fn set(&mut self, key: &str, value: &str) -> Option<String> {
        let found = self
            .lines
            .iter()
            .rposition(|l| split(l).is_some_and(|(k, _)| k == key));
        match found {
            Some(i) => {
                let (_, v) = split(&self.lines[i]).expect("matched above");
                let old = self.lines[i][v..].trim_end().to_string();
                self.lines[i] = format!("{}{value}", &self.lines[i][..v]);
                Some(old)
            }
            None => {
                self.lines.push(format!("{key}={value}"));
                None
            }
        }
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
        f.write_str(&self.lines.join("\n"))?;
        if self.trailing_newline {
            f.write_str("\n")?;
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
        assert_eq!(d.set("hw.ramSize", "4096M").as_deref(), Some("2G"));
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
        assert_eq!(d.set("key", "new").as_deref(), Some("old"));
        assert_eq!(d.set("added", "1"), None);
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
    fn save_is_atomic_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.ini");
        let mut d = IniDoc::parse(FIXTURE);
        d.set("avd.name", "phone-01");
        d.save(&p).unwrap();
        let back = IniDoc::load(&p).unwrap();
        assert_eq!(back.get("avd.name"), Some("phone-01"));
        assert!(!p.with_extension("ini.aodm-tmp").exists());
    }
}
