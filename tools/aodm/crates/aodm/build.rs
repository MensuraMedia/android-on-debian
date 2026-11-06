//! Bakes the git revision (shown by `aodm --version` and the version card) and,
//! when built by adx, the framework root (lets a standalone binary find its config).
use std::process::Command;

fn main() {
    let rev = Command::new("git")
        .args(["describe", "--always", "--dirty", "--abbrev=7"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=AODM_GIT={rev}");
    if let Ok(root) = std::env::var("ADX_ROOT") {
        println!("cargo:rustc-env=ADX_ROOT={root}");
    }
    println!("cargo:rerun-if-env-changed=ADX_ROOT");
    println!("cargo:rerun-if-changed=../../../../.git/HEAD");
}
