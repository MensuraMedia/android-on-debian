//! Configuration, with one source of truth: `config/defaults.env` + `config/local.env`.
//!
//! Precedence per key:
//! 1. the process environment (under `adx`, or a shell that sourced `lib/shell-env.sh`,
//!    this already equals the config files);
//! 2. the config files, sourced through bash when the framework is found
//!    (`ADX_ROOT` in the environment, or baked in at build time);
//! 3. built-in fallbacks (only when neither is available).

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
}

/// Keys aodm reads, with fallbacks used only when the framework config is unavailable.
/// shortcut: fallbacks duplicate defaults.env; they apply only to a standalone binary
/// without the framework, upgrade to a generated table if they drift.
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
    ("ADX_EMU_GPU", "auto"),
    ("ADX_EMU_ENV", ""),
    ("ADX_EMU_HEADLESS", "true"),
    ("ADX_EMU_BOOT_TIMEOUT", "240"),
];

impl Env {
    /// Loads configuration from the real process environment and framework.
    pub fn load() -> Env {
        let process: HashMap<String, String> = std::env::vars().collect();
        let root = process
            .get("ADX_ROOT")
            .cloned()
            .or_else(|| option_env!("ADX_ROOT").map(str::to_string))
            .filter(|r| Path::new(r).join("config/defaults.env").is_file());
        // Under adx the process env already holds the config; skip the bash round trip.
        let config = match (&root, process.contains_key("ADX_LOG_DIR")) {
            (Some(r), false) => source_config(Path::new(r)),
            _ => HashMap::new(),
        };
        let home = process.get("HOME").cloned().unwrap_or_default();
        Env::resolve(&process, &config, root.as_deref(), &home)
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
            let v = if let Some(v) = process.get(*key) {
                Value {
                    value: v.clone(),
                    source: Source::Process,
                }
            } else if let Some(v) = config.get(*key) {
                Value {
                    value: v.clone(),
                    source: Source::Config,
                }
            } else if *key == "ADX_ROOT" {
                Value {
                    value: root.unwrap_or_default().to_string(),
                    source: Source::Fallback,
                }
            } else {
                Value {
                    value: expand(fallback),
                    source: Source::Fallback,
                }
            };
            vars.insert((*key).to_string(), v);
        }
        Env { vars }
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

/// Sources defaults.env then local.env in bash (the framework's own semantics) and
/// returns the resulting ADX_* / ANDROID_* variables. Empty on any failure.
fn source_config(root: &Path) -> HashMap<String, String> {
    let script = r#"set -a; . "$1/config/defaults.env"; [ -f "$1/config/local.env" ] && . "$1/config/local.env"; env -0"#;
    let out = Command::new("bash")
        .args(["-c", script, "aodm-config"])
        .arg(root)
        .env_clear()
        .env("HOME", std::env::var("HOME").unwrap_or_default())
        .env("PATH", "/usr/bin:/bin")
        .output();
    let Ok(out) = out else { return HashMap::new() };
    if !out.status.success() {
        return HashMap::new();
    }
    out.stdout
        .split(|b| *b == 0)
        .filter_map(|kv| std::str::from_utf8(kv).ok())
        .filter_map(|kv| kv.split_once('='))
        .filter(|(k, _)| k.starts_with("ADX_") || k.starts_with("ANDROID_"))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
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

    #[test]
    fn process_beats_config_beats_fallback() {
        let process = map(&[("ADX_EMU_GPU", "host")]);
        let config = map(&[("ADX_EMU_GPU", "software"), ("ADX_AOD_MAX_SAVES", "7")]);
        let env = Env::resolve(&process, &config, Some("/r"), "/home/u");
        assert_eq!(env.get("ADX_EMU_GPU"), "host");
        assert_eq!(env.vars["ADX_EMU_GPU"].source, Source::Process);
        assert_eq!(env.num("ADX_AOD_MAX_SAVES", 3), 7);
        assert_eq!(env.vars["ADX_AOD_MAX_SAVES"].source, Source::Config);
        assert_eq!(env.get("ADX_AOD_HOME"), "/home/u/.local/share/aod");
        assert_eq!(env.vars["ADX_AOD_HOME"].source, Source::Fallback);
        assert_eq!(env.get("ADX_ROOT"), "/r");
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
    fn sources_real_framework_config() {
        // tools/aodm/crates/aod-core -> repo root
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..");
        let cfg = source_config(&root);
        assert!(
            cfg.contains_key("ADX_LOG_DIR"),
            "defaults.env should define ADX_LOG_DIR"
        );
        assert!(cfg.contains_key("ADX_AOD_HOME"));
    }
}
