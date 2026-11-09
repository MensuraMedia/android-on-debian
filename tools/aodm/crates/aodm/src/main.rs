//! `aodm`: android-on-debian phone image manager. Command reference: docs/aodm-cli.md.
//! P2: `list`, `card`, `env`. P3: `start`, `stop`, `restart`, `health`. P4: `save`,
//! `clone`, `rename`, `delete`. P5a: CLI conventions (global flags, `--json`, exit
//! codes, `image` group), `show`, `wipe`, `group`, `tag`, and the manager commands
//! `doctor`, `config`, `logs`, `audit`, `disk`, `gc`, `completion`.

mod admin;
mod out;

use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Context;
use aod_core::card::{self, Build};
use aod_core::env::Env;
use aod_core::lifecycle::{self, Ctx, Outcome};
use aod_core::log::Logger;
use aod_core::registry::{self, Registry};
use aod_core::{avd, emu, images, proc, saves};
use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand};
use serde_json::json;

use admin::ConfigCmd;
use out::{Out, Partial, Usage};

const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " (", env!("AODM_GIT"), ")");

#[derive(Parser)]
#[command(
    name = "aodm",
    version = VERSION,
    about = "Manage android-on-debian phone images",
    after_help = "Exit codes: 0 ok, 1 failed, 2 usage, 3 refused, 4 partial, 5 timeout.\nReference: docs/aodm-cli.md"
)]
struct Cli {
    #[command(flatten)]
    g: Global,
    #[command(subcommand)]
    cmd: Cmd,
}

/// Flags every command takes (docs/aodm-cli.md §3).
#[derive(Args)]
struct Global {
    /// Results as one JSON document on stdout (schema 1).
    #[arg(long, global = true)]
    json: bool,
    /// Show what would change, change nothing (commands that change several things).
    #[arg(long, global = true)]
    dry_run: bool,
    /// No progress messages on stderr.
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    quiet: bool,
    /// Also print every log line on stderr.
    #[arg(short, long, global = true)]
    verbose: bool,
    /// Plain output (aodm prints no colour; accepted for scripts that always pass it).
    #[arg(long, global = true)]
    no_color: bool,
    /// Work on DIR/avd, DIR/aod, DIR/saves and log to DIR/log instead of the real homes.
    #[arg(long, global = true, value_name = "DIR")]
    sandbox: Option<PathBuf>,
    /// Skip typed confirmations (required without a terminal).
    #[arg(long, global = true)]
    yes: bool,
}

#[derive(Subcommand)]
enum Cmd {
    #[command(flatten)]
    Img(ImageCmd),
    /// Image commands (the same as without the `image` prefix).
    #[command(subcommand)]
    Image(ImageCmd),
    /// Version card: aodm/adx versions, host specs, toolchain, emulation, images.
    Card,
    /// Show the configuration aodm uses and where each value comes from.
    Env,
    /// Full-copy saves of an image: create, list, show, verify, restore, open, pin, prune.
    #[command(subcommand)]
    Save(SaveCmd),
    /// Groups of images (select them with --group G).
    #[command(subcommand)]
    Group(GroupCmd),
    /// Free-form labels on images (select them with --tag K=V).
    #[command(subcommand)]
    Tag(TagCmd),
    /// Self-check: config, framework, SDK, KVM, registry, locks, saves filesystem, space.
    Doctor,
    /// Read, check and change aodm's settings (config/local.env).
    #[command(subcommand)]
    Config(ConfigCmd),
    /// aodm's lines from the framework log, or an image's emulator log.
    Logs {
        /// Last N lines.
        #[arg(short = 'n', long, default_value_t = 50)]
        lines: usize,
        /// Keep printing new lines (Ctrl-C to stop).
        #[arg(short, long)]
        follow: bool,
        /// The emulator log of this image instead.
        #[arg(long, value_name = "NAME")]
        image: Option<String>,
        /// Only this level and above: info, warn, error.
        #[arg(long)]
        level: Option<String>,
    },
    /// Changes only (save, restore, delete, config set, …): when, what, who.
    Audit {
        /// From this date or date-time on (e.g. 2026-11-07).
        #[arg(long)]
        since: Option<String>,
        #[arg(long, value_name = "NAME")]
        image: Option<String>,
    },
    /// Space per image and its saves, free space per filesystem, leftover work folders.
    Disk,
    /// Remove leftovers of interrupted runs, empty save folders and old rotated logs.
    Gc,
    /// Shell completion script (e.g. aodm completion bash > ~/.local/share/bash-completion/completions/aodm).
    Completion { shell: clap_complete::Shell },
}

#[derive(Subcommand)]
enum ImageCmd {
    /// List phone images with state, API, device, size and groups.
    List {
        /// Only images in this group.
        #[arg(long)]
        group: Option<String>,
        /// Only images with this tag (K=V).
        #[arg(long, value_name = "K=V")]
        tag: Option<String>,
    },
    /// Everything about one image: paths, config, registry entry, saves, last start.
    Show { image: String },
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
    /// Copy a stopped image under a new name (first boot is cold).
    Clone { image: String, new: String },
    /// Rename a stopped image (its saves and registry entry move along).
    Rename { image: String, new: String },
    /// Delete an image (asks you to type its name, or --yes). Its saves are kept.
    Delete { image: String },
    /// Factory-reset a stopped image's user data without booting it (asks first).
    Wipe { image: String },
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
    /// A save's manifest: source, system image, emulator, files with checksums.
    Show { image: String, id: String },
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
    Delete { image: String, id: String },
    /// Delete the oldest unpinned saves, keeping the newest N.
    Prune {
        image: String,
        /// Unpinned saves to keep (default: ADX_AOD_MAX_SAVES - 1, room for one more).
        #[arg(long)]
        keep: Option<usize>,
    },
}

#[derive(Subcommand)]
enum GroupCmd {
    /// Groups with their members.
    List,
    /// Put images in group G (created on first use).
    Add {
        group: String,
        #[arg(required = true)]
        images: Vec<String>,
    },
    /// Take images out of group G.
    Remove {
        group: String,
        #[arg(required = true)]
        images: Vec<String>,
    },
    /// Remove group G from every image (the images stay).
    Delete { group: String },
}

#[derive(Subcommand)]
enum TagCmd {
    /// Set tags on an image: K=V …
    Set {
        image: String,
        #[arg(required = true, value_name = "K=V")]
        tags: Vec<String>,
    },
    /// Remove tags from an image: K …
    Unset {
        image: String,
        #[arg(required = true, value_name = "K")]
        keys: Vec<String>,
    },
}

/// Which images: names, or --all / --group / --tag; --running narrows to running ones.
#[derive(Args)]
struct Targets {
    /// Image names.
    names: Vec<String>,
    /// Every image.
    #[arg(long, conflicts_with_all = ["names", "group", "tag"])]
    all: bool,
    /// Images in this group.
    #[arg(long, conflicts_with = "names")]
    group: Option<String>,
    /// Images with this tag.
    #[arg(long, value_name = "K=V", conflicts_with = "names")]
    tag: Option<String>,
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
}

static QUIET: AtomicBool = AtomicBool::new(false);
static YES: AtomicBool = AtomicBool::new(false);

fn main() {
    let matches = Cli::command().get_matches();
    let path = command_path(&matches);
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    let g = &cli.g;
    QUIET.store(g.quiet, Ordering::Relaxed);
    YES.store(g.yes, Ordering::Relaxed);
    let mut env = Env::load();
    let mut out = Out::new(g.json);
    if let Some(d) = &g.sandbox
        && let Err(e) = sandbox(&mut env, d)
    {
        eprintln!("aodm: error: --sandbox: {e:#}");
        std::process::exit(1);
    }
    let mut log = Logger::new(&env);
    if g.verbose {
        log.echo = Some(|l| eprintln!("aodm: log: {l}"));
    }
    if let Some(e) = &env.config_error {
        say(&format!("warning: {e}"));
        log.warn(e);
    }
    log.info(&format!(
        "run: aodm {path}{}",
        if g.dry_run { " (dry run)" } else { "" }
    ));
    let r = check_dry_run(&cli.cmd, g.dry_run).and_then(|()| {
        run(
            cli.cmd,
            &env,
            &log,
            &mut out,
            g.dry_run,
            g.sandbox.is_some(),
        )
    });
    // Every failure is reported once on stderr and recorded in the framework log.
    let code = match &r {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("aodm: error: {e:#}");
            log.error(&format!("{e:#}"));
            out::code(e)
        }
    };
    out.finish(&path, r.as_ref().err());
    std::process::exit(code);
}

/// `save restore`, as words (for logs and the JSON `command` field).
fn command_path(m: &clap::ArgMatches) -> String {
    let mut words = Vec::new();
    let mut cur = m;
    while let Some((name, sub)) = cur.subcommand() {
        words.push(name.to_string());
        cur = sub;
    }
    words.join(" ")
}

/// `--sandbox DIR`: every path aodm writes goes under DIR.
fn sandbox(env: &mut Env, dir: &std::path::Path) -> anyhow::Result<()> {
    let dir = std::path::absolute(dir)?;
    for (key, sub) in [
        ("ANDROID_AVD_HOME", "avd"),
        ("ADX_AOD_HOME", "aod"),
        ("ADX_AOD_SAVES_DIR", "saves"),
        ("ADX_LOG_DIR", "log"),
        // path.rel= in .ini files resolves against the Android user home.
        ("ANDROID_USER_HOME", "android"),
    ] {
        let p = dir.join(sub);
        std::fs::create_dir_all(&p).with_context(|| p.display().to_string())?;
        env.set(key, p.to_string_lossy());
    }
    env.set("ANDROID_EMULATOR_HOME", "");
    // Marks the run as sandboxed: `inventory` drops images whose folder lies outside.
    env.set("ADX_AOD_SANDBOX", dir.canonicalize()?.to_string_lossy());
    Ok(())
}

/// A global --dry-run on a command that can't honour it must not run the real thing.
fn check_dry_run(cmd: &Cmd, dry: bool) -> anyhow::Result<()> {
    let ok = match cmd {
        Cmd::Img(c) | Cmd::Image(c) => {
            matches!(c, ImageCmd::Delete { .. } | ImageCmd::Wipe { .. })
        }
        Cmd::Save(c) => matches!(
            c,
            SaveCmd::Restore { .. } | SaveCmd::Delete { .. } | SaveCmd::Prune { .. }
        ),
        Cmd::Config(c) => matches!(c, ConfigCmd::Set { .. } | ConfigCmd::Unset { .. }),
        Cmd::Gc => true,
        _ => false,
    };
    if dry && !ok {
        return Err(Usage(
            "--dry-run is not supported by this command (it would run for real)".into(),
        )
        .into());
    }
    Ok(())
}

fn run(
    cmd: Cmd,
    env: &Env,
    log: &Logger,
    out: &mut Out,
    dry: bool,
    in_sandbox: bool,
) -> anyhow::Result<()> {
    let ctx = Ctx::new(env, log, say);
    match cmd {
        Cmd::Img(c) | Cmd::Image(c) => image(c, &ctx, out, dry)?,
        Cmd::Card => {
            let (images, procs, reg) = inventory(env, log)?;
            log.info("card");
            let c = card::build(env, &build(), &images, &procs, &reg);
            out.table(&card::render_text(&c), [json!(c)]);
            problems(&images);
        }
        Cmd::Env => {
            log.info("env");
            let entries = env.entries();
            let width = entries.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
            for (key, v) in entries {
                out.line(
                    format!("{key:<width$}  {:<8}  {}", v.source.as_str(), v.value),
                    json!({"key": key, "value": v.value, "source": v.source.as_str()}),
                );
            }
            for w in &env.ignored {
                say(&format!("note: {w}"));
            }
        }
        Cmd::Save(c) => save(c, &ctx, out, dry)?,
        Cmd::Group(c) => group(c, &ctx, out)?,
        Cmd::Tag(c) => tag(c, &ctx, out)?,
        Cmd::Doctor => {
            let (images, _, _) = inventory(env, log)?;
            admin::doctor(&ctx, &images, env!("AODM_GIT"), out)?;
        }
        Cmd::Config(c) => {
            admin::config(c, &ctx, in_sandbox, dry, YES.load(Ordering::Relaxed), out)?
        }
        Cmd::Logs {
            lines,
            follow,
            image,
            level,
        } => admin::logs(env, lines, follow, image.as_deref(), level.as_deref(), out)?,
        Cmd::Audit { since, image } => admin::audit(env, since.as_deref(), image.as_deref(), out)?,
        Cmd::Disk => {
            let (images, _, _) = inventory(env, log)?;
            admin::disk(&ctx, &images, out)?;
        }
        Cmd::Gc => admin::gc(&ctx, dry, out)?,
        Cmd::Completion { shell } => {
            if out.json {
                return Err(
                    Usage("completion prints a shell script; not with --json".into()).into(),
                );
            }
            let mut buf = Vec::new();
            clap_complete::generate(shell, &mut Cli::command(), "aodm", &mut buf);
            out::emit(&String::from_utf8_lossy(&buf));
        }
    }
    Ok(())
}

fn image(c: ImageCmd, ctx: &Ctx, out: &mut Out, dry: bool) -> anyhow::Result<()> {
    let (env, log) = (ctx.env, ctx.log);
    match c {
        ImageCmd::List { group, tag } => {
            let (mut images, procs, reg) = inventory(env, log)?;
            let tag = tag.as_deref().map(parse_tag).transpose()?;
            if let Some(g) = &group {
                images.retain(|i| reg.groups(&i.name).iter().any(|x| x == g));
            }
            if let Some((k, v)) = &tag {
                images.retain(|i| reg.tag(&i.name, k) == Some(v.as_str()));
            }
            let c = card::build(env, &build(), &images, &procs, &reg);
            log.info(&format!("list: {} image(s)", c.images.len()));
            if c.images.is_empty() && (group.is_some() || tag.is_some()) {
                out.text("no matching images");
                return Ok(());
            }
            out.table(
                &card::render_images(&c.images),
                c.images.iter().map(|r| json!(r)),
            );
            problems(&images);
        }
        ImageCmd::Show { image } => {
            let (images, procs, reg) = inventory(env, log)?;
            show(env, one(&images, &image)?, &procs, &reg, out);
        }
        ImageCmd::Start { targets, opts } => {
            let (images, procs, reg) = inventory(env, log)?;
            let sel = select(&targets, &images, &procs, &reg)?;
            // Running images are refused with --wipe, so only the others are asked about.
            let stopped: Vec<_> = sel
                .iter()
                .copied()
                .filter(|i| emu::running(&procs, i).is_none())
                .collect();
            confirm_wipe(&stopped, &opts)?;
            report(&start(ctx, &sel, &images, &reg, &opts)?, out)?;
        }
        ImageCmd::Stop { targets } => {
            let (images, procs, reg) = inventory(env, log)?;
            let sel = select(&targets, &images, &procs, &reg)?;
            let mut res = Vec::new();
            let mut running = Vec::new();
            for i in &sel {
                match emu::running(&procs, i) {
                    Some(p) => running.push(p),
                    None => res.push(Outcome {
                        image: i.name.clone(),
                        result: Ok(not_running(i, &procs)),
                    }),
                }
            }
            res.extend(lifecycle::stop(ctx, &running));
            report(&res, out)?;
        }
        ImageCmd::Restart { targets, opts } => {
            let (images, procs, reg) = inventory(env, log)?;
            let sel = select(&targets, &images, &procs, &reg)?;
            confirm_wipe(&sel, &opts)?;
            let running: Vec<_> = sel.iter().filter_map(|i| emu::running(&procs, i)).collect();
            let stopped = lifecycle::stop(ctx, &running);
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
            let mut res: Vec<Outcome> = stopped.into_iter().filter(|o| o.result.is_err()).collect();
            if !again.is_empty() {
                res.extend(start(ctx, &again, &images, &reg, &opts)?);
            }
            report(&res, out)?;
        }
        ImageCmd::Health { targets } => {
            let (images, procs, reg) = inventory(env, log)?;
            let any = !targets.names.is_empty()
                || targets.all
                || targets.group.is_some()
                || targets.tag.is_some();
            let sel = if any {
                select(&targets, &images, &procs, &reg)?
            } else {
                // No selector: every image (or, with --running alone, every running one).
                images
                    .iter()
                    .filter(|i| !targets.running || emu::running(&procs, i).is_some())
                    .collect()
            };
            if sel.is_empty() {
                out.text(format!(
                    "no {}images",
                    if targets.running { "running " } else { "" }
                ));
                return Ok(());
            }
            let rows = lifecycle::health(ctx, &sel, &procs);
            let bad = rows.iter().filter(|h| !h.ok).count();
            log.info(&format!(
                "health: {} image(s), {bad} need attention",
                rows.len()
            ));
            out.table(&health_table(&rows), rows.iter().map(|h| json!(h)));
            if bad > 0 {
                anyhow::bail!("{bad} image(s) need attention");
            }
        }
        ImageCmd::Clone { image, new } => {
            images::sweep_all(ctx);
            let (images, _, _) = inventory(env, log)?;
            let msg = images::clone(ctx, one(&images, &image)?, &new)?;
            out.line(
                format!("{new}: {msg}"),
                json!({"image": new, "ok": true, "message": msg}),
            );
        }
        ImageCmd::Rename { image, new } => {
            images::sweep_all(ctx);
            let (images, _, _) = inventory(env, log)?;
            let msg = images::rename(ctx, one(&images, &image)?, &new)?;
            out.line(
                format!("{new}: {msg}"),
                json!({"image": new, "ok": true, "message": msg}),
            );
        }
        ImageCmd::Delete { image } => {
            if !dry {
                images::sweep_all(ctx);
            }
            let (images, _, _) = inventory(env, log)?;
            let img = one(&images, &image)?;
            if dry {
                out.line(
                    format!(
                        "would delete {image}: {} ({})",
                        img.dir.display(),
                        avd::human(img.disk_bytes())
                    ),
                    json!({"image": image, "path": img.dir, "bytes": img.disk_bytes(), "changed": false}),
                );
                return Ok(());
            }
            confirm(
                &format!(
                    "This deletes image {image} and all its user data ({}, {}). Its saves are kept.",
                    img.dir.display(),
                    avd::human(img.disk_bytes())
                ),
                &image,
            )?;
            let msg = images::delete(ctx, img, &images)?;
            out.line(
                format!("{image}: {msg}"),
                json!({"image": image, "ok": true, "message": msg}),
            );
        }
        ImageCmd::Wipe { image } => {
            if !dry {
                images::sweep_all(ctx);
            }
            let (images, _, _) = inventory(env, log)?;
            let img = one(&images, &image)?;
            let plan = images::wipe_plan(img);
            if dry {
                for (p, n) in &plan {
                    out.line(
                        format!("would remove {} ({})", p.display(), avd::human(*n)),
                        json!({"image": image, "path": p, "bytes": n, "changed": false}),
                    );
                }
                return Ok(());
            }
            let size: u64 = plan.iter().map(|(_, n)| n).sum();
            confirm(
                &format!(
                    "This erases all user data of {image} (apps, accounts, files, snapshots; {}).",
                    avd::human(size)
                ),
                &image,
            )?;
            let msg = images::wipe(ctx, img)?;
            out.line(
                format!("{image}: {msg}"),
                json!({"image": image, "ok": true, "message": msg}),
            );
        }
    }
    Ok(())
}

/// `aodm show NAME`.
fn show(env: &Env, img: &avd::Image, procs: &[proc::EmuProc], reg: &Registry, out: &mut Out) {
    let running = emu::running(procs, img);
    let entry = reg.entry(&img.name).cloned().unwrap_or_default();
    let all_saves = images::cfg_dir(env, "ADX_AOD_SAVES_DIR")
        .ok()
        .and_then(|r| saves::list(&r, Some(&img.name)).ok())
        .unwrap_or_default();
    let emu_log = emu::log_path(&env.path("ADX_LOG_DIR"), &img.name);
    let last_start = std::fs::metadata(&emu_log)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| jiff::Timestamp::try_from(t).ok())
        .map(|t| {
            t.to_zoned(jiff::tz::TimeZone::system())
                .strftime("%Y-%m-%d %H:%M:%S")
                .to_string()
        });
    let state = match running {
        Some(p) => format!(
            "running (pid {}, {})",
            p.pid,
            p.serial().unwrap_or_else(|| "no serial".into())
        ),
        None => "stopped".into(),
    };
    let rev = img
        .sysimg_revision(&env.path("ANDROID_HOME"))
        .unwrap_or_else(|| "?".into());
    let tags: Vec<String> = entry.tags.iter().map(|(k, v)| format!("{k}={v}")).collect();
    let save_ids: Vec<String> = all_saves
        .iter()
        .map(|s| format!("{}{}", s.id, if s.pinned { " (pinned)" } else { "" }))
        .collect();
    let size = img.disk_bytes();
    let rows: Vec<(&str, String)> = vec![
        ("name", img.name.clone()),
        ("state", state.clone()),
        (
            "problem",
            img.problem.clone().unwrap_or_else(|| "none".into()),
        ),
        ("folder", img.dir.display().to_string()),
        ("ini", img.ini_path.display().to_string()),
        ("size", avd::human(size)),
        ("api", img.api().into()),
        ("abi", img.abi().into()),
        ("tag", img.tag().into()),
        (
            "system image",
            format!("{} rev {rev}", img.sysdir().unwrap_or("?")),
        ),
        ("device", img.device().into()),
        ("ram", img.ram().into()),
        ("groups", or_none(&entry.groups.join(", "))),
        ("tags", or_none(&tags.join(", "))),
        (
            "pinned port",
            entry
                .port
                .map(|p| p.to_string())
                .unwrap_or_else(|| "none".into()),
        ),
        (
            "template",
            entry.template.clone().unwrap_or_else(|| "none".into()),
        ),
        (
            "origin",
            entry.origin.clone().unwrap_or_else(|| "none".into()),
        ),
        ("saves", or_none(&save_ids.join(", "))),
        ("emulator log", emu_log.display().to_string()),
        (
            "last start",
            last_start
                .clone()
                .unwrap_or_else(|| "never (no emulator log)".into()),
        ),
    ];
    let config: serde_json::Map<String, serde_json::Value> = img
        .config
        .as_ref()
        .map(|c| {
            c.entries()
                .map(|(k, v)| (k.to_string(), json!(v)))
                .collect()
        })
        .unwrap_or_default();
    let w = rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    let mut text: String = rows
        .iter()
        .map(|(k, v)| format!("{k:<w$}  {v}\n"))
        .collect();
    text.push_str(&format!(
        "{:<w$}  {} keys (aodm show {} --json)\n",
        "config.ini",
        config.len(),
        img.name
    ));
    let saves_json: Vec<_> = all_saves
        .iter()
        .map(|s| json!({"id": s.id, "pinned": s.pinned}))
        .collect();
    out.table(
        &text,
        [json!({
            "image": img.name, "ok": true, "state": state, "running": running.is_some(),
            "pid": running.map(|p| p.pid), "serial": running.and_then(|p| p.serial()),
            "problem": img.problem, "folder": img.dir, "ini": img.ini_path,
            "bytes": size, "api": img.api(), "abi": img.abi(), "tag": img.tag(),
            "sysdir": img.sysdir(), "sysimg_revision": rev, "device": img.device(),
            "ram": img.ram(), "groups": entry.groups, "tags": entry.tags,
            "port": entry.port, "template": entry.template, "origin": entry.origin,
            "saves": saves_json, "emulator_log": emu_log, "last_start": last_start,
            "config": config,
        })],
    );
}

fn or_none(s: &str) -> String {
    if s.is_empty() {
        "none".into()
    } else {
        s.into()
    }
}

fn save(c: SaveCmd, ctx: &Ctx, out: &mut Out, dry: bool) -> anyhow::Result<()> {
    let (env, log) = (ctx.env, ctx.log);
    let root = images::cfg_dir(env, "ADX_AOD_SAVES_DIR")?;
    // A dry run changes nothing, not even leftovers of crashed runs.
    if !dry {
        images::sweep_all(ctx);
    }
    match c {
        SaveCmd::Create { image, label } => {
            let (images, _, _) = inventory(env, log)?;
            let s = saves::create(ctx, one(&images, &image)?, label.as_deref())?;
            let size = avd::du(&s.path);
            out.line(
                format!("{image}: saved as {} ({})", s.id, avd::human(size)),
                json!({"image": image, "ok": true, "id": s.id, "bytes": size, "path": s.path}),
            );
        }
        SaveCmd::List { image } => {
            let all = saves::list(&root, image.as_deref())?;
            log.info(&format!("save list: {} save(s)", all.len()));
            if all.is_empty() {
                out.text(format!("no saves in {}", root.display()));
                return Ok(());
            }
            out.text(format!(
                "{:<16}  {:<32}  {:>7}  {:<6}  NOTE",
                "IMAGE", "SAVE", "SIZE", "PINNED"
            ));
            for s in &all {
                let size = avd::du(&s.path);
                let note = match &s.manifest {
                    Ok(m) => format!(
                        "system image rev {}, emulator {}",
                        m.sysimg_revision, m.emulator
                    ),
                    Err(e) => format!("DAMAGED: {e}"),
                };
                out.line(
                    format!(
                        "{:<16}  {:<32}  {:>7}  {:<6}  {note}",
                        s.image,
                        s.id,
                        avd::human(size),
                        if s.pinned { "yes" } else { "" }
                    ),
                    json!({
                        "image": s.image, "id": s.id, "bytes": size, "pinned": s.pinned,
                        "damaged": s.manifest.as_ref().err(),
                        "sysimg_revision": s.manifest.as_ref().ok().map(|m| &m.sysimg_revision),
                        "emulator": s.manifest.as_ref().ok().map(|m| &m.emulator),
                    }),
                );
            }
        }
        SaveCmd::Show { image, id } => {
            let s = saves::find(&root, &image, &id)?;
            let m = s
                .manifest
                .as_ref()
                .map_err(|e| anyhow::anyhow!("save {image}/{} is damaged: {e}", s.id))?;
            let mut text = format!(
                "save        {image}/{}\npath        {}\npinned      {}\ncreated     {}\nlabel       {}\nsource      {}\ntarget      {} ({}, {})\nsystem img  rev {}\nemulator    {}\naodm        {}\nfiles       {}\n",
                s.id,
                s.path.display(),
                if s.pinned { "yes" } else { "no" },
                m.created,
                or_none(&m.label),
                m.source.display(),
                m.target,
                m.tag,
                m.abi,
                m.sysimg_revision,
                m.emulator,
                m.aodm,
                m.files.len()
            );
            for f in &m.files {
                text.push_str(&format!(
                    "  {:>8}  {}  {}\n",
                    avd::human(f.size),
                    f.sha256.get(..12).unwrap_or(&f.sha256),
                    f.path
                ));
            }
            out.table(
                &text,
                [json!({"image": image, "id": s.id, "pinned": s.pinned, "manifest": m})],
            );
        }
        SaveCmd::Verify { image, id } => {
            let s = saves::find(&root, &image, &id)?;
            say(&format!("verifying {image}/{}…", s.id));
            let problems = saves::verify(ctx, &s)?;
            if !problems.is_empty() {
                for p in &problems {
                    out.line(
                        format!("{image}/{}: {p}", s.id),
                        json!({"image": image, "id": s.id, "problem": p}),
                    );
                }
                anyhow::bail!("save {image}/{} is damaged", s.id);
            }
            out.line(
                format!("{image}/{}: intact", s.id),
                json!({"image": image, "id": s.id, "ok": true}),
            );
        }
        SaveCmd::Restore {
            image,
            id,
            backup_first,
        } => {
            let (images, _, _) = inventory(env, log)?;
            let img = one(&images, &image)?;
            let s = saves::find(&root, &image, &id)?;
            if dry {
                out.line(
                    format!(
                        "would restore {image} from save {}{}",
                        s.id,
                        if backup_first {
                            " (saving the current state first)"
                        } else {
                            ""
                        }
                    ),
                    json!({"image": image, "id": s.id, "backup_first": backup_first, "changed": false}),
                );
                return Ok(());
            }
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
            )?;
            if backup_first {
                let b = saves::create(ctx, img, Some("before-restore"))?;
                say(&format!("current state saved as {}", b.id));
            }
            let msg = saves::restore(ctx, &s, img)?;
            out.line(
                format!("{image}: {msg}"),
                json!({"image": image, "ok": true, "id": s.id, "message": msg}),
            );
        }
        SaveCmd::Open { image, id, new } => {
            let s = saves::find(&root, &image, &id)?;
            let msg = saves::open(ctx, &s, &new)?;
            out.line(
                format!("{new}: {msg}"),
                json!({"image": new, "ok": true, "message": msg}),
            );
        }
        SaveCmd::Pin { image, id, off } => {
            let s = saves::find(&root, &image, &id)?;
            saves::pin(ctx, &s, !off)?;
            out.line(
                format!(
                    "{image}/{}: {}",
                    s.id,
                    if off { "unpinned" } else { "pinned" }
                ),
                json!({"image": image, "id": s.id, "pinned": !off}),
            );
        }
        SaveCmd::Delete { image, id } => {
            let s = saves::find(&root, &image, &id)?;
            if dry {
                out.line(
                    format!("would delete save {image}/{}", s.id),
                    json!({"image": image, "id": s.id, "changed": false}),
                );
                return Ok(());
            }
            confirm(&format!("This deletes save {image}/{}.", s.id), "delete")?;
            let freed = saves::delete(ctx, &s)?;
            out.line(
                format!("{image}/{}: deleted ({} freed)", s.id, avd::human(freed)),
                json!({"image": image, "id": s.id, "bytes": freed, "changed": true}),
            );
        }
        SaveCmd::Prune { image, keep } => {
            let keep = keep.unwrap_or((env.num("ADX_AOD_MAX_SAVES", 3) as usize).saturating_sub(1));
            let all = saves::list(&root, Some(&image))?;
            let gone = saves::prune_plan(&all, keep);
            if gone.is_empty() {
                out.text(format!(
                    "{image}: nothing to prune (keeping {keep} unpinned)"
                ));
                return Ok(());
            }
            let ids: Vec<&str> = gone.iter().map(|s| s.id.as_str()).collect();
            if dry {
                for id in &ids {
                    out.line(
                        format!("would delete save {image}/{id}"),
                        json!({"image": image, "id": id, "changed": false}),
                    );
                }
                return Ok(());
            }
            confirm(
                &format!(
                    "This deletes {} save(s) of {image}: {}.",
                    ids.len(),
                    ids.join(", ")
                ),
                "prune",
            )?;
            let mut freed = 0;
            for s in gone {
                let n = saves::delete(ctx, s)?;
                freed += n;
                out.line(
                    format!("{image}/{}: deleted ({} freed)", s.id, avd::human(n)),
                    json!({"image": image, "id": s.id, "bytes": n, "changed": true}),
                );
            }
            out.text(format!(
                "{image}: pruned {} save(s), {} freed",
                ids.len(),
                avd::human(freed)
            ));
        }
    }
    Ok(())
}

fn group(c: GroupCmd, ctx: &Ctx, out: &mut Out) -> anyhow::Result<()> {
    let home = images::cfg_dir(ctx.env, "ADX_AOD_HOME")?;
    match c {
        GroupCmd::List => {
            let reg = Registry::load(&home)?;
            let groups = reg.all_groups();
            if groups.is_empty() {
                out.text("no groups (aodm group add G NAME…)");
            }
            for (g, members) in groups {
                out.line(
                    format!("{g:<16}  {:>3}  {}", members.len(), members.join(", ")),
                    json!({"group": g, "images": members}),
                );
            }
        }
        GroupCmd::Add {
            group,
            images: names,
        } => {
            registry::validate_label("group name", &group)?;
            let (all, _, _) = inventory(ctx.env, ctx.log)?;
            for n in &names {
                one(&all, n)?;
            }
            Registry::update(&home, |r| {
                for n in &names {
                    let e = r.images.entry(n.clone()).or_default();
                    if !e.groups.contains(&group) {
                        e.groups.push(group.clone());
                    }
                }
            })?;
            for n in &names {
                ctx.log.audit("group-add", n, &group);
                out.line(
                    format!("{n}: in group {group}"),
                    json!({"image": n, "group": group, "ok": true}),
                );
            }
        }
        GroupCmd::Remove {
            group,
            images: names,
        } => {
            Registry::update(&home, |r| {
                for n in &names {
                    if let Some(e) = r.images.get_mut(n) {
                        e.groups.retain(|g| g != &group);
                    }
                }
                r.compact();
            })?;
            for n in &names {
                ctx.log.audit("group-remove", n, &group);
                out.line(
                    format!("{n}: not in group {group}"),
                    json!({"image": n, "group": group, "ok": true}),
                );
            }
        }
        GroupCmd::Delete { group } => {
            let mut members = Vec::new();
            Registry::update(&home, |r| {
                for (n, e) in r.images.iter_mut() {
                    if e.groups.contains(&group) {
                        e.groups.retain(|g| g != &group);
                        members.push(n.clone());
                    }
                }
                r.compact();
            })?;
            ctx.log.audit(
                "group-delete",
                "-",
                &format!("{group} ({} image(s))", members.len()),
            );
            out.line(
                format!("group {group} removed from {} image(s)", members.len()),
                json!({"group": group, "images": members}),
            );
        }
    }
    Ok(())
}

fn parse_tag(s: &str) -> anyhow::Result<(String, String)> {
    let (k, v) = s
        .split_once('=')
        .ok_or_else(|| Usage(format!("{s:?}: tags are K=V")))?;
    // A malformed argument is a usage error (exit 2), like a missing one.
    registry::validate_label("tag key", k)
        .and_then(|()| registry::validate_value(v))
        .map_err(|e| Usage(e.to_string()))?;
    Ok((k.to_string(), v.to_string()))
}

fn tag(c: TagCmd, ctx: &Ctx, out: &mut Out) -> anyhow::Result<()> {
    let home = images::cfg_dir(ctx.env, "ADX_AOD_HOME")?;
    match c {
        TagCmd::Set { image, tags } => {
            let (all, _, _) = inventory(ctx.env, ctx.log)?;
            one(&all, &image)?;
            let kv = tags
                .iter()
                .map(|t| parse_tag(t))
                .collect::<anyhow::Result<Vec<_>>>()?;
            Registry::update(&home, |r| {
                let e = r.images.entry(image.clone()).or_default();
                for (k, v) in &kv {
                    e.tags.insert(k.clone(), v.clone());
                }
            })?;
            for (k, v) in &kv {
                ctx.log.audit("tag-set", &image, &format!("{k}={v}"));
                out.line(
                    format!("{image}: {k}={v}"),
                    json!({"image": image, "key": k, "value": v}),
                );
            }
        }
        TagCmd::Unset { image, keys } => {
            Registry::update(&home, |r| {
                if let Some(e) = r.images.get_mut(&image) {
                    for k in &keys {
                        e.tags.remove(k);
                    }
                }
                r.compact();
            })?;
            for k in &keys {
                ctx.log.audit("tag-unset", &image, k);
                out.line(
                    format!("{image}: {k} removed"),
                    json!({"image": image, "key": k}),
                );
            }
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
/// Declining is a refusal (exit 3).
fn confirm(what: &str, word: &str) -> anyhow::Result<()> {
    let refused = |m: String| -> anyhow::Error { aod_core::Error::Refused(m).into() };
    if YES.load(Ordering::Relaxed) {
        return Ok(());
    }
    if !std::io::stdin().is_terminal() {
        return Err(refused(format!(
            "{what} Add --yes to confirm (no terminal to ask)."
        )));
    }
    eprint!("{what} Type '{word}' to continue: ");
    std::io::stderr().flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    if line.trim() != word {
        return Err(refused("not confirmed; nothing changed".into()));
    }
    Ok(())
}

pub(crate) fn say(msg: &str) {
    if !QUIET.load(Ordering::Relaxed) {
        eprintln!("aodm: {msg}");
    }
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
        say(&format!("warning: {w}"));
    }
    Ok(out)
}

/// One line per image; fails if any image failed: exit 4 when others succeeded, else
/// the shared code of the failures (all refused 3, all timed out 5) or 1.
fn report(res: &[Outcome], out: &mut Out) -> anyhow::Result<()> {
    let mut codes = Vec::new();
    for o in res {
        match &o.result {
            Ok(m) => out.line(
                format!("{}: {m}", o.image),
                json!({"image": o.image, "ok": true, "message": m, "code": 0}),
            ),
            Err(e) => {
                let c = out::core_code(e);
                codes.push(c);
                eprintln!("{}: FAILED: {e}", o.image);
                out.push(
                    json!({"image": o.image, "ok": false, "message": e.to_string(), "code": c}),
                );
            }
        }
    }
    let failed = codes.len();
    let msg = format!("{failed} of {} image(s) failed", res.len());
    if failed == 0 {
        return Ok(());
    }
    if failed < res.len() {
        return Err(Partial(msg).into());
    }
    let c = if codes.iter().all(|c| *c == codes[0]) {
        codes[0]
    } else {
        1
    };
    Err(out::Coded(c, msg).into())
}

/// --wipe destroys user data: confirm by typing `wipe`, or pass --yes.
fn confirm_wipe(sel: &[&avd::Image], opts: &StartOpts) -> anyhow::Result<()> {
    if !opts.wipe || sel.is_empty() {
        return Ok(());
    }
    let names = sel
        .iter()
        .map(|i| i.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    confirm(&format!("--wipe erases all user data of {names}."), "wipe")
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
    let tag = t.tag.as_deref().map(parse_tag).transpose()?;
    let by_set = t.group.is_some() || tag.is_some();
    let any = t.all || t.running;
    let in_set = |i: &avd::Image| {
        t.group
            .as_ref()
            .is_none_or(|g| reg.groups(&i.name).contains(g))
            && tag
                .as_ref()
                .is_none_or(|(k, v)| reg.tag(&i.name, k) == Some(v.as_str()))
    };
    let sel: Vec<_> = images
        .iter()
        .filter(|i| {
            t.names.contains(&i.name)
                || (by_set && in_set(i))
                || (any && t.names.is_empty() && !by_set)
        })
        .filter(|i| !t.running || emu::running(procs, i).is_some())
        .collect();
    if sel.is_empty() {
        if t.names.is_empty() && !any && !by_set {
            return Err(Usage(
                "which images? give names, --all, --group G, --tag K=V or --running".into(),
            )
            .into());
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
            say(&format!("{} is broken: {p}", i.name));
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
    let mut images = avd::scan(&env.path("ANDROID_AVD_HOME"), &env.android_user_home())
        .context("scanning phone images")?;
    // --sandbox: a sandbox .ini whose path= points at a real image must not let any
    // command reach that folder. Images resolving outside the sandbox are not listed.
    let sb = env.get("ADX_AOD_SANDBOX");
    if !sb.is_empty() {
        images.retain(|i| {
            let real = std::fs::canonicalize(&i.dir).unwrap_or_else(|_| i.dir.clone());
            let inside = real.starts_with(sb);
            if !inside {
                say(&format!(
                    "warning: --sandbox: {} ignored, its folder {} is outside the sandbox",
                    i.name,
                    i.dir.display()
                ));
                log.warn(&format!(
                    "sandbox: ignored {} ({})",
                    i.name,
                    i.dir.display()
                ));
            }
            inside
        });
    }
    // A broken registry must not hide the images: warn and continue without it.
    let reg = Registry::load(&env.path("ADX_AOD_HOME")).unwrap_or_else(|e| {
        say(&format!("warning: registry ignored: {e}"));
        log.warn(&format!("registry ignored: {e}"));
        Registry::default()
    });
    Ok((images, proc::emulators(), reg))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every command path (`save restore`), from the clap definition.
    fn paths(c: &clap::Command, prefix: &str, out: &mut Vec<String>) {
        for s in c.get_subcommands() {
            let p = format!("{prefix}{}", s.get_name());
            out.push(p.clone());
            paths(s, &format!("{p} "), out);
        }
    }

    /// Command rows of docs/aodm-cli.md: (command words, done?). A row is done when its
    /// last cell says `done`, or the table has no status column and its section heading
    /// ends in `— done`. Spans after the first without `aodm ` are relative to it
    /// (`aodm group add …` / `group remove …`).
    fn documented() -> Vec<(String, bool)> {
        let doc = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../docs/aodm-cli.md"),
        )
        .unwrap();
        let mut section_done = false;
        let mut status_col = false;
        let mut out = Vec::new();
        for l in doc.lines() {
            if l.starts_with('#') {
                section_done = l.trim_end().ends_with("— done");
                continue;
            }
            if !l.starts_with('|') {
                status_col = false;
                continue;
            }
            let cells: Vec<&str> = l.trim_matches('|').split('|').map(str::trim).collect();
            if cells.first() == Some(&"Command") {
                status_col = cells.last() == Some(&"Status");
                continue;
            }
            let done = if status_col {
                cells.last() == Some(&"done")
            } else {
                section_done
            };
            let first = cells[0];
            if !first.starts_with("`aodm ") {
                continue;
            }
            for span in first.split('`').skip(1).step_by(2) {
                let words: Vec<&str> = span
                    .trim_start_matches("aodm ")
                    .split_whitespace()
                    .take_while(|w| {
                        w.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                            && w.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                    })
                    .collect();
                if !words.is_empty() {
                    out.push((words.join(" "), done));
                }
            }
        }
        out
    }

    #[test]
    fn definition_is_valid_and_every_help_renders() {
        Cli::command().debug_assert();
        let mut all = Vec::new();
        paths(&Cli::command(), "", &mut all);
        for p in all {
            let mut args = vec!["aodm"];
            args.extend(p.split(' '));
            args.push("--help");
            let e = Cli::try_parse_from(&args).err().expect("--help exits");
            assert_eq!(e.kind(), clap::error::ErrorKind::DisplayHelp, "{p}");
        }
    }

    /// The docs/CLI gate (aodm-cli.md §10): every command is documented, and every
    /// command documented as done exists.
    #[test]
    fn docs_match_commands() {
        let mut real = Vec::new();
        paths(&Cli::command(), "", &mut real);
        let docs = documented();
        // The parser must see both kinds of rows, or the gate would pass vacuously.
        assert!(docs.contains(&("save show".into(), true)), "{docs:?}");
        assert!(docs.contains(&("config set".into(), true)), "{docs:?}");
        assert!(docs.contains(&("settings plan".into(), false)), "{docs:?}");
        for p in real
            .iter()
            .filter(|p| !p.starts_with("image ") && !p.ends_with("help"))
        {
            assert!(
                docs.iter()
                    .any(|(d, _)| d == p || d.starts_with(&format!("{p} "))),
                "`aodm {p}` is not in docs/aodm-cli.md"
            );
        }
        // A built command still documented only as planned is drift too.
        for p in &real {
            let rows: Vec<bool> = docs
                .iter()
                .filter(|(d, _)| d == p)
                .map(|(_, x)| *x)
                .collect();
            assert!(
                rows.is_empty() || rows.contains(&true),
                "`aodm {p}` exists but docs/aodm-cli.md marks it planned"
            );
        }
        for (d, done) in &docs {
            if *done && d != "help" {
                assert!(
                    real.contains(d),
                    "docs/aodm-cli.md says `aodm {d}` is done, but it doesn't exist"
                );
            }
        }
    }
}
