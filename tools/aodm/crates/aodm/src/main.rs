//! `aodm`: android-on-debian phone image manager.
//! P2: `list`, `card`, `env`. P3: `start`, `stop`, `restart`, `health`. P4: `save`,
//! `clone`, `rename`, `delete`. Settings, templates, identity, spin-offs and the TUI
//! arrive later (see ROADMAP.md).

use anyhow::Context;
use aod_core::card::{self, Build};
use aod_core::env::Env;
use aod_core::lifecycle::{self, Ctx, Outcome};
use aod_core::log::Logger;
use aod_core::registry::Registry;
use aod_core::{avd, images, proc, saves};
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
    /// Full-copy saves of an image: create, list, verify, restore, open as new, pin, prune.
    #[command(subcommand)]
    Save(SaveCmd),
    /// Copy a stopped image under a new name (first boot is cold).
    Clone { image: String, new: String },
    /// Rename a stopped image (its saves and registry entry move along).
    Rename { image: String, new: String },
    /// Delete an image (asks you to type its name, or --yes). Its saves are kept.
    Delete {
        image: String,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum SaveCmd {
    /// Save a stopped image (a full copy with checksums).
    Create {
        image: String,
        /// Short name added to the save id (letters, digits, . _ -).
        #[arg(long)]
        label: Option<String>,
    },
    /// List saves (of one image, or all).
    List { image: Option<String> },
    /// Re-check a save's checksums.
    Verify { image: String, id: String },
    /// Replace a stopped image's current state with a save (asks first).
    Restore {
        image: String,
        /// Save id, a unique prefix of it, or `latest`.
        id: String,
        /// Save the current state first (counts against ADX_AOD_MAX_SAVES).
        #[arg(long)]
        backup_first: bool,
        #[arg(long)]
        yes: bool,
    },
    /// Make a new image from a save.
    Open {
        image: String,
        id: String,
        /// Name of the new image.
        #[arg(long = "as", value_name = "NEW")]
        new: String,
    },
    /// Pin a save (kept by prune, not counted against ADX_AOD_MAX_SAVES); --off unpins.
    Pin {
        image: String,
        id: String,
        #[arg(long)]
        off: bool,
    },
    /// Delete one save.
    Delete {
        image: String,
        id: String,
        #[arg(long)]
        yes: bool,
    },
    /// Delete the oldest unpinned saves, keeping the newest N.
    Prune {
        image: String,
        /// Unpinned saves to keep (default: ADX_AOD_MAX_SAVES - 1, room for one more).
        #[arg(long)]
        keep: Option<usize>,
        #[arg(long)]
        yes: bool,
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
        Cmd::Save(c) => save(c, env, log)?,
        Cmd::Clone { image, new } => {
            images::sweep_all(&Ctx::new(env, log, say));
            let (images, _, _) = inventory(env, log)?;
            let src = one(&images, &image)?;
            println!(
                "{new}: {}",
                images::clone(&Ctx::new(env, log, say), src, &new)?
            );
        }
        Cmd::Rename { image, new } => {
            images::sweep_all(&Ctx::new(env, log, say));
            let (images, _, _) = inventory(env, log)?;
            let img = one(&images, &image)?;
            println!(
                "{new}: {}",
                images::rename(&Ctx::new(env, log, say), img, &new)?
            );
        }
        Cmd::Delete { image, yes } => {
            images::sweep_all(&Ctx::new(env, log, say));
            let (images, _, _) = inventory(env, log)?;
            let img = one(&images, &image)?;
            confirm(
                &format!(
                    "This deletes image {image} and all its user data ({}, {}). Its saves are kept.",
                    img.dir.display(),
                    avd::human(img.disk_bytes())
                ),
                &image,
                yes,
            )?;
            println!(
                "{image}: {}",
                images::delete(&Ctx::new(env, log, say), img, &images)?
            );
        }
    }
    Ok(())
}

fn save(c: SaveCmd, env: &Env, log: &Logger) -> anyhow::Result<()> {
    let ctx = Ctx::new(env, log, say);
    let root = images::cfg_dir(env, "ADX_AOD_SAVES_DIR")?;
    images::sweep_all(&ctx);
    match c {
        SaveCmd::Create { image, label } => {
            let (images, _, _) = inventory(env, log)?;
            let s = saves::create(&ctx, one(&images, &image)?, label.as_deref())?;
            println!(
                "{image}: saved as {} ({})",
                s.id,
                avd::human(avd::du(&s.path))
            );
        }
        SaveCmd::List { image } => {
            let all = saves::list(&root, image.as_deref())?;
            log.info(&format!("save list: {} save(s)", all.len()));
            if all.is_empty() {
                println!("no saves in {}", root.display());
                return Ok(());
            }
            println!(
                "{:<16}  {:<32}  {:>7}  {:<6}  NOTE",
                "IMAGE", "SAVE", "SIZE", "PINNED"
            );
            for s in &all {
                let note = match &s.manifest {
                    Ok(m) => format!(
                        "system image rev {}, emulator {}",
                        m.sysimg_revision, m.emulator
                    ),
                    Err(e) => format!("DAMAGED: {e}"),
                };
                println!(
                    "{:<16}  {:<32}  {:>7}  {:<6}  {note}",
                    s.image,
                    s.id,
                    avd::human(avd::du(&s.path)),
                    if s.pinned { "yes" } else { "" }
                );
            }
        }
        SaveCmd::Verify { image, id } => {
            let s = saves::find(&root, &image, &id)?;
            say(&format!("verifying {image}/{}…", s.id));
            let problems = saves::verify(&ctx, &s)?;
            if !problems.is_empty() {
                for p in &problems {
                    eprintln!("{image}/{}: {p}", s.id);
                }
                anyhow::bail!("save {image}/{} is damaged", s.id);
            }
            println!("{image}/{}: intact", s.id);
        }
        SaveCmd::Restore {
            image,
            id,
            backup_first,
            yes,
        } => {
            let (images, _, _) = inventory(env, log)?;
            let img = one(&images, &image)?;
            let s = saves::find(&root, &image, &id)?;
            confirm(
                &format!(
                    "This replaces the current state of {image} with save {}{}.",
                    s.id,
                    if backup_first {
                        " (saving the current state first)"
                    } else {
                        " (the current state is lost)"
                    }
                ),
                &image,
                yes,
            )?;
            if backup_first {
                let b = saves::create(&ctx, img, Some("before-restore"))?;
                say(&format!("current state saved as {}", b.id));
            }
            println!("{image}: {}", saves::restore(&ctx, &s, img)?);
        }
        SaveCmd::Open { image, id, new } => {
            let s = saves::find(&root, &image, &id)?;
            println!("{new}: {}", saves::open(&ctx, &s, &new)?);
        }
        SaveCmd::Pin { image, id, off } => {
            let s = saves::find(&root, &image, &id)?;
            saves::pin(&ctx, &s, !off)?;
            println!(
                "{image}/{}: {}",
                s.id,
                if off { "unpinned" } else { "pinned" }
            );
        }
        SaveCmd::Delete { image, id, yes } => {
            let s = saves::find(&root, &image, &id)?;
            confirm(
                &format!("This deletes save {image}/{}.", s.id),
                "delete",
                yes,
            )?;
            let freed = saves::delete(&ctx, &s)?;
            println!("{image}/{}: deleted ({} freed)", s.id, avd::human(freed));
        }
        SaveCmd::Prune { image, keep, yes } => {
            let keep = keep.unwrap_or((env.num("ADX_AOD_MAX_SAVES", 3) as usize).saturating_sub(1));
            let all = saves::list(&root, Some(&image))?;
            let gone = saves::prune_plan(&all, keep);
            if gone.is_empty() {
                println!("{image}: nothing to prune (keeping {keep} unpinned)");
                return Ok(());
            }
            let ids: Vec<&str> = gone.iter().map(|s| s.id.as_str()).collect();
            confirm(
                &format!(
                    "This deletes {} save(s) of {image}: {}.",
                    ids.len(),
                    ids.join(", ")
                ),
                "prune",
                yes,
            )?;
            let mut freed = 0;
            for s in gone {
                freed += saves::delete(&ctx, s)?;
            }
            println!(
                "{image}: pruned {} save(s), {} freed",
                ids.len(),
                avd::human(freed)
            );
        }
    }
    Ok(())
}

/// One image by name.
fn one<'a>(images: &'a [avd::Image], name: &str) -> anyhow::Result<&'a avd::Image> {
    images
        .iter()
        .find(|i| i.name == name)
        .ok_or_else(|| anyhow::anyhow!("no image named '{name}' (aodm list)"))
}

/// Destructive steps: the user types `word`, or passes --yes (required without a tty).
fn confirm(what: &str, word: &str, yes: bool) -> anyhow::Result<()> {
    if yes {
        return Ok(());
    }
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("{what} Add --yes to confirm (no terminal to ask).");
    }
    print!("{what} Type '{word}' to continue: ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    if line.trim() != word {
        anyhow::bail!("not confirmed; nothing changed");
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
    confirm(
        &format!("--wipe erases all user data of {names}."),
        "wipe",
        false,
    )
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
