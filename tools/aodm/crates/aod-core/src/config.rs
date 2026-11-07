//! `aodm config`: checks settings and edits `config/local.env`, the git-ignored file
//! adx and aodm both read last (so it wins over `defaults.env`). Edits keep every other
//! line as it was and replace the file atomically.

use std::fs;
use std::path::{Path, PathBuf};

use crate::env::{Env, KEYS};
use crate::error::{Error, Result};
use crate::{avd, emu, ports};

/// Keys that must hold an absolute path (relative ones would land in the current directory).
const PATHS: &[&str] = &[
    "ANDROID_HOME",
    "ANDROID_USER_HOME",
    "ANDROID_AVD_HOME",
    "ANDROID_EMULATOR_HOME",
    "ADX_JAVA_HOME",
    "ADX_LOG_DIR",
    "ADX_AOD_HOME",
    "ADX_AOD_SAVES_DIR",
];

/// Numeric keys with their allowed range.
const NUMBERS: &[(&str, u64, u64)] = &[
    ("ADX_AOD_MAX_SAVES", 1, 1000),
    ("ADX_AOD_MAX_INSTANCES", 1, 16),
    ("ADX_AOD_RAM_RESERVE_MB", 0, 1 << 20),
    ("ADX_LOG_MAX_KB", 1, 1 << 22),
    ("ADX_LOG_KEEP", 1, 100),
    ("ADX_EMU_BOOT_TIMEOUT", 10, 3600),
];

/// `-gpu` modes the emulator accepts.
const GPU: &[&str] = &[
    "auto",
    "auto-no-window",
    "host",
    "software",
    "swiftshader_indirect",
    "angle_indirect",
    "guest",
];

/// Keys `aodm config set` may write (everything aodm reads except ADX_ROOT, which locates
/// the config itself).
pub fn settable(key: &str) -> bool {
    key != "ADX_ROOT" && KEYS.iter().any(|(k, _)| *k == key)
}

/// Problem with one value, or None. Empty optional values are fine.
pub fn check(key: &str, value: &str) -> Option<String> {
    let bad = |why: String| Some(format!("{key}={value:?}: {why}"));
    if PATHS.contains(&key) {
        if value.is_empty() && key == "ANDROID_EMULATOR_HOME" {
            return None;
        }
        if !Path::new(value).is_absolute() {
            return bad("must be an absolute path".into());
        }
        return None;
    }
    if let Some((_, lo, hi)) = NUMBERS.iter().find(|(k, ..)| *k == key) {
        return match value.parse::<u64>() {
            Ok(n) if (*lo..=*hi).contains(&n) => None,
            _ => bad(format!("must be a whole number from {lo} to {hi}")),
        };
    }
    match key {
        "ADX_AOD_BASE_PORT" | "ADX_EMU_PORT" => match value.parse::<u16>() {
            Ok(p) if ports::slots(ports::FIRST).is_ok_and(|s| s.contains(&p)) => None,
            _ => bad(format!(
                "must be an even port from {} to {} (adb finds emulators only there)",
                ports::FIRST,
                ports::LAST
            )),
        },
        "ADX_EMU_GPU" if !GPU.contains(&value) => bad(format!("must be one of {}", GPU.join(", "))),
        "ADX_EMU_HEADLESS" if !matches!(value, "true" | "false") => {
            bad("must be true or false".into())
        }
        "ADX_EMU_ENV" => emu::parse_env(value).err().map(|e| e.to_string()),
        "ADX_AVD_NAME" => avd::validate_name(value).err().map(|e| e.to_string()),
        _ => None,
    }
}

/// Every problem in the resolved configuration (empty when valid).
pub fn validate(env: &Env) -> Vec<String> {
    let mut out: Vec<String> = KEYS
        .iter()
        .filter(|(k, _)| *k != "ADX_ROOT")
        .filter_map(|(k, _)| check(k, env.get(k)))
        .collect();
    if env.get("ADX_AOD_BASE_PORT") != env.get("ADX_EMU_PORT")
        && env.num("ADX_AOD_BASE_PORT", 0) > env.num("ADX_EMU_PORT", 0)
    {
        out.push(format!(
            "ADX_EMU_PORT={} is below ADX_AOD_BASE_PORT={}: adx's image would sit outside aodm's range",
            env.get("ADX_EMU_PORT"),
            env.get("ADX_AOD_BASE_PORT")
        ));
    }
    if let Some(e) = &env.config_error {
        out.push(e.clone());
    }
    out
}

/// `config/local.env` of the framework this aodm reads.
pub fn local_env(env: &Env) -> Result<PathBuf> {
    match env.get("ADX_ROOT") {
        "" => Err(Error::Config(
            "framework not found (ADX_ROOT): no config/local.env to write".into(),
        )),
        r => Ok(Path::new(r).join("config/local.env")),
    }
}

/// `KEY=` or `export KEY=` (any blanks), the lines adx's `adx_local_set` also finds.
fn is_line_for(line: &str, key: &str) -> bool {
    let l = line.trim_start();
    let l = match l.strip_prefix("export") {
        Some(r) if r.starts_with([' ', '\t']) => r.trim_start(),
        _ => l,
    };
    l.starts_with(&format!("{key}="))
}

/// A line whose value goes on to the next line (open quote, trailing backslash):
/// rewriting only its first line would break the file for bash.
fn continues(line: &str) -> bool {
    let quotes = |q| line.matches(q).count() % 2 == 1;
    line.trim_end().ends_with('\\') || quotes('"') || quotes('\'')
}

fn check_lines(path: &Path, text: &str, key: &str) -> Result<()> {
    if text.lines().any(|l| is_line_for(l, key) && continues(l)) {
        return Err(Error::Refused(format!(
            "{key} in {} spans several lines: edit it by hand",
            path.display()
        )));
    }
    Ok(())
}

/// Refuses a key/value `set` would refuse (also used by `--dry-run`).
pub fn check_set(key: &str, value: &str) -> Result<()> {
    if !settable(key) {
        return Err(Error::Invalid {
            what: "setting",
            detail: format!("{key} is not a setting aodm knows (aodm config list)"),
        });
    }
    // Written inside double quotes in a file bash sources: nothing that bash would expand.
    if value
        .chars()
        .any(|c| c.is_control() || matches!(c, '"' | '\\' | '$' | '`'))
    {
        return Err(Error::Invalid {
            what: "value",
            detail: format!(
                "{key}: quotes, backslashes, $, ` and control characters are not allowed"
            ),
        });
    }
    if let Some(p) = check(key, value) {
        return Err(Error::Invalid {
            what: "value",
            detail: p,
        });
    }
    Ok(())
}

/// Sets `key` in local.env (replaces its line, or appends one). Returns the old line.
pub fn set(path: &Path, key: &str, value: &str) -> Result<Option<String>> {
    check_set(key, value)?;
    let text = read(path)?;
    check_lines(path, &text, key)?;
    let new = format!("{key}=\"{value}\"");
    let mut old = None;
    let mut lines: Vec<String> = Vec::new();
    for l in text.lines() {
        if is_line_for(l, key) {
            if old.is_none() {
                old = Some(l.to_string());
                lines.push(new.clone());
            }
            // Later duplicates would win in bash: drop them.
        } else {
            lines.push(l.to_string());
        }
    }
    if old.is_none() {
        if lines.is_empty() {
            lines.push("# Machine overrides (git-ignored). See config/local.env.example.".into());
        }
        lines.push(format!(
            "# set by aodm config on {}",
            jiff::Zoned::now().strftime("%Y-%m-%d")
        ));
        lines.push(new);
    }
    write(path, &lines)?;
    Ok(old)
}

/// Removes `key` from local.env (back to defaults.env). Returns the removed line.
pub fn unset(path: &Path, key: &str) -> Result<Option<String>> {
    if !settable(key) {
        return Err(Error::Invalid {
            what: "setting",
            detail: format!("{key} is not a setting aodm knows (aodm config list)"),
        });
    }
    let text = read(path)?;
    check_lines(path, &text, key)?;
    let old = text
        .lines()
        .find(|l| is_line_for(l, key))
        .map(str::to_string);
    if old.is_some() {
        let lines: Vec<String> = text
            .lines()
            .filter(|l| !is_line_for(l, key))
            .map(str::to_string)
            .collect();
        write(path, &lines)?;
    }
    Ok(old)
}

fn read(path: &Path) -> Result<String> {
    match fs::read_to_string(path) {
        Ok(t) => Ok(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(Error::Io {
            path: path.into(),
            source: e,
        }),
    }
}

/// Atomic replace that keeps what the user set up: a symlinked local.env (dotfiles)
/// is written through to its target, and the file keeps its permissions.
/// shortcut: no lock against a concurrent `adx_local_set` append (one of the two edits
/// can be lost); both are interactive and rare, add a flock if that changes.
fn write(path: &Path, lines: &[String]) -> Result<()> {
    let path = &fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let tmp = path.with_extension(format!("env.{}.tmp", std::process::id()));
    let mut text = lines.join("\n");
    text.push('\n');
    let r = (|| {
        use std::io::Write;
        let mut f = fs::File::create(&tmp)?;
        if let Ok(m) = fs::metadata(path) {
            f.set_permissions(m.permissions())?;
        }
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        fs::rename(&tmp, path)
    })();
    r.map_err(|e| {
        let _ = fs::remove_file(&tmp);
        Error::Io {
            path: path.into(),
            source: e,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_values() {
        assert!(check("ADX_AOD_HOME", "/x").is_none());
        assert!(check("ADX_AOD_HOME", "~/x").is_some());
        assert!(check("ANDROID_EMULATOR_HOME", "").is_none());
        assert!(check("ADX_AOD_MAX_INSTANCES", "17").is_some());
        assert!(check("ADX_AOD_MAX_SAVES", "3").is_none());
        assert!(check("ADX_EMU_PORT", "5555").is_some());
        assert!(check("ADX_EMU_PORT", "5556").is_none());
        assert!(check("ADX_EMU_GPU", "software").is_none());
        assert!(check("ADX_EMU_GPU", "fast").is_some());
        assert!(check("ADX_EMU_HEADLESS", "yes").is_some());
        assert!(check("ADX_AVD_NAME", "../x").is_some());
        assert!(check("ADX_EMU_ENV", "A=1 B=2").is_none());
    }

    #[test]
    fn set_and_unset_keep_other_lines() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("local.env");
        fs::write(
            &f,
            "# keep me\nADX_EMU_GPU=\"software\"\nexport ADX_AOD_MAX_SAVES=2\nADX_AOD_MAX_SAVES=9\n",
        )
        .unwrap();
        let old = set(&f, "ADX_AOD_MAX_SAVES", "5").unwrap();
        assert_eq!(old.as_deref(), Some("export ADX_AOD_MAX_SAVES=2"));
        assert_eq!(
            fs::read_to_string(&f).unwrap(),
            "# keep me\nADX_EMU_GPU=\"software\"\nADX_AOD_MAX_SAVES=\"5\"\n"
        );
        set(&f, "ADX_EMU_HEADLESS", "false").unwrap();
        assert!(
            fs::read_to_string(&f)
                .unwrap()
                .ends_with("ADX_EMU_HEADLESS=\"false\"\n")
        );
        assert_eq!(
            unset(&f, "ADX_EMU_GPU").unwrap().as_deref(),
            Some("ADX_EMU_GPU=\"software\"")
        );
        assert!(!fs::read_to_string(&f).unwrap().contains("GPU"));
        assert_eq!(unset(&f, "ADX_EMU_GPU").unwrap(), None);
    }

    #[test]
    fn keeps_symlink_and_mode_and_refuses_multiline() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("dotfiles-local.env");
        fs::write(
            &real,
            "export\tADX_EMU_GPU=host\nADX_EMU_ENV=\"A=1\nB=2\"\n",
        )
        .unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
        let link = dir.path().join("local.env");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert_eq!(
            set(&link, "ADX_EMU_GPU", "software").unwrap().as_deref(),
            Some("export\tADX_EMU_GPU=host")
        );
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::metadata(&real).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(
            fs::read_to_string(&real)
                .unwrap()
                .starts_with("ADX_EMU_GPU=\"software\"\n")
        );
        assert!(
            set(&link, "ADX_EMU_ENV", "C=3").is_err(),
            "value spans two lines"
        );
        assert!(unset(&link, "ADX_EMU_ENV").is_err());
        assert!(unset(&link, "PATH").is_err());
    }

    #[test]
    fn set_refuses_unsafe_or_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("local.env");
        assert!(set(&f, "ADX_ROOT", "/x").is_err());
        assert!(set(&f, "PATH", "/x").is_err());
        assert!(set(&f, "ADX_AOD_HOME", "/x/$(rm -rf ~)").is_err());
        assert!(set(&f, "ADX_AOD_HOME", "/x\nADX_EMU_GPU=host").is_err());
        assert!(set(&f, "ADX_AOD_MAX_SAVES", "lots").is_err());
        assert!(!f.exists(), "nothing written on refusal");
        set(&f, "ADX_AOD_HOME", "/data/aod").unwrap();
        assert!(
            fs::read_to_string(&f)
                .unwrap()
                .starts_with("# Machine overrides")
        );
    }
}
