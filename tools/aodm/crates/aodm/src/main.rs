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

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let env = Env::load();
    let log = Logger::new(&env);
    if let Some(e) = &env.config_error {
        eprintln!("aodm: warning: {e}");
        log.warn(e);
    }
    match cli.cmd {
        Cmd::List { group } => {
            let (images, procs, reg) = inventory(&env)?;
            let mut c = card::build(&env, &build(), &images, &procs, &reg);
            if let Some(g) = &group {
                c.images.retain(|r| r.groups.split(',').any(|x| x == g));
            }
            log.info(&format!(
                "list: {} image(s){}",
                c.images.len(),
                group.map(|g| format!(" in group {g}")).unwrap_or_default()
            ));
            print!("{}", card::render_images(&c.images));
        }
        Cmd::Card => {
            let (images, procs, reg) = inventory(&env)?;
            log.info("card");
            print!(
                "{}",
                card::render_text(&card::build(&env, &build(), &images, &procs, &reg))
            );
        }
        Cmd::Env => {
            log.info("env");
            let entries = env.entries();
            let width = entries.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
            for (key, v) in entries {
                println!("{key:<width$}  {:<8}  {}", v.source.as_str(), v.value);
            }
        }
    }
    Ok(())
}

fn build() -> Build<'static> {
    Build {
        aodm: VERSION,
        rustc: env!("AODM_RUSTC"),
    }
}

fn inventory(env: &Env) -> anyhow::Result<(Vec<avd::Image>, Vec<proc::EmuProc>, Registry)> {
    let images = avd::scan(&env.path("ANDROID_AVD_HOME")).context("scanning phone images")?;
    let reg = Registry::load(&env.path("ADX_AOD_HOME")).context("reading the image registry")?;
    Ok((images, proc::emulators(), reg))
}
