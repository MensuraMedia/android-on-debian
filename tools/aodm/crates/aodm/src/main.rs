//! `aodm`: android-on-debian phone image manager.
//! P1 ships `env` and `--version`; inventory, lifecycle, saves, settings, templates,
//! identity, spin-offs and the TUI arrive in later phases (see ROADMAP.md).

use aod_core::env::Env;
use aod_core::log::Logger;
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
