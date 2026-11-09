//! Bakes the git revision (shown by `aodm --version` and the version card) and,
//! when built by adx, the framework root (lets a standalone binary find its config).
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}

fn main() {
    let rev =
        git(&["describe", "--always", "--dirty", "--abbrev=7"]).unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=AODM_GIT={rev}");
    // Toolchain that built this binary (shown on the version card).
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let rustc_v = Command::new(rustc)
        .arg("--version")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    println!("cargo:rustc-env=AODM_RUSTC={rustc_v}");
    if let Ok(root) = std::env::var("ADX_ROOT") {
        println!("cargo:rustc-env=ADX_ROOT={root}");
    }
    println!("cargo:rerun-if-env-changed=ADX_ROOT");
    // Rebuild when the commit or the index changes (new commit, branch switch, staging).
    // shortcut: unstaged edits don't rerun this script, so `-dirty` can lag until the next stage/commit.
    if let Some(dir) = git(&["rev-parse", "--absolute-git-dir"]) {
        println!("cargo:rerun-if-changed={dir}/HEAD");
        println!("cargo:rerun-if-changed={dir}/index");
        if let Some(r) = git(&["symbolic-ref", "-q", "HEAD"]) {
            println!("cargo:rerun-if-changed={dir}/{r}");
        }
    }
}
