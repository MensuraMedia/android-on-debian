//! `aodm`: android-on-debian phone image manager.
//! P2: `list`, `card`, `env`. Lifecycle, saves, settings, templates, identity,
//! spin-offs and the TUI arrive in later phases (see ROADMAP.md).

use anyhow::Context;
use aod_core::card::{self, Build};
use aod_core::env::Env;
use aod_core::log::Logger;
use aod_core::registry::Registry;
use aod_core::{avd, proc};
use clap::{Parser, Subcommand};

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
    }
    Ok(())
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
