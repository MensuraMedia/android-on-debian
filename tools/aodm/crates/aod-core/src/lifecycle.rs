//! Start, stop and health for many images at once. Returns one outcome per image;
//! front ends (CLI, TUI) only render them (and get progress through `Ctx::say`).

use std::path::{Path, PathBuf};
use std::process::Child;
use std::time::Duration;

use crate::adb::Adb;
use crate::avd::Image;
use crate::budget::{self, Host, Want};
use crate::console::{self, Console};
use crate::emu::{self, Launch};
use crate::env::Env;
use crate::error::{Error, Result};
use crate::log::Logger;
use crate::ports::{self, PortLock};
use crate::proc::{self, EmuProc};
use crate::registry::Registry;

pub struct Ctx<'a> {
    pub env: &'a Env,
    pub log: &'a Logger,
    pub adb: Adb,
    pub emulator: PathBuf,
    /// Progress for the user (the CLI prints to stderr; tests and the TUI choose).
    pub say: fn(&str),
    token: Option<String>,
}

impl<'a> Ctx<'a> {
    pub fn new(env: &'a Env, log: &'a Logger, say: fn(&str)) -> Ctx<'a> {
        let sdk = env.path("ANDROID_HOME");
        Ctx {
            env,
            log,
            adb: Adb::new(&sdk),
            emulator: sdk.join("emulator/emulator"),
            say,
            token: console::token(&PathBuf::from(std::env::var("HOME").unwrap_or_default())),
        }
    }

    /// Launch settings from config; flags from the command line are applied on top.
    pub fn launch(&self, window: bool) -> Result<Launch> {
        Ok(Launch {
            gpu: self.env.get("ADX_EMU_GPU").to_string(),
            headless: !window && self.env.get("ADX_EMU_HEADLESS") == "true",
            env: spawn_env(self.env)?,
            log_keep: self.env.num("ADX_LOG_KEEP", 5),
            ..Launch::default()
        })
    }
}

/// What the emulator must see to find the same SDK and images aodm scanned, even when
/// aodm runs from a shell without them (adx exports its config; aodm reads it itself).
fn spawn_env(env: &Env) -> Result<Vec<(String, String)>> {
    let mut v: Vec<(String, String)> = [
        "ANDROID_HOME",
        "ANDROID_AVD_HOME",
        "ANDROID_USER_HOME",
        "ANDROID_EMULATOR_HOME",
    ]
    .iter()
    .filter(|k| !env.get(k).is_empty())
    .map(|k| (k.to_string(), env.get(k).to_string()))
    .collect();
    v.extend(emu::parse_env(env.get("ADX_EMU_ENV"))?);
    Ok(v)
}

/// Result for one image: `Ok(message)` or the error.
pub struct Outcome {
    pub image: String,
    pub result: Result<String>,
}

fn cores(i: &Image) -> u64 {
    i.config
        .as_ref()
        .and_then(|c| c.get("hw.cpu.ncore"))
        .and_then(|v| v.parse().ok())
        .unwrap_or(2)
}

fn port_cfg(env: &Env, key: &str) -> Result<u16> {
    env.get(key)
        .parse()
        .map_err(|_| Error::Config(format!("{key}={}: not a port number", env.get(key))))
}

/// Ports other images must not take: registry pins, and adx's own AVD on ADX_EMU_PORT.
/// Pins must be adb-visible slots and unique, or nothing starts.
pub fn reserved(env: &Env, reg: &Registry) -> Result<Vec<(String, u16)>> {
    let slots = ports::slots(ports::FIRST)?;
    let adx = (
        env.get("ADX_AVD_NAME").to_string(),
        port_cfg(env, "ADX_EMU_PORT")?,
    );
    let mut r: Vec<(String, u16)> = Vec::new();
    for (name, e) in &reg.images {
        let Some(p) = e.port else { continue };
        if !slots.contains(&p) {
            return Err(Error::Config(format!(
                "images.toml: {name}: port {p} must be even and within {}..{} (adb's range)",
                ports::FIRST,
                ports::LAST
            )));
        }
        if let Some((other, _)) = r.iter().chain([&adx]).find(|(n, q)| *q == p && n != name) {
            return Err(Error::Config(format!(
                "images.toml: {name}: port {p} is already {other}'s"
            )));
        }
        r.push((name.clone(), p));
    }
    if !r.iter().any(|(n, _)| *n == adx.0) {
        r.push(adx);
    }
    Ok(r)
}

/// Starts the stopped images among `targets` and waits for all of them to boot.
/// `Err` only when nothing may start (budget, bad config); per-image failures are
/// outcomes. Also returns budget warnings. Running state, locks and the budget are
/// read after taking `ports.lock`, so a concurrent start is seen.
pub fn start(
    ctx: &Ctx,
    targets: &[&Image],
    all: &[Image],
    reg: &Registry,
    launch: &Launch,
) -> Result<(Vec<String>, Vec<Outcome>)> {
    let slots = ports::slots(
        u16::try_from(ctx.env.num("ADX_AOD_BASE_PORT", 5554)).map_err(|_| {
            Error::Config(format!(
                "ADX_AOD_BASE_PORT={}: not a port",
                ctx.env.get("ADX_AOD_BASE_PORT")
            ))
        })?,
    )?;
    let reserved = reserved(ctx.env, reg)?;
    // Held until every emulator of this run has booted, so a concurrent aodm start can't
    // pick a port an emulator here hasn't bound yet.
    let _lock = PortLock::acquire(&ctx.env.path("ADX_AOD_HOME"), || {
        (ctx.say)("waiting for another aodm start to finish (ports.lock) ...")
    })?;
    let procs = proc::emulators();
    let mut out = Vec::new();
    let mut todo = Vec::new();
    for i in targets {
        if let Some(p) = emu::running(&procs, i) {
            if launch.wipe {
                out.push(err(
                    &i.name,
                    Error::Refused("running, not wiped: use `aodm restart --wipe`".into()),
                ));
            } else {
                let at = p
                    .port
                    .map(|p| format!(" on emulator-{p}"))
                    .unwrap_or_default();
                out.push(ok(&i.name, format!("already running{at}")));
            }
        } else if let Some(problem) = &i.problem {
            out.push(err(&i.name, Error::Refused(format!("broken: {problem}"))));
        } else if emu::in_use(&i.dir) && {
            (ctx.say)(&format!(
                "{}: waiting for its image lock to be released ...",
                i.name
            ));
            !emu::free_within(&i.dir, Duration::from_secs(10))
        } {
            out.push(err(
                &i.name,
                Error::Refused("image files are locked by another process (another user's or a foreign emulator?)".into()),
            ));
        } else if proc::running(&procs, &i.name).is_some() {
            out.push(err(
                &i.name,
                Error::Refused(
                    "an emulator of the same name from another AVD home is running; stop it first"
                        .into(),
                ),
            ));
        } else {
            todo.push(*i);
        }
    }
    if todo.is_empty() {
        return Ok((Vec::new(), out));
    }
    let want: Vec<Want> = todo
        .iter()
        .map(|i| Want {
            name: &i.name,
            ram_mb: budget::ram_mb(i.ram()).unwrap_or(2048),
            cores: cores(i),
        })
        .collect();
    let host = Host {
        running: procs.len(),
        max_instances: ctx.env.num("ADX_AOD_MAX_INSTANCES", 3),
        avail_mb: crate::card::meminfo("MemAvailable:").unwrap_or(0) / (1024 * 1024),
        reserve_mb: ctx.env.num("ADX_AOD_RAM_RESERVE_MB", 2048),
        threads: std::thread::available_parallelism().map_or(1, |n| n.get() as u64),
        running_cores: procs
            .iter()
            .map(|p| all.iter().find(|i| i.name == p.avd).map_or(2, cores))
            .sum(),
    };
    let warnings = budget::check(&host, &want)?;
    for w in &warnings {
        ctx.log.warn(w);
    }
    (ctx.say)(&format!(
        "starting {} image(s); booting takes a minute or more ...",
        todo.len()
    ));
    let mut used: Vec<u16> = procs.iter().filter_map(|p| p.port).collect();
    let log_dir = ctx.env.path("ADX_LOG_DIR");
    let mut booting: Vec<(&Image, u16, Child, PathBuf)> = Vec::new();
    for i in todo {
        let started = ports::pick(&i.name, &slots, &reserved, &used, ports::bindable).and_then(|port| {
            let log = emu::log_path(&log_dir, &i.name);
            let l = Launch {
                offline: reg.offline(&i.name),
                ..launch.clone()
            };
            let child = emu::spawn(&ctx.emulator, &i.name, port, &l, &log)?;
            ctx.log.info(&format!(
                "start {} on emulator-{port} (pid {}, gpu {}, headless {}, cold {}, wipe {}, offline {}, log {})",
                i.name,
                child.id(),
                l.gpu,
                l.headless,
                l.cold,
                l.wipe,
                l.offline,
                log.display()
            ));
            let lc = emu::logcat_path(&log_dir, &i.name);
            let wait = Duration::from_secs(ctx.env.num("ADX_EMU_BOOT_TIMEOUT", 240));
            match emu::logcat(&ctx.adb.bin, &format!("emulator-{port}"), &lc, l.log_keep, wait) {
                Ok(()) => ctx.log.info(&format!("{}: logcat to {}", i.name, lc.display())),
                Err(e) => ctx.log.warn(&format!("{}: logcat not saved: {e}", i.name)),
            }
            Ok((port, child, log))
        });
        match started {
            Ok((port, child, log)) => {
                used.push(port);
                booting.push((i, port, child, log));
            }
            Err(e) => out.push(err(&i.name, e)),
        }
    }
    let timeout = Duration::from_secs(ctx.env.num("ADX_EMU_BOOT_TIMEOUT", 240));
    let names: Vec<String> = booting.iter().map(|b| b.0.name.clone()).collect();
    let booted = parallel(&names, booting.iter_mut(), |(i, port, child, log)| {
        let serial = format!("emulator-{port}");
        let r = emu::wait_boot(child, &ctx.adb, &serial, timeout, log);
        if r.is_err() {
            abandon(ctx, i, *port, child);
        }
        r.map(|t| format!("booted on {serial} in {} s", t.as_secs()))
    });
    for o in &booted {
        match &o.result {
            Ok(m) => ctx.log.info(&format!("{}: {m}", o.image)),
            Err(e) => ctx.log.error(&format!("{}: start failed: {e}", o.image)),
        }
    }
    out.extend(booted);
    Ok((warnings, out))
}

/// A boot that failed is shut down the safe way (console kill after an identity check,
/// then SIGTERM), never SIGKILL, so it doesn't hold a port or RAM. An emulator that
/// already exited is only reaped.
fn abandon(ctx: &Ctx, i: &Image, port: u16, child: &mut Child) {
    if let Ok(Some(_)) = child.try_wait() {
        return;
    }
    let p = EmuProc {
        pid: child.id(),
        avd: i.name.clone(),
        port: Some(port),
    };
    match emu::stop(&p, ctx.token.as_deref(), Duration::from_secs(30)) {
        Ok(_) => {
            let _ = child.wait();
        }
        Err(e) => ctx
            .log
            .error(&format!("{}: could not stop the failed boot: {e}", i.name)),
    }
}

/// Runs `f` for each item on its own thread; a panicking thread becomes a failed outcome.
fn parallel<T: Send>(
    names: &[String],
    items: impl Iterator<Item = T>,
    f: impl Fn(T) -> Result<String> + Sync,
) -> Vec<Outcome> {
    std::thread::scope(|s| {
        let handles: Vec<_> = items.map(|it| s.spawn(|| f(it))).collect();
        handles
            .into_iter()
            .zip(names)
            .map(|(h, name)| Outcome {
                image: name.clone(),
                result: h.join().unwrap_or_else(|_| {
                    Err(Error::Failed("internal error (thread panicked)".into()))
                }),
            })
            .collect()
    })
}

/// Stops the given running emulators (in parallel; each may take a while to save).
pub fn stop(ctx: &Ctx, running: &[&EmuProc]) -> Vec<Outcome> {
    let names: Vec<String> = running.iter().map(|p| p.avd.clone()).collect();
    let out = parallel(&names, running.iter(), |p| {
        ctx.log.info(&format!("stop {} (pid {})", p.avd, p.pid));
        emu::stop(p, ctx.token.as_deref(), Duration::from_secs(30)).map(|how| match how {
            emu::Stopped::Console => "stopped".to_string(),
            emu::Stopped::Sigterm(why) => {
                ctx.log.warn(&format!(
                    "{}: console stop failed ({why}); used SIGTERM",
                    p.avd
                ));
                format!("stopped with SIGTERM ({why})")
            }
        })
    });
    for o in &out {
        match &o.result {
            Ok(m) => ctx.log.info(&format!("{}: {m}", o.image)),
            Err(e) => ctx.log.error(&format!("{}: stop failed: {e}", o.image)),
        }
    }
    out
}

/// One image's health. `ok` is false when something needs attention.
#[derive(serde::Serialize)]
pub struct Health {
    pub image: String,
    pub state: String,
    pub serial: String,
    pub adb: String,
    pub boot: String,
    pub console: String,
    pub note: String,
    pub ok: bool,
}

pub fn health(ctx: &Ctx, targets: &[&Image], procs: &[EmuProc]) -> Vec<Health> {
    let log_dir = ctx.env.path("ADX_LOG_DIR");
    targets
        .iter()
        .map(|i| health_one(ctx, i, procs, &log_dir))
        .collect()
}

fn health_one(ctx: &Ctx, i: &Image, procs: &[EmuProc], log_dir: &Path) -> Health {
    let dash = || "-".to_string();
    let errors = emu::log_errors(&emu::log_path(log_dir, &i.name));
    let last_run = if errors.is_empty() {
        String::new()
    } else {
        format!("{} error line(s) in emulator log", errors.len())
    };
    let idle = |state: &str, note: String, ok: bool| Health {
        image: i.name.clone(),
        state: state.into(),
        serial: dash(),
        adb: dash(),
        boot: dash(),
        console: dash(),
        note,
        ok,
    };
    let Some(p) = emu::running(procs, i) else {
        return match &i.problem {
            Some(pr) => idle("broken", pr.clone(), false),
            None if emu::in_use(&i.dir) => idle(
                "locked",
                "files locked by a process aodm can't see".into(),
                false,
            ),
            None if proc::running(procs, &i.name).is_some() => idle(
                "stopped",
                "a same-named emulator from another AVD home is running".into(),
                true,
            ),
            None => idle("stopped", last_run, true),
        };
    };
    let Some(port) = p.port else {
        return Health {
            state: format!("running (pid {})", p.pid),
            serial: "?".into(),
            ..idle(
                "",
                "started without -port: aodm can't tell its serial".into(),
                false,
            )
        };
    };
    let serial = format!("emulator-{port}");
    let adb = ctx.adb.state(&serial);
    let booted = adb == "device" && ctx.adb.booted(&serial);
    let console = Console::connect(port, ctx.token.as_deref(), Duration::from_secs(3))
        .and_then(|mut c| Ok((c.command("avd name")?, c.command("avd status")?)));
    let (console, mut notes) = match console {
        Ok((name, _)) if name.trim() != i.name => (
            "wrong image".to_string(),
            vec![format!("console :{port} answers as '{}'", name.trim())],
        ),
        Ok((_, status)) => (
            status
                .trim()
                .trim_start_matches("virtual device is ")
                .to_string(),
            vec![],
        ),
        Err(e) => ("no answer".to_string(), vec![e.to_string()]),
    };
    if adb != "device" {
        notes.push(match adb.as_str() {
            "unauthorized" => "adb unauthorized: aodm restart --wipe".into(),
            "not found" => "adb lost it: `adb kill-server` (affects every device)".into(),
            _ => format!("adb: {adb}"),
        });
    }
    if !last_run.is_empty() {
        notes.push(last_run);
    }
    Health {
        image: i.name.clone(),
        state: format!("running (pid {})", p.pid),
        serial,
        adb,
        boot: if booted { "done".into() } else { "no".into() },
        ok: booted && console == "running",
        console,
        note: notes.join("; "),
    }
}

fn ok(image: &str, m: String) -> Outcome {
    Outcome {
        image: image.to_string(),
        result: Ok(m),
    }
}

fn err(image: &str, e: Error) -> Outcome {
    Outcome {
        image: image.to_string(),
        result: Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> Env {
        let cfg: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Env::resolve(&HashMap::new(), &cfg, None, "/home/u")
    }

    fn reg(pins: &[(&str, u16)]) -> Registry {
        let mut r = Registry::default();
        for (n, p) in pins {
            r.images.entry(n.to_string()).or_default().port = Some(*p);
        }
        r
    }

    #[test]
    fn reserved_ports_are_validated() {
        let e = env(&[("ADX_AVD_NAME", "adx_api37"), ("ADX_EMU_PORT", "5554")]);
        assert_eq!(
            reserved(&e, &reg(&[("a", 5558)])).unwrap(),
            [("a".to_string(), 5558), ("adx_api37".to_string(), 5554)]
        );
        for bad in [
            &[("a", 5600)][..],
            &[("a", 5557)],
            &[("a", 5554)],
            &[("a", 5558), ("b", 5558)],
        ] {
            assert!(reserved(&e, &reg(bad)).is_err(), "{bad:?}");
        }
        assert!(
            reserved(&e, &reg(&[("adx_api37", 5556)])).is_ok(),
            "adx's own AVD may be re-pinned"
        );
        let bad_port = env(&[("ADX_AVD_NAME", "adx_api37"), ("ADX_EMU_PORT", "x")]);
        assert!(reserved(&bad_port, &Registry::default()).is_err());
    }

    #[test]
    fn emulator_gets_resolved_locations() {
        let e = env(&[
            ("ANDROID_HOME", "/sdk"),
            ("ANDROID_AVD_HOME", "/sandbox/avd"),
            ("ADX_EMU_ENV", "__NV_PRIME_RENDER_OFFLOAD=1"),
        ]);
        let v = spawn_env(&e).unwrap();
        assert!(v.contains(&("ANDROID_HOME".into(), "/sdk".into())));
        assert!(v.contains(&("ANDROID_AVD_HOME".into(), "/sandbox/avd".into())));
        assert_eq!(
            v.last().unwrap(),
            &("__NV_PRIME_RENDER_OFFLOAD".to_string(), "1".to_string())
        );
    }

    #[test]
    fn panicking_thread_is_a_failure() {
        let names = vec!["a".to_string(), "b".to_string()];
        let out = parallel(&names, [1, 2].into_iter(), |n| {
            if n == 2 {
                panic!("boom")
            } else {
                Ok("fine".into())
            }
        });
        assert!(out[0].result.is_ok());
        assert_eq!(out[1].image, "b");
        assert!(out[1].result.is_err());
    }
}
