//! Administering the manager itself: doctor, config, logs, audit, disk, gc.

use std::io::{Read, Seek, SeekFrom, Write};
use std::time::Duration;

use aod_core::avd::{self, Image};
use aod_core::doctor::{self, Level};
use aod_core::env::Env;
use aod_core::lifecycle::Ctx;
use aod_core::log::{self, AUDIT};
use aod_core::{config, emu, storage};
use clap::Subcommand;
use serde_json::json;

use crate::out::{Out, Usage};

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Every setting with its value and source (like `aodm env`), problems marked.
    List,
    /// One value (prints only the value: for scripts).
    Get { key: String },
    /// Write KEY="VALUE" to config/local.env (checked first; other lines kept).
    Set { key: String, value: String },
    /// Remove KEY from config/local.env (back to defaults.env).
    Unset { key: String },
    /// Check every setting: types, ranges, absolute paths, port slots. Exit 1 on problems.
    Validate,
    /// Which files are read, in which order.
    Path,
}

pub fn doctor(ctx: &Ctx, imgs: &[Image], rev: &str, out: &mut Out) -> anyhow::Result<()> {
    let checks = doctor::run(ctx, imgs, rev);
    let w = checks.iter().map(|c| c.name.len()).max().unwrap_or(0);
    for c in &checks {
        out.line(
            format!("{:<4}  {:<w$}  {}", c.level.as_str(), c.name, c.detail),
            json!({"check": c.name, "level": c.level.as_str(), "detail": c.detail}),
        );
    }
    let failed = checks.iter().filter(|c| c.level == Level::Fail).count();
    if failed > 0 {
        anyhow::bail!("{failed} check(s) failed");
    }
    Ok(())
}

pub fn config(
    c: ConfigCmd,
    ctx: &Ctx,
    sandbox: bool,
    dry: bool,
    yes: bool,
    out: &mut Out,
) -> anyhow::Result<()> {
    let env = ctx.env;
    match c {
        ConfigCmd::List => {
            let problems = config::validate(env);
            let entries = env.entries();
            let w = entries.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
            for (k, v) in entries {
                let bad = problems.iter().any(|p| p.starts_with(&format!("{k}=")));
                out.line(
                    format!(
                        "{k:<w$}  {:<8}  {}{}",
                        v.source.as_str(),
                        v.value,
                        if bad {
                            "   <- invalid (aodm config validate)"
                        } else {
                            ""
                        }
                    ),
                    json!({"key": k, "value": v.value, "source": v.source.as_str(), "valid": !bad}),
                );
            }
        }
        ConfigCmd::Get { key } => {
            if !aod_core::env::KEYS.iter().any(|(k, _)| *k == key) {
                return Err(Usage(format!(
                    "{key} is not a setting aodm reads (aodm config list)"
                ))
                .into());
            }
            out.line(env.get(&key), json!({"key": key, "value": env.get(&key)}));
        }
        ConfigCmd::Set { key, value } => {
            refuse_sandbox(sandbox)?;
            let file = config::local_env(env)?;
            if dry {
                config::check_set(&key, &value)?;
                out.line(
                    format!("would set {key}=\"{value}\" in {}", file.display()),
                    json!({"key": key, "value": value, "file": file, "changed": false}),
                );
                return Ok(());
            }
            strands_data(env, &key, &value, yes)?;
            let old = config::set(&file, &key, &value)?;
            ctx.log.audit(
                "config-set",
                "-",
                &format!(
                    "{key}={value:?} (was {})",
                    old.as_deref().unwrap_or("unset")
                ),
            );
            out.line(
                format!(
                    "{key}=\"{value}\" written to {} (was: {})",
                    file.display(),
                    old.as_deref().unwrap_or("not set there")
                ),
                json!({"key": key, "value": value, "file": file, "previous": old, "changed": true}),
            );
        }
        ConfigCmd::Unset { key } => {
            refuse_sandbox(sandbox)?;
            let file = config::local_env(env)?;
            if dry {
                out.line(
                    format!("would remove {key} from {}", file.display()),
                    json!({"key": key, "file": file, "changed": false}),
                );
                return Ok(());
            }
            let old = config::unset(&file, &key)?;
            if let Some(o) = &old {
                ctx.log
                    .audit("config-unset", "-", &format!("{key} (was {o})"));
            }
            out.line(
                match &old {
                    Some(o) => format!("removed {o} from {}", file.display()),
                    None => format!("{key} was not set in {}", file.display()),
                },
                json!({"key": key, "file": file, "previous": old, "changed": old.is_some()}),
            );
            if old.is_some() && matches!(key.as_str(), "ADX_EMU_GPU" | "ADX_EMU_ENV") {
                crate::say(&format!(
                    "note: on hybrid-GPU machines `adx emulator install` (and `adx up`) sets {key} again; use `aodm config set` to keep your own value"
                ));
            }
        }
        ConfigCmd::Validate => {
            let problems = config::validate(env);
            for p in &problems {
                out.line(format!("invalid: {p}"), json!({"problem": p}));
            }
            if !problems.is_empty() {
                anyhow::bail!("{} setting(s) invalid", problems.len());
            }
            out.text("every setting is valid");
        }
        ConfigCmd::Path => {
            let root = env.get("ADX_ROOT");
            let mut files: Vec<(String, bool)> = Vec::new();
            if !root.is_empty() {
                for f in ["config/defaults.env", "config/local.env"] {
                    let p = std::path::Path::new(root).join(f);
                    files.push((p.display().to_string(), p.is_file()));
                }
            }
            for (n, (f, exists)) in files.iter().enumerate() {
                out.line(
                    format!("{}. {f}{}", n + 1, if *exists { "" } else { "  (missing)" }),
                    json!({"order": n + 1, "file": f, "exists": exists}),
                );
            }
            out.text(format!(
                "{}. the process environment, for keys the files leave unset",
                files.len() + 1
            ));
            out.text(format!("{}. built-in fallbacks", files.len() + 2));
            out.text("(later files win; aodm config set writes config/local.env)");
        }
    }
    Ok(())
}

/// Moving a data folder by setting its path leaves the data behind (and adx reads the
/// same file): refuse while the old folder holds something, unless --yes.
fn strands_data(env: &Env, key: &str, value: &str, yes: bool) -> anyhow::Result<()> {
    const DATA: &[&str] = &[
        "ANDROID_AVD_HOME",
        "ANDROID_USER_HOME",
        "ADX_AOD_HOME",
        "ADX_AOD_SAVES_DIR",
    ];
    let old = env.get(key);
    let used = std::fs::read_dir(old).is_ok_and(|mut d| d.next().is_some());
    if DATA.contains(&key) && old != value && used && !yes {
        return Err(aod_core::Error::Refused(format!(
            "{key}: {old} holds data that would not move (images, registry or saves; adx reads this setting too). Move it first (mv {old} {value}), then repeat with --yes"
        ))
        .into());
    }
    Ok(())
}

fn refuse_sandbox(sandbox: bool) -> anyhow::Result<()> {
    if sandbox {
        return Err(aod_core::Error::Refused(
            "--sandbox: config set/unset would edit the real config/local.env; not done".into(),
        )
        .into());
    }
    Ok(())
}

/// `aodm logs`: aodm's lines from adx.log, or one image's emulator log.
pub fn logs(
    env: &Env,
    lines: usize,
    follow: bool,
    image: Option<&str>,
    level: Option<&str>,
    out: &mut Out,
) -> anyhow::Result<()> {
    if follow && out.json {
        return Err(Usage("--follow and --json don't combine".into()).into());
    }
    if image.is_some() && level.is_some() {
        return Err(Usage("--level filters aodm's own log; not with --image".into()).into());
    }
    let dir = env.path("ADX_LOG_DIR");
    // None: an emulator log, every line. Some(n): aodm's lines at level rank n or above.
    let min = match (image, level.unwrap_or("info")) {
        (Some(_), _) => None,
        (None, "info") => Some(0),
        (None, "warn") => Some(1),
        (None, "error") => Some(2),
        (None, l) => return Err(Usage(format!("--level {l}: use info, warn or error")).into()),
    };
    let path = match image {
        Some(i) => {
            avd::validate_name(i)?;
            emu::log_path(&dir, i)
        }
        None => dir.join("adx.log"),
    };
    let keep = move |l: &str| match min {
        None => true,
        Some(min) => {
            let rank = if l.contains(" ERROR ") {
                2
            } else if l.contains(" WARN ") {
                1
            } else {
                0
            };
            l.contains("[aodm #") && rank >= min
        }
    };
    let all: Vec<String> = match image {
        // The emulator log of the current run (previous runs are emulator-NAME.log.N).
        Some(_) => std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?
            .lines()
            .map(str::to_string)
            .collect(),
        None => log::read_all(&path),
    };
    let picked: Vec<&String> = all.iter().filter(|l| keep(l)).collect();
    for l in &picked[picked.len().saturating_sub(lines)..] {
        out.line(l.as_str(), json!({"line": l}));
    }
    if follow {
        tail(&path, &keep)?;
    }
    Ok(())
}

/// Prints new lines as they arrive, until Ctrl-C. Follows rotation (file shrinks).
fn tail(path: &std::path::Path, keep: &dyn Fn(&str) -> bool) -> anyhow::Result<()> {
    let mut pos = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let mut rest = String::new();
    loop {
        std::thread::sleep(Duration::from_millis(500));
        let Ok(mut f) = std::fs::File::open(path) else {
            continue;
        };
        let len = f.metadata()?.len();
        if len < pos {
            pos = 0;
        }
        if len == pos {
            continue;
        }
        f.seek(SeekFrom::Start(pos))?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        pos += buf.len() as u64;
        rest.push_str(&String::from_utf8_lossy(&buf));
        while let Some(i) = rest.find('\n') {
            let l: String = rest.drain(..=i).collect();
            let l = l.trim_end();
            // Stop when the reader is gone (`aodm logs -f | head`).
            if keep(l) && writeln!(std::io::stdout(), "{l}").is_err() {
                return Ok(());
            }
        }
    }
}

/// One audit line, split: `2026-11-07 10:00:00 INFO  [aodm #42] audit: save phone: … (by u)`.
/// Anchored to the whole prefix aodm writes, so `audit:` quoted inside another
/// message (an error naming a user-supplied argument) never counts.
fn parse_audit(l: &str) -> Option<(&str, &str, &str, &str, &str)> {
    let time = l.get(..19)?;
    let rest = l.get(19..)?.strip_prefix(" INFO  [aodm #")?;
    let (run, rest) = rest.split_once("] ")?;
    if run.is_empty() || !run.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    let rest = rest.strip_prefix(AUDIT)?;
    let (action, rest) = rest.split_once(' ')?;
    let (image, detail) = rest.split_once(": ")?;
    Some((time, run, action, image, detail))
}

pub fn audit(
    env: &Env,
    since: Option<&str>,
    image: Option<&str>,
    out: &mut Out,
) -> anyhow::Result<()> {
    if let Some(s) = since {
        let ok = s.len() >= 10
            && s.len() <= 19
            && s.bytes().enumerate().all(|(i, b)| match i {
                4 | 7 => b == b'-',
                10 => b == b' ',
                13 | 16 => b == b':',
                _ => b.is_ascii_digit(),
            });
        if !ok {
            return Err(Usage(format!(
                "--since {s:?}: use YYYY-MM-DD or \"YYYY-MM-DD HH:MM[:SS]\""
            ))
            .into());
        }
    }
    let lines = log::read_all(&env.path("ADX_LOG_DIR").join("adx.log"));
    let mut n = 0;
    for l in &lines {
        let Some((time, run, action, img, detail)) = parse_audit(l) else {
            continue;
        };
        // Timestamps sort as text, so a date or date-time prefix works as a bound.
        if since.is_some_and(|s| time < s) || image.is_some_and(|i| i != img) {
            continue;
        }
        n += 1;
        out.line(
            format!("{time}  {action:<12}  {img:<16}  {detail}"),
            json!({"time": time, "run": run, "action": action, "image": img, "detail": detail}),
        );
    }
    if n == 0 {
        out.text("no audit entries match");
    }
    Ok(())
}

pub fn disk(ctx: &Ctx, imgs: &[Image], out: &mut Out) -> anyhow::Result<()> {
    let d = storage::disk(ctx, imgs)?;
    let w = d
        .images
        .iter()
        .map(|i| i.image.len())
        .max()
        .unwrap_or(5)
        .max(5);
    out.text(format!(
        "{:<w$}  {:>7}  {:>5}  {:>10}",
        "IMAGE", "SIZE", "SAVES", "SAVES SIZE"
    ));
    for i in &d.images {
        out.line(
            format!(
                "{:<w$}  {:>7}  {:>5}  {:>10}",
                i.image,
                avd::human(i.bytes),
                i.saves,
                avd::human(i.saves_bytes)
            ),
            json!({"kind": "image", "image": i.image, "bytes": i.bytes, "saves": i.saves, "saves_bytes": i.saves_bytes}),
        );
    }
    out.text("");
    for f in &d.filesystems {
        out.line(
            format!(
                "{} free on the filesystem of {} ({})",
                avd::human(f.free),
                f.path.display(),
                f.keys.join(", ")
            ),
            json!({"kind": "filesystem", "path": f.path, "settings": f.keys, "free": f.free}),
        );
    }
    if d.leftovers.is_empty() {
        out.text("no leftover work folders");
    }
    for (p, n, alive) in &d.leftovers {
        out.line(
            format!(
                "leftover {} ({}){}",
                p.display(),
                avd::human(*n),
                if *alive {
                    ", its run is still going"
                } else {
                    ", aodm gc removes it"
                }
            ),
            json!({"kind": "leftover", "path": p, "bytes": n, "alive": alive}),
        );
    }
    Ok(())
}

pub fn gc(ctx: &Ctx, dry: bool, out: &mut Out) -> anyhow::Result<()> {
    let plan = storage::gc_plan(ctx)?;
    if plan.is_empty() {
        out.text("nothing to clean");
        return Ok(());
    }
    for g in &plan {
        out.line(
            format!(
                "{} {} ({}, {})",
                if dry { "would remove" } else { "removing" },
                g.path.display(),
                g.why,
                avd::human(g.bytes)
            ),
            json!({"path": g.path, "why": g.why, "bytes": g.bytes, "removed": !dry}),
        );
    }
    if !dry {
        let freed = storage::gc(ctx, &plan)?;
        out.text(format!(
            "{} item(s) removed, {} freed",
            plan.len(),
            avd::human(freed)
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_lines_parse() {
        let l = "2026-11-07 10:00:00 INFO  [aodm #42] audit: save phone-a: 20261107-100000Z (4.9G) in 6.5 s (by user)";
        assert_eq!(
            parse_audit(l),
            Some((
                "2026-11-07 10:00:00",
                "42",
                "save",
                "phone-a",
                "20261107-100000Z (4.9G) in 6.5 s (by user)"
            ))
        );
        assert_eq!(
            parse_audit("2026-11-07 10:00:00 INFO  [aodm #42] list: 1 image(s)"),
            None
        );
        let forged =
            "2026-11-07 10:00:00 ERROR [aodm #42] no image named 'audit: delete x: gone (by root)'";
        assert_eq!(parse_audit(forged), None);
        let forged = "2026-11-07 10:00:00 INFO  [aodm #42] run: aodm show audit: delete x: y";
        assert_eq!(parse_audit(forged), None);
    }
}
