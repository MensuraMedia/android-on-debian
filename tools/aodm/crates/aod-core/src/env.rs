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
}

/// Keys aodm reads, with fallbacks used only when the framework config is unavailable.
/// shortcut: fallbacks duplicate defaults.env; a test (fallbacks_match_defaults_env)
/// keeps them in sync while both live in this repo.
const KEYS: &[(&str, &str)] = &[
    ("ADX_ROOT", ""),
    ("ANDROID_HOME", "~/Android/Sdk"),
    ("ANDROID_AVD_HOME", "~/.android/avd"),
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
        Env {
            vars,
            config_error: None,
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

    /// All known keys with value and source, sorted (for `aodm env`).
    pub fn entries(&self) -> Vec<(&str, &Value)> {
        let mut v: Vec<_> = self.vars.iter().map(|(k, v)| (k.as_str(), v)).collect();
        v.sort_by_key(|(k, _)| *k);
        v
    }
}

/// Sources defaults.env then local.env in bash with the given environment (adx's own
/// semantics) and returns the resulting ADX_* / ANDROID_* variables. `BASH_ENV`/`ENV`
/// are removed so the environment can't inject code into this shell.
fn source_config(
    root: &Path,
    process: &HashMap<String, String>,
) -> Result<HashMap<String, String>, String> {
    let script = r#"set -a; . "$1/config/defaults.env"; if [ -f "$1/config/local.env" ]; then . "$1/config/local.env"; fi; env -0"#;
    let mut cmd = Command::new("bash");
    cmd.args(["--noprofile", "--norc", "-c", script, "aodm-config"])
        .arg(root)
        .env_clear()
        .envs(
            process
                .iter()
                .filter(|(k, _)| !matches!(k.as_str(), "BASH_ENV" | "ENV")),
        );
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
        Path::new(env!("CARGO_MANIFEST_DIR"))
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
