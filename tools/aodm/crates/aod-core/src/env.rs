//! Configuration, with one source of truth: `config/defaults.env` + `config/local.env`.
//!
//! Same precedence as `adx` itself:
//! 1. the config files, sourced through bash with the real environment (so a value
//!    written as `${VAR:-default}` in defaults.env honours `VAR` from the environment,
//!    exactly as under adx). A stale shell environment can't override them.
//! 2. the process environment, for keys the config files don't set (e.g. `ANDROID_AVD_HOME`);
//! 3. built-in fallbacks, when the framework isn't found at all (standalone binary).
//!
//! The framework is found via `ADX_ROOT` in the environment, else the root baked in at
//! build time.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a value came from (shown by `aodm env`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Process,
    Config,
    Fallback,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Process => "env",
            Source::Config => "config",
            Source::Fallback => "fallback",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Value {
    pub value: String,
    pub source: Source,
}

#[derive(Debug, Clone)]
pub struct Env {
    vars: HashMap<String, Value>,
    /// Why the framework config could not be read, if it couldn't.
    pub config_error: Option<String>,
    /// Environment values ignored because the config files set the key (adx semantics).
    pub ignored: Vec<String>,
}

/// Keys aodm reads, with fallbacks used only when the framework config is unavailable.
/// shortcut: fallbacks duplicate defaults.env; a test (fallbacks_match_defaults_env)
/// keeps them in sync while both live in this repo.
const KEYS: &[(&str, &str)] = &[
    ("ADX_ROOT", ""),
    ("ANDROID_HOME", "~/Android/Sdk"),
    // The emulator's own lookup order for AVDs: ANDROID_AVD_HOME, then
    // $ANDROID_EMULATOR_HOME/avd, then $ANDROID_USER_HOME/avd, then ~/.android/avd.
    // shortcut: the deprecated ANDROID_SDK_HOME/.android is not consulted.
    ("ANDROID_EMULATOR_HOME", ""),
    ("ANDROID_USER_HOME", "~/.android"),
    ("ANDROID_AVD_HOME", ""),
    ("ADX_JAVA_HOME", "~/.local/opt/jdk-21"),
    ("ADX_LOG_DIR", "~/.local/state/adx"),
    ("ADX_LOG_MAX_KB", "5120"),
    ("ADX_LOG_KEEP", "5"),
    ("ADX_AOD_HOME", "~/.local/share/aod"),
    ("ADX_AOD_SAVES_DIR", "~/.local/share/aod/saves"),
    ("ADX_AOD_MAX_SAVES", "3"),
    ("ADX_AOD_BASE_PORT", "5554"),
    ("ADX_AOD_MAX_INSTANCES", "3"),
    ("ADX_AOD_RAM_RESERVE_MB", "2048"),
    ("ADX_AVD_NAME", "adx_api37"),
    ("ADX_EMU_PORT", "5554"),
    ("ADX_EMU_GPU", "host"),
    ("ADX_EMU_ENV", ""),
    ("ADX_EMU_HEADLESS", "true"),
    ("ADX_EMU_BOOT_TIMEOUT", "240"),
];

fn is_root(r: &str) -> bool {
    !r.is_empty() && Path::new(r).join("config/defaults.env").is_file()
}

impl Env {
    /// Loads configuration from the real process environment and framework.
    pub fn load() -> Env {
        let process: HashMap<String, String> = std::env::vars().collect();
        // An invalid ADX_ROOT in the environment falls back to the baked-in root.
        let root = process
            .get("ADX_ROOT")
            .cloned()
            .filter(|r| is_root(r))
            .or_else(|| {
                option_env!("ADX_ROOT")
                    .map(str::to_string)
                    .filter(|r| is_root(r))
            });
        let (config, config_error) = match &root {
            Some(r) => match source_config(Path::new(r), &process) {
                Ok(c) => (c, None),
                Err(e) => (HashMap::new(), Some(e)),
            },
            None => (
                HashMap::new(),
                Some("framework not found (ADX_ROOT unset or invalid); using fallbacks".into()),
            ),
        };
        let home = process.get("HOME").cloned().unwrap_or_default();
        let mut env = Env::resolve(&process, &config, root.as_deref(), &home);
        env.config_error = config_error;
        env.ignored = env
            .vars
            .iter()
            .filter(|(k, v)| {
                v.source == Source::Config && process.get(*k).is_some_and(|p| p != &v.value)
            })
            .map(|(k, v)| {
                format!(
                    "{k}={} in the environment is ignored (config sets {})",
                    process[k], v.value
                )
            })
            .collect();
        env.ignored.sort();
        env
    }

    /// Pure resolution step (tested without touching the real environment).
    pub fn resolve(
        process: &HashMap<String, String>,
        config: &HashMap<String, String>,
        root: Option<&str>,
        home: &str,
    ) -> Env {
        let expand = |v: &str| match v.strip_prefix("~/") {
            Some(rest) => format!("{home}/{rest}"),
            None => v.to_string(),
        };
        let mut vars = HashMap::new();
        for (key, fallback) in KEYS {
            let v = if *key == "ADX_ROOT" {
                // Report the root actually used, not a rejected one from the environment.
                let source = if process.get("ADX_ROOT").map(String::as_str) == root {
                    Source::Process
                } else {
                    Source::Fallback
                };
                Value {
                    value: root.unwrap_or_default().to_string(),
                    source,
                }
            } else if let Some(v) = config.get(*key) {
                Value {
                    value: v.clone(),
                    source: Source::Config,
                }
            } else if let Some(v) = process.get(*key) {
                Value {
                    value: v.clone(),
                    source: Source::Process,
                }
            } else {
                Value {
                    value: expand(fallback),
                    source: Source::Fallback,
                }
            };
            vars.insert((*key).to_string(), v);
        }
        // AVD home: explicit value, else <emulator or user home>/avd (as the emulator does).
        if vars["ANDROID_AVD_HOME"].value.is_empty() {
            let base = if vars["ANDROID_EMULATOR_HOME"].value.is_empty() {
                "ANDROID_USER_HOME"
            } else {
                "ANDROID_EMULATOR_HOME"
            };
            let v = Value {
                value: format!("{}/avd", vars[base].value),
                source: vars[base].source,
            };
            vars.insert("ANDROID_AVD_HOME".into(), v);
        }
        // Saves follow ADX_AOD_HOME unless set (as defaults.env does), so a sandbox home
        // never writes saves into the real one.
        if vars["ADX_AOD_SAVES_DIR"].source == Source::Fallback {
            let home = &vars["ADX_AOD_HOME"];
            let v = Value {
                value: format!("{}/saves", home.value),
                source: home.source,
            };
            vars.insert("ADX_AOD_SAVES_DIR".into(), v);
        }
        Env {
            vars,
            config_error: None,
            ignored: Vec::new(),
        }
    }

    pub fn get(&self, key: &str) -> &str {
        self.vars.get(key).map(|v| v.value.as_str()).unwrap_or("")
    }

    pub fn path(&self, key: &str) -> PathBuf {
        PathBuf::from(self.get(key))
    }

    /// Numeric value; a bad number falls back to `default` instead of failing a command.
    pub fn num(&self, key: &str, default: u64) -> u64 {
        self.get(key).parse().unwrap_or(default)
    }

    /// The Android user directory `path.rel=` in AVD `.ini` files is relative to.
    pub fn android_user_home(&self) -> PathBuf {
        match self.get("ANDROID_EMULATOR_HOME") {
            "" => self.path("ANDROID_USER_HOME"),
            e => PathBuf::from(e),
        }
    }

    /// All known keys with value and source, sorted (for `aodm env`).
    pub fn entries(&self) -> Vec<(&str, &Value)> {
        let mut v: Vec<_> = self.vars.iter().map(|(k, v)| (k.as_str(), v)).collect();
        v.sort_by_key(|(k, _)| *k);
        v
    }
}

/// Sources defaults.env then local.env in bash (adx's own semantics) and returns the
/// resulting ADX_* / ANDROID_* variables. Only an allowlist of variables reaches bash
/// (HOME, PATH, USER, LANG, LC_*, XDG_*, ADX_*, ANDROID_*): no exported functions
/// (BASH_FUNC_*), BASH_ENV/ENV, SHELLOPTS/PS4 or IFS, so the environment can't run code
/// in this shell or forge values. (It could still affect tools the config files call
/// through PATH, exactly as under adx.)
fn source_config(
    root: &Path,
    process: &HashMap<String, String>,
) -> Result<HashMap<String, String>, String> {
    let script = r#"set -a; . "$1/config/defaults.env"; if [ -f "$1/config/local.env" ]; then . "$1/config/local.env"; fi; /usr/bin/env -0"#;
    let mut cmd = Command::new("bash");
    cmd.args(["--noprofile", "--norc", "-c", script, "aodm-config"])
        .arg(root)
        .env_clear()
        .envs(process.iter().filter(|(k, _)| allowed(k)));
    let out = cmd
        .output()
        .map_err(|e| format!("cannot run bash to read {}: {e}", root.display()))?;
    if !out.status.success() {
        return Err(format!(
            "reading {}/config failed (exit {}): {}",
            root.display(),
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let after: HashMap<String, String> = out
        .stdout
        .split(|b| *b == 0)
        .filter_map(|kv| std::str::from_utf8(kv).ok())
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    // Only what the config files set (or changed) counts as "config".
    Ok(after
        .into_iter()
        .filter(|(k, v)| {
            (k.starts_with("ADX_") || k.starts_with("ANDROID_"))
                && config_sets(root, k, process.get(k), v)
        })
        .collect())
}

fn allowed(k: &str) -> bool {
    matches!(k, "HOME" | "PATH" | "USER" | "LANG")
        || ["LC_", "XDG_", "ADX_", "ANDROID_"]
            .iter()
            .any(|p| k.starts_with(p))
}

/// True when `key` is assigned in the config files (so its value is the config's,
/// even if it equals the environment's).
fn config_sets(root: &Path, key: &str, before: Option<&String>, after: &str) -> bool {
    if before.map(String::as_str) != Some(after) {
        return true;
    }
    ["config/defaults.env", "config/local.env"].iter().any(|f| {
        std::fs::read_to_string(root.join(f))
            .map(|t| {
                t.lines().any(|l| {
                    l.trim_start()
                        .trim_start_matches("export ")
                        .starts_with(&format!("{key}="))
                })
            })
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn repo_root() -> PathBuf {
        // Runtime, not env!(): a binary built in a (deleted) snapshot must not point there.
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("../../../..")
            .canonicalize()
            .unwrap()
    }

    #[test]
    fn config_beats_process_beats_fallback() {
        let process = map(&[
            ("ADX_EMU_GPU", "host"),
            ("ADX_ROOT", "/r"),
            ("ANDROID_AVD_HOME", "/avd"),
        ]);
        let config = map(&[("ADX_EMU_GPU", "software"), ("ADX_AOD_MAX_SAVES", "7")]);
        let env = Env::resolve(&process, &config, Some("/r"), "/home/u");
        assert_eq!(
            env.get("ADX_EMU_GPU"),
            "software",
            "a stale shell value must not win"
        );
        assert_eq!(env.vars["ADX_EMU_GPU"].source, Source::Config);
        assert_eq!(env.get("ANDROID_AVD_HOME"), "/avd");
        assert_eq!(env.vars["ANDROID_AVD_HOME"].source, Source::Process);
        assert_eq!(env.num("ADX_AOD_MAX_SAVES", 3), 7);
        assert_eq!(env.get("ADX_AOD_HOME"), "/home/u/.local/share/aod");
        assert_eq!(env.vars["ADX_AOD_HOME"].source, Source::Fallback);
        assert_eq!(env.vars["ADX_ROOT"].source, Source::Process);
    }

    #[test]
    fn rejected_root_is_not_reported() {
        let env = Env::resolve(
            &map(&[("ADX_ROOT", "/nonexistent")]),
            &HashMap::new(),
            Some("/baked"),
            "/h",
        );
        assert_eq!(env.get("ADX_ROOT"), "/baked");
        assert_eq!(env.vars["ADX_ROOT"].source, Source::Fallback);
    }

    #[test]
    fn bad_number_uses_default() {
        let env = Env::resolve(
            &map(&[("ADX_AOD_MAX_SAVES", "lots")]),
            &HashMap::new(),
            None,
            "/h",
        );
        assert_eq!(env.num("ADX_AOD_MAX_SAVES", 3), 3);
    }

    #[test]
    fn sources_real_config_with_adx_semantics() {
        let root = repo_root();
        let mut process = map(&[
            ("HOME", "/home/t"),
            ("PATH", "/usr/bin:/bin"),
            ("ADX_APP_SKILLS", "false"),
            ("ADX_EMU_GPU", "stale"),
            ("BASH_ENV", "/nonexistent-should-be-ignored"),
        ]);
        process.insert("XDG_STATE_HOME".into(), "/xdg/state".into());
        let cfg = source_config(&root, &process).unwrap();
        assert_eq!(
            cfg["ADX_LOG_DIR"], "/xdg/state/adx",
            "XDG honoured like under adx"
        );
        assert_eq!(
            cfg["ADX_APP_SKILLS"], "false",
            "VAR:-default assignments honour the environment"
        );
        assert_ne!(
            cfg["ADX_EMU_GPU"], "stale",
            "plain assignments override the environment"
        );
    }

    #[test]
    fn environment_cannot_inject_or_forge() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("pwned");
        let set_fn = format!("() {{ touch {}; builtin set \"$@\"; }}", marker.display());
        let ps4 = format!("$(touch {})", marker.display());
        let process = map(&[
            ("HOME", "/home/t"),
            ("PATH", "/usr/bin:/bin"),
            ("BASH_FUNC_set%%", &set_fn),
            ("BASH_FUNC_env%%", "() { echo ADX_EMU_GPU=evil; }"),
            ("SHELLOPTS", "xtrace"),
            ("PS4", &ps4),
        ]);
        let cfg = source_config(&repo_root(), &process).unwrap();
        assert!(!marker.exists(), "code from the environment ran");
        assert_ne!(cfg.get("ADX_EMU_GPU").map(String::as_str), Some("evil"));
    }

    #[test]
    fn saves_follow_aod_home() {
        let none = HashMap::new();
        let env = Env::resolve(&map(&[("ADX_AOD_HOME", "/sbx")]), &none, None, "/h");
        assert_eq!(env.get("ADX_AOD_SAVES_DIR"), "/sbx/saves");
        let env = Env::resolve(
            &map(&[
                ("ADX_AOD_HOME", "/sbx"),
                ("ADX_AOD_SAVES_DIR", "/big/saves"),
            ]),
            &none,
            None,
            "/h",
        );
        assert_eq!(env.get("ADX_AOD_SAVES_DIR"), "/big/saves");
    }

    #[test]
    fn avd_home_follows_emulator_lookup_order() {
        let none = HashMap::new();
        let env = Env::resolve(&none, &none, None, "/h");
        assert_eq!(env.get("ANDROID_AVD_HOME"), "/h/.android/avd");
        let env = Env::resolve(&map(&[("ANDROID_USER_HOME", "/u")]), &none, None, "/h");
        assert_eq!(env.get("ANDROID_AVD_HOME"), "/u/avd");
        let env = Env::resolve(
            &map(&[("ANDROID_USER_HOME", "/u"), ("ANDROID_EMULATOR_HOME", "/e")]),
            &none,
            None,
            "/h",
        );
        assert_eq!(
            (env.get("ANDROID_AVD_HOME"), env.android_user_home()),
            ("/e/avd", PathBuf::from("/e"))
        );
        let env = Env::resolve(
            &map(&[("ANDROID_AVD_HOME", "/x"), ("ANDROID_EMULATOR_HOME", "/e")]),
            &none,
            None,
            "/h",
        );
        assert_eq!(env.get("ANDROID_AVD_HOME"), "/x");
    }

    #[test]
    fn config_failure_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("config")).unwrap();
        std::fs::write(dir.path().join("config/defaults.env"), "exit 3\n").unwrap();
        let err = source_config(dir.path(), &map(&[("PATH", "/usr/bin:/bin")])).unwrap_err();
        assert!(err.contains("exit 3"), "{err}");
    }

    /// The built-in fallbacks must equal what defaults.env produces (no local.env, no
    /// XDG overrides), so a standalone binary behaves like the framework.
    #[test]
    fn fallbacks_match_defaults_env() {
        let root = repo_root();
        let home = "/home/fallback-test";
        let out = Command::new("bash")
            .args([
                "--noprofile",
                "--norc",
                "-c",
                r#"set -a; . "$1/config/defaults.env"; env -0"#,
                "t",
            ])
            .arg(&root)
            .env_clear()
            .env("HOME", home)
            .env("PATH", "/usr/bin:/bin")
            .output()
            .unwrap();
        let cfg: HashMap<String, String> = out
            .stdout
            .split(|b| *b == 0)
            .filter_map(|kv| {
                std::str::from_utf8(kv)
                    .ok()?
                    .split_once('=')
                    .map(|(k, v)| (k.to_string(), v.to_string()))
            })
            .collect();
        let env = Env::resolve(&HashMap::new(), &HashMap::new(), None, home);
        for (key, _) in KEYS {
            if let Some(defaults) = cfg.get(*key) {
                assert_eq!(
                    env.get(key),
                    defaults,
                    "fallback for {key} drifted from defaults.env"
                );
            }
        }
    }
}
