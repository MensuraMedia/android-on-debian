//! Start, stop and health for many images at once. Returns one outcome per image;
//! front ends (CLI, TUI) only render them.

use std::path::PathBuf;
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
    token: Option<String>,
}

impl<'a> Ctx<'a> {
    pub fn new(env: &'a Env, log: &'a Logger) -> Ctx<'a> {
        let sdk = env.path("ANDROID_HOME");
        Ctx {
            env,
            log,
            adb: Adb::new(&sdk),
            emulator: sdk.join("emulator/emulator"),
            token: console::token(&PathBuf::from(std::env::var("HOME").unwrap_or_default())),
        }
    }

    /// Launch settings from config; flags from the command line are applied on top.
    pub fn launch(&self, window: bool) -> Result<Launch> {
        Ok(Launch {
            gpu: self.env.get("ADX_EMU_GPU").to_string(),
            headless: !window && self.env.get("ADX_EMU_HEADLESS") == "true",
            env: emu::parse_env(self.env.get("ADX_EMU_ENV"))?,
            ..Launch::default()
        })
    }
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

/// Ports other images must not take: registry pins, and adx's own AVD on ADX_EMU_PORT.
fn reserved(env: &Env, reg: &Registry) -> Vec<(String, u16)> {
    let mut r: Vec<(String, u16)> = reg
        .images
        .iter()
        .filter_map(|(n, e)| Some((n.clone(), e.port?)))
        .collect();
    if let Ok(p) = env.get("ADX_EMU_PORT").parse()
        && !r.iter().any(|(n, _)| n == env.get("ADX_AVD_NAME"))
    {
        r.push((env.get("ADX_AVD_NAME").to_string(), p));
    }
    r
}

/// Starts the stopped images among `targets` and waits for all of them to boot.
/// `Err` only when nothing may start (budget, bad config); per-image failures are
/// outcomes. Also returns budget warnings.
pub fn start(
    ctx: &Ctx,
    targets: &[&Image],
    all: &[Image],
    procs: &[EmuProc],
    reg: &Registry,
    launch: &Launch,
) -> Result<(Vec<String>, Vec<Outcome>)> {
    let mut out = Vec::new();
    let mut todo = Vec::new();
    for i in targets {
        if let Some(p) = proc::running(procs, &i.name) {
            let at = p
                .port
                .map(|p| format!(" on emulator-{p}"))
                .unwrap_or_default();
            out.push(ok(&i.name, format!("already running{at}")));
        } else if let Some(problem) = &i.problem {
            out.push(err(&i.name, Error::Refused(format!("broken: {problem}"))));
        } else if emu::in_use(&i.dir) {
            out.push(err(&i.name, Error::Refused(
                "image files are locked by another process (another user's or a foreign emulator?)".into(),
            )));
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
    let slots = ports::slots(ctx.env.num("ADX_AOD_BASE_PORT", 5554) as u16)?;
    let reserved = reserved(ctx.env, reg);
    let mut used: Vec<u16> = procs.iter().filter_map(|p| p.port).collect();
    let log_dir = ctx.env.path("ADX_LOG_DIR");
    // Held until every emulator has booted, so a concurrent aodm start can't pick
    // a port that an emulator here hasn't bound yet.
    let _lock = PortLock::acquire(&ctx.env.path("ADX_AOD_HOME"))?;
    let mut booting: Vec<(&Image, u16, Child, PathBuf)> = Vec::new();
    for i in todo {
        let started =
            ports::pick(&i.name, &slots, &reserved, &used, ports::bindable).and_then(|port| {
                let log = emu::log_path(&log_dir, &i.name);
                ctx.log.info(&format!(
                    "start {} on emulator-{port} (gpu {}, headless {}, cold {}, wipe {}, log {})",
                    i.name,
                    launch.gpu,
                    launch.headless,
                    launch.cold,
                    launch.wipe,
                    log.display()
                ));
                Ok((
                    port,
                    emu::spawn(&ctx.emulator, &i.name, port, launch, &log)?,
                    log,
                ))
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
    let booted: Vec<Outcome> = std::thread::scope(|s| {
        let handles: Vec<_> = booting
            .iter_mut()
            .map(|(i, port, child, log)| {
                s.spawn(move || {
                    let serial = format!("emulator-{port}");
                    let r = emu::wait_boot(child, &ctx.adb, &serial, timeout, log);
                    if r.is_err() {
                        // Don't leave a half-booted emulator holding a port and RAM.
                        if let Ok(mut c) =
                            Console::connect(*port, ctx.token.as_deref(), Duration::from_secs(3))
                        {
                            let _ = c.command("kill");
                        }
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                    Outcome {
                        image: i.name.clone(),
                        result: r.map(|t| format!("booted on {serial} in {} s", t.as_secs())),
                    }
                })
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
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

/// Stops the given running emulators (in parallel; each may take a while to save).
pub fn stop(ctx: &Ctx, running: &[&EmuProc]) -> Vec<Outcome> {
    let out: Vec<Outcome> = std::thread::scope(|s| {
        let handles: Vec<_> = running
            .iter()
            .map(|p| {
                s.spawn(move || {
                    ctx.log.info(&format!("stop {} (pid {})", p.avd, p.pid));
                    let r =
                        emu::stop(p, ctx.token.as_deref(), Duration::from_secs(30)).map(|how| {
                            match how {
                                emu::Stopped::Console => "stopped".to_string(),
                                emu::Stopped::Sigterm => {
                                    "stopped (console didn't answer: SIGTERM)".to_string()
                                }
                            }
                        });
                    Outcome {
                        image: p.avd.clone(),
                        result: r,
                    }
                })
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
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
    let dash = || "-".to_string();
    let log_dir = ctx.env.path("ADX_LOG_DIR");
    targets
        .iter()
        .map(|i| {
            let errors = emu::log_errors(&emu::log_path(&log_dir, &i.name));
            let last_run = if errors.is_empty() {
                String::new()
            } else {
                format!("{} error line(s) in emulator log", errors.len())
            };
            let Some(p) = proc::running(procs, &i.name) else {
                let (state, note) = match &i.problem {
                    Some(pr) => ("broken", pr.clone()),
                    None if emu::in_use(&i.dir) => {
                        ("locked", "files locked by a process aodm can't see".into())
                    }
                    None => ("stopped", last_run),
                };
                return Health {
                    image: i.name.clone(),
                    state: state.into(),
                    serial: dash(),
                    adb: dash(),
                    boot: dash(),
                    console: dash(),
                    ok: state == "stopped",
                    note,
                };
            };
            let Some(port) = p.port else {
                return Health {
                    image: i.name.clone(),
                    state: format!("running (pid {})", p.pid),
                    serial: "?".into(),
                    adb: dash(),
                    boot: dash(),
                    console: dash(),
                    note: "started without -port: aodm can't tell its serial".into(),
                    ok: false,
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
            let ok = booted && console == "running";
            Health {
                image: i.name.clone(),
                state: format!("running (pid {})", p.pid),
                serial,
                adb,
                boot: if booted { "done".into() } else { "no".into() },
                console,
                note: notes.join("; "),
                ok,
            }
        })
        .collect()
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
