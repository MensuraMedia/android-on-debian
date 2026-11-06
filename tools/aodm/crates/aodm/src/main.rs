//! `aodm`: android-on-debian phone image manager.
//! P2: `list`, `card`, `env`. P3: `start`, `stop`, `restart`, `health`. Saves,
//! settings, templates, identity, spin-offs and the TUI arrive later (see ROADMAP.md).

use anyhow::Context;
use aod_core::card::{self, Build};
use aod_core::env::Env;
use aod_core::lifecycle::{self, Ctx, Outcome};
use aod_core::log::Logger;
use aod_core::registry::Registry;
use aod_core::{avd, proc};
use clap::{Args, Parser, Subcommand};
use std::io::{BufRead, IsTerminal, Write};

const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " (", env!("AODM_GIT"), ")");

#[derive(Parser)]
#[command(name = "aodm", version = VERSION, about = "Manage android-on-debian phone images")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// List phone images with state, API, device, size and groups.
    List {
        /// Only images in this group.
        #[arg(long)]
        group: Option<String>,
    },
    /// Version card: aodm/adx versions, host specs, toolchain, emulation, images.
    Card,
    /// Show the configuration aodm uses and where each value comes from.
    Env,
    /// Start images and wait until they have booted (ports and RAM budget checked).
    Start {
        #[command(flatten)]
        targets: Targets,
        #[command(flatten)]
        opts: StartOpts,
    },
    /// Stop running images (console `kill`, saves the quick-boot snapshot).
    Stop {
        #[command(flatten)]
        targets: Targets,
    },
    /// Stop, then start again.
    Restart {
        #[command(flatten)]
        targets: Targets,
        #[command(flatten)]
        opts: StartOpts,
    },
    /// Check process, adb, boot and console of each image; exit 1 if any needs attention.
    Health {
        #[command(flatten)]
        targets: Targets,
    },
}

/// Which images: names, or --all / --group; --running narrows to running ones.
#[derive(Args)]
struct Targets {
    /// Image names.
    names: Vec<String>,
    /// Every image.
    #[arg(long, conflicts_with_all = ["names", "group"])]
    all: bool,
    /// Images in this group.
    #[arg(long, conflicts_with = "names")]
    group: Option<String>,
    /// Only running images (alone: all running images).
    #[arg(long)]
    running: bool,
}

#[derive(Args)]
struct StartOpts {
    /// Show the emulator window (default follows ADX_EMU_HEADLESS).
    #[arg(long)]
    window: bool,
    /// Cold boot: ignore the quick-boot snapshot.
    #[arg(long)]
    cold: bool,
    /// Factory reset: erases the image's user data (asks to confirm).
    #[arg(long)]
    wipe: bool,
    /// Don't ask before --wipe.
    #[arg(long)]
    yes: bool,
}

fn main() {
    let cli = Cli::parse();
    let env = Env::load();
    let log = Logger::new(&env);
    if let Some(e) = &env.config_error {
        eprintln!("aodm: warning: {e}");
        log.warn(e);
    }
    // Every failure is reported once on stderr and recorded in the framework log.
    if let Err(e) = run(cli.cmd, &env, &log) {
        eprintln!("aodm: error: {e:#}");
        log.error(&format!("{e:#}"));
        std::process::exit(1);
    }
}

fn run(cmd: Cmd, env: &Env, log: &Logger) -> anyhow::Result<()> {
    match cmd {
        Cmd::List { group } => {
            let (mut images, procs, reg) = inventory(env, log)?;
            if let Some(g) = &group {
                images.retain(|i| reg.groups(&i.name).iter().any(|x| x == g));
            }
            let c = card::build(env, &build(), &images, &procs, &reg);
            log.info(&format!(
                "list: {} image(s){}",
                c.images.len(),
                group
                    .as_ref()
                    .map(|g| format!(" in group {g}"))
                    .unwrap_or_default()
            ));
            if c.images.is_empty()
                && let Some(g) = &group
            {
                println!("no images in group {g}");
                return Ok(());
            }
            print!("{}", card::render_images(&c.images));
            problems(&images);
        }
        Cmd::Card => {
            let (images, procs, reg) = inventory(env, log)?;
            log.info("card");
            print!(
                "{}",
                card::render_text(&card::build(env, &build(), &images, &procs, &reg))
            );
            problems(&images);
        }
        Cmd::Env => {
            log.info("env");
            let entries = env.entries();
            let width = entries.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
            for (key, v) in entries {
                println!("{key:<width$}  {:<8}  {}", v.source.as_str(), v.value);
            }
            for w in &env.ignored {
                eprintln!("aodm: note: {w}");
            }
        }
        Cmd::Start { targets, opts } => {
            let (images, procs, reg) = inventory(env, log)?;
            let sel = select(&targets, &images, &procs, &reg)?;
            // Running images are refused with --wipe, so only the others are asked about.
            let stopped: Vec<_> = sel
                .iter()
                .copied()
                .filter(|i| aod_core::emu::running(&procs, i).is_none())
                .collect();
            confirm_wipe(&stopped, &opts)?;
            let ctx = Ctx::new(env, log, say);
            report(&start(&ctx, &sel, &images, &reg, &opts)?)?;
        }
        Cmd::Stop { targets } => {
            let (images, procs, reg) = inventory(env, log)?;
            let sel = select(&targets, &images, &procs, &reg)?;
            let mut out = Vec::new();
            let mut running = Vec::new();
            for i in &sel {
                match aod_core::emu::running(&procs, i) {
                    Some(p) => running.push(p),
                    None => out.push(Outcome {
                        image: i.name.clone(),
                        result: Ok(not_running(i, &procs)),
                    }),
                }
            }
            out.extend(lifecycle::stop(&Ctx::new(env, log, say), &running));
            report(&out)?;
        }
        Cmd::Restart { targets, opts } => {
            let (images, procs, reg) = inventory(env, log)?;
            let sel = select(&targets, &images, &procs, &reg)?;
            confirm_wipe(&sel, &opts)?;
            let ctx = Ctx::new(env, log, say);
            let running: Vec<_> = sel
                .iter()
                .filter_map(|i| aod_core::emu::running(&procs, i))
                .collect();
            let stopped = lifecycle::stop(&ctx, &running);
            // One stuck emulator must not leave the rest of the group down: start every
            // image that is stopped now, report the failed stops alongside.
            let failed: Vec<&str> = stopped
                .iter()
                .filter(|o| o.result.is_err())
                .map(|o| o.image.as_str())
                .collect();
            let again: Vec<_> = sel
                .iter()
                .copied()
                .filter(|i| !failed.contains(&i.name.as_str()))
                .collect();
            let mut out: Vec<Outcome> = stopped.into_iter().filter(|o| o.result.is_err()).collect();
            if !again.is_empty() {
                out.extend(start(&ctx, &again, &images, &reg, &opts)?);
            }
            report(&out)?;
        }
        Cmd::Health { targets } => {
            let (images, procs, reg) = inventory(env, log)?;
            let sel = if targets.names.is_empty() && !targets.all && targets.group.is_none() {
                // No selector: every image (or, with --running alone, every running one).
                images
                    .iter()
                    .filter(|i| !targets.running || aod_core::emu::running(&procs, i).is_some())
                    .collect()
            } else {
                select(&targets, &images, &procs, &reg)?
            };
            if sel.is_empty() {
                println!("no {}images", if targets.running { "running " } else { "" });
                return Ok(());
            }
            let rows = lifecycle::health(&Ctx::new(env, log, say), &sel, &procs);
            let bad = rows.iter().filter(|h| !h.ok).count();
            log.info(&format!(
                "health: {} image(s), {bad} need attention",
                rows.len()
            ));
            print!("{}", health_table(&rows));
            if bad > 0 {
                anyhow::bail!("{bad} image(s) need attention");
            }
        }
    }
    Ok(())
}

fn say(msg: &str) {
    eprintln!("aodm: {msg}");
}

fn not_running(i: &avd::Image, procs: &[proc::EmuProc]) -> String {
    if proc::running(procs, &i.name).is_some() {
        "not running here (a same-named emulator from another AVD home is; not touched)".into()
    } else {
        "not running".into()
    }
}

fn start(
    ctx: &Ctx,
    sel: &[&avd::Image],
    images: &[avd::Image],
    reg: &Registry,
    opts: &StartOpts,
) -> anyhow::Result<Vec<Outcome>> {
    let mut launch = ctx.launch(opts.window)?;
    launch.cold = opts.cold;
    launch.wipe = opts.wipe;
    let (warnings, out) = lifecycle::start(ctx, sel, images, reg, &launch)?;
    for w in warnings {
        eprintln!("aodm: warning: {w}");
    }
    Ok(out)
}

/// Prints one line per image; fails if any image failed.
fn report(out: &[Outcome]) -> anyhow::Result<()> {
    let mut failed = 0;
    for o in out {
        match &o.result {
            Ok(m) => println!("{}: {m}", o.image),
            Err(e) => {
                failed += 1;
                eprintln!("{}: FAILED: {e}", o.image);
            }
        }
    }
    if failed > 0 {
        anyhow::bail!("{failed} of {} image(s) failed", out.len());
    }
    Ok(())
}

/// --wipe destroys user data: confirm by typing `wipe`, or pass --yes.
fn confirm_wipe(sel: &[&avd::Image], opts: &StartOpts) -> anyhow::Result<()> {
    if !opts.wipe || opts.yes || sel.is_empty() {
        return Ok(());
    }
    let names = sel
        .iter()
        .map(|i| i.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("--wipe erases the user data of {names}: add --yes to confirm");
    }
    print!("--wipe erases all user data of {names}. Type 'wipe' to continue: ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    if line.trim() != "wipe" {
        anyhow::bail!("not confirmed; nothing changed");
    }
    Ok(())
}

/// Resolves a selector to images, in inventory order. Unknown names are an error.
fn select<'a>(
    t: &Targets,
    images: &'a [avd::Image],
    procs: &[proc::EmuProc],
    reg: &Registry,
) -> anyhow::Result<Vec<&'a avd::Image>> {
    for n in &t.names {
        if !images.iter().any(|i| &i.name == n) {
            anyhow::bail!("no image named '{n}' (aodm list)");
        }
    }
    let any = t.all || t.running;
    let sel: Vec<_> = images
        .iter()
        .filter(|i| {
            t.names.contains(&i.name)
                || t.group
                    .as_ref()
                    .is_some_and(|g| reg.groups(&i.name).contains(g))
                || (any && t.names.is_empty() && t.group.is_none())
        })
        .filter(|i| !t.running || aod_core::emu::running(procs, i).is_some())
        .collect();
    if sel.is_empty() {
        if t.names.is_empty() && !any && t.group.is_none() {
            anyhow::bail!("which images? give names, --all, --group G or --running");
        }
        if t.running {
            anyhow::bail!("no matching images (--running: none of them is running)");
        }
        anyhow::bail!("no matching images");
    }
    Ok(sel)
}

fn health_table(rows: &[lifecycle::Health]) -> String {
    let head = ["IMAGE", "STATE", "SERIAL", "ADB", "BOOT", "CONSOLE", "NOTE"];
    let cells: Vec<[&str; 7]> = rows
        .iter()
        .map(|h| {
            [
                &*h.image,
                &*h.state,
                &*h.serial,
                &*h.adb,
                &*h.boot,
                &*h.console,
                &*h.note,
            ]
        })
        .collect();
    let mut w = head.map(str::len);
    for c in &cells {
        for (k, v) in c.iter().enumerate() {
            w[k] = w[k].max(v.len());
        }
    }
    let line = |c: [&str; 7]| {
        let mut s = String::new();
        for (k, v) in c.iter().enumerate() {
            if k == 6 {
                s.push_str(v);
            } else {
                s.push_str(&format!("{v:<width$}  ", width = w[k]));
            }
        }
        s.trim_end().to_string() + "\n"
    };
    std::iter::once(head).chain(cells).map(line).collect()
}

/// Broken images are listed (so they can be fixed or removed); say why under the table.
fn problems(images: &[avd::Image]) {
    for i in images {
        if let Some(p) = &i.problem {
            eprintln!("aodm: {} is broken: {p}", i.name);
        }
    }
}

fn build() -> Build<'static> {
    Build {
        aodm: VERSION,
        rustc: env!("AODM_RUSTC"),
    }
}

fn inventory(
    env: &Env,
    log: &Logger,
) -> anyhow::Result<(Vec<avd::Image>, Vec<proc::EmuProc>, Registry)> {
    let images = avd::scan(&env.path("ANDROID_AVD_HOME"), &env.android_user_home())
        .context("scanning phone images")?;
    // A broken registry must not hide the images: warn and continue without it.
    let reg = Registry::load(&env.path("ADX_AOD_HOME")).unwrap_or_else(|e| {
        eprintln!("aodm: warning: registry ignored: {e}");
        log.warn(&format!("registry ignored: {e}"));
        Registry::default()
    });
    Ok((images, proc::emulators(), reg))
}
