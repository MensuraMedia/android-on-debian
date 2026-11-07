//! Version card: system specs and versions as data, rendered by each front end
//! (CLI text, TUI tables, later GUI grid). Everything is read from files; the only
//! process the card starts is `git describe` for the framework revision (loading
//! the configuration separately runs bash once, see env.rs).

use std::fs;
use std::path::Path;

use crate::avd::{self, Image};
use crate::env::Env;
use crate::proc::EmuProc;
use crate::registry::Registry;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Section {
    pub title: &'static str,
    pub rows: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ImageRow {
    pub name: String,
    pub state: String,
    pub api: String,
    pub abi: String,
    pub tag: String,
    pub revision: String,
    pub device: String,
    pub ram: String,
    pub size: String,
    /// Number of saves, e.g. `3 (1 pinned)`; `-` for none.
    pub saves: String,
    pub groups: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VersionCard {
    /// Left column, then right column.
    pub left: Vec<Section>,
    pub right: Vec<Section>,
    pub images: Vec<ImageRow>,
}

/// Builder inputs that come from the binary (its own version and toolchain).
pub struct Build<'a> {
    pub aodm: &'a str,
    pub rustc: &'a str,
}

fn read(path: &str) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn os_name() -> String {
    read("/etc/os-release")
        .and_then(|t| {
            t.lines().find_map(|l| {
                l.strip_prefix("PRETTY_NAME=")
                    .map(|v| v.trim_matches('"').to_string())
            })
        })
        .unwrap_or_else(|| "?".into())
}

fn cpu() -> String {
    let info = read("/proc/cpuinfo").unwrap_or_default();
    let model = info.lines().find_map(|l| {
        l.strip_prefix("model name")
            .and_then(|r| r.split_once(':'))
            .map(|(_, v)| v.trim().to_string())
    });
    let threads = info.lines().filter(|l| l.starts_with("processor")).count();
    format!(
        "{} ({threads} threads)",
        model.unwrap_or_else(|| "?".into())
    )
}

pub(crate) fn meminfo(key: &str) -> Option<u64> {
    read("/proc/meminfo")?
        .lines()
        .find_map(|l| {
            l.strip_prefix(key)?
                .trim()
                .trim_end_matches(" kB")
                .trim()
                .parse::<u64>()
                .ok()
        })
        .map(|kb| kb * 1024)
}

fn kvm() -> String {
    match fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/kvm")
    {
        Ok(_) => "accessible".into(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "missing".into(),
        Err(_) => "no access".into(),
    }
}

fn disk_free(path: &Path) -> String {
    // The saves directory may not exist yet: measure its nearest existing ancestor.
    let existing = path
        .ancestors()
        .find(|p| p.exists())
        .unwrap_or(Path::new("/"));
    rustix::fs::statvfs(existing)
        .map(|s| avd::human(s.f_bavail * s.f_frsize))
        .unwrap_or_else(|_| "?".into())
}

/// adx version: the first released `## [x.y.z]` in CHANGELOG.md, plus `git describe`.
fn adx_version(root: &str) -> String {
    if root.is_empty() {
        return "not found".into();
    }
    let released = read(&format!("{root}/CHANGELOG.md")).and_then(|t| {
        t.lines().find_map(|l| {
            let (v, _) = l.strip_prefix("## [")?.split_once(']')?;
            v.starts_with(|c: char| c.is_ascii_digit())
                .then(|| v.to_string())
        })
    });
    let git = std::process::Command::new("git")
        .args(["-C", root, "describe", "--always", "--dirty", "--abbrev=7"])
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    match (released, git) {
        (Some(v), Some(g)) => format!("{v} ({g})"),
        (Some(v), None) => v,
        (None, Some(g)) => g,
        (None, None) => "?".into(),
    }
}

fn sdk_rev(home: &Path, rel: &str) -> String {
    avd::props(&home.join(rel).join("source.properties"), "Pkg.Revision")
        .unwrap_or_else(|| "missing".into())
}

fn dirs(path: &Path) -> String {
    let mut v: Vec<String> = fs::read_dir(path)
        .map(|it| {
            it.flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    if v.is_empty() {
        "none".into()
    } else {
        v.join(", ")
    }
}

fn row(k: &str, v: impl Into<String>) -> (String, String) {
    (k.to_string(), v.into())
}

pub fn build(
    env: &Env,
    b: &Build,
    images: &[Image],
    procs: &[EmuProc],
    reg: &Registry,
) -> VersionCard {
    let sdk = env.path("ANDROID_HOME");
    let total = meminfo("MemTotal:")
        .map(avd::human)
        .unwrap_or_else(|| "?".into());
    let avail = meminfo("MemAvailable:")
        .map(avd::human)
        .unwrap_or_else(|| "?".into());
    let emu_env = if env.get("ADX_EMU_ENV").contains("__NV_PRIME_RENDER_OFFLOAD") {
        "NVIDIA offload".to_string()
    } else if env.get("ADX_EMU_ENV").is_empty() {
        "none".into()
    } else {
        "custom".into()
    };
    let left = vec![
        Section {
            title: "aodm",
            rows: vec![
                row("version", b.aodm),
                row("adx", adx_version(env.get("ADX_ROOT"))),
                row(
                    "config",
                    if env.get("ADX_ROOT").is_empty() {
                        "fallbacks only".into()
                    } else {
                        env.get("ADX_ROOT").to_string()
                    },
                ),
            ],
        },
        Section {
            title: "Host",
            rows: vec![
                row("os", os_name()),
                row(
                    "kernel",
                    read("/proc/sys/kernel/osrelease").unwrap_or_else(|| "?".into()),
                ),
                row("cpu", cpu()),
                row("ram", format!("{total} ({avail} available)")),
                row("kvm", kvm()),
                row(
                    "disk free",
                    format!("{} (saves fs)", disk_free(&env.path("ADX_AOD_SAVES_DIR"))),
                ),
            ],
        },
    ];
    let right = vec![
        Section {
            title: "Toolchain",
            rows: vec![
                row("emulator", sdk_rev(&sdk, "emulator")),
                row("adb", sdk_rev(&sdk, "platform-tools")),
                row("cmdline-tools", sdk_rev(&sdk, "cmdline-tools/latest")),
                row("SDK platform", dirs(&sdk.join("platforms"))),
                row("build-tools", dirs(&sdk.join("build-tools"))),
                row(
                    "JDK",
                    avd::props(&env.path("ADX_JAVA_HOME").join("release"), "JAVA_VERSION")
                        .unwrap_or_else(|| "missing".into()),
                ),
                row("Rust", b.rustc),
            ],
        },
        Section {
            title: "Emulation",
            rows: vec![
                row("gpu mode", env.get("ADX_EMU_GPU")),
                row("emu env", emu_env),
                row(
                    "running",
                    format!(
                        "{} of max {}",
                        procs.len(),
                        env.get("ADX_AOD_MAX_INSTANCES")
                    ),
                ),
            ],
        },
    ];
    let images = images
        .iter()
        .map(|i| ImageRow {
            name: i.name.clone(),
            state: crate::emu::running(procs, i)
                .map(|p| match p.port {
                    Some(port) => format!("running :{port}"),
                    None => "running".into(),
                })
                .unwrap_or_else(|| {
                    if i.problem.is_some() {
                        "broken".into()
                    } else {
                        "stopped".into()
                    }
                }),
            api: i.api().into(),
            abi: i.abi().into(),
            tag: i.tag().into(),
            revision: i.sysimg_revision(&sdk).unwrap_or_else(|| "?".into()),
            device: i.device().into(),
            ram: i.ram().into(),
            size: avd::human(i.disk_bytes()),
            saves: {
                let s = crate::saves::list(&env.path("ADX_AOD_SAVES_DIR"), Some(&i.name))
                    .unwrap_or_default();
                let pinned = s.iter().filter(|s| s.pinned).count();
                match (s.len(), pinned) {
                    (0, _) => "-".into(),
                    (n, 0) => n.to_string(),
                    (n, p) => format!("{n} ({p} pinned)"),
                }
            },
            groups: reg.groups(&i.name).join(","),
        })
        .collect();
    VersionCard {
        left,
        right,
        images,
    }
}

/// Plain-text rendering: two even columns (each padded to its longest key and
/// value), then the image table.
pub fn render_text(card: &VersionCard) -> String {
    let column = |sections: &[Section]| -> Vec<String> {
        let kw = sections
            .iter()
            .flat_map(|s| s.rows.iter().map(|(k, _)| k.len()))
            .max()
            .unwrap_or(0);
        let mut out = Vec::new();
        for (i, s) in sections.iter().enumerate() {
            if i > 0 {
                out.push(String::new());
            }
            out.push(s.title.to_string());
            out.extend(s.rows.iter().map(|(k, v)| format!("  {k:<kw$}  {v}")));
        }
        out
    };
    let (l, r) = (column(&card.left), column(&card.right));
    let lw = l.iter().map(|s| s.chars().count()).max().unwrap_or(0) + 4;
    let mut out = String::new();
    for i in 0..l.len().max(r.len()) {
        let left = l.get(i).map(String::as_str).unwrap_or("");
        let right = r.get(i).map(String::as_str).unwrap_or("");
        let pad = lw - left.chars().count();
        out.push_str(format!("{left}{}{right}", " ".repeat(pad)).trim_end());
        out.push('\n');
    }
    out.push('\n');
    out.push_str(&render_images(&card.images));
    out
}

/// The image table, also used by `aodm list`.
pub fn render_images(rows: &[ImageRow]) -> String {
    let head = [
        "IMAGE", "STATE", "API", "ABI", "TAG", "REV", "DEVICE", "RAM", "SIZE", "SAVES", "GROUPS",
    ];
    let cells: Vec<[&str; 11]> = rows
        .iter()
        .map(|r| {
            [
                r.name.as_str(),
                &r.state,
                &r.api,
                &r.abi,
                &r.tag,
                &r.revision,
                &r.device,
                &r.ram,
                &r.size,
                &r.saves,
                &r.groups,
            ]
        })
        .collect();
    let width = |c: usize| {
        cells
            .iter()
            .map(|r| r[c].chars().count())
            .chain([head[c].len()])
            .max()
            .unwrap_or(0)
    };
    let widths: Vec<usize> = (0..head.len()).map(width).collect();
    let line = |vals: [&str; 11]| {
        let s: Vec<String> = vals
            .iter()
            .zip(&widths)
            .map(|(v, w)| format!("{v:<w$}"))
            .collect();
        s.join("  ").trim_end().to_string() + "\n"
    };
    let mut out = line(head);
    if cells.is_empty() {
        out.push_str("(no images)\n");
    }
    for c in cells {
        out.push_str(&line(c));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card() -> VersionCard {
        let s = |t, rows: &[(&str, &str)]| Section {
            title: t,
            rows: rows
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        };
        VersionCard {
            left: vec![
                s("aodm", &[("version", "0.3.0"), ("adx", "0.2.0")]),
                s("Host", &[("os", "Mint"), ("disk free", "303G")]),
            ],
            right: vec![s(
                "Toolchain",
                &[("emulator", "37.2.13"), ("cmdline-tools", "23.0")],
            )],
            images: vec![ImageRow {
                name: "adx_api37".into(),
                state: "running :5554".into(),
                api: "37.0".into(),
                abi: "x86_64".into(),
                tag: "google_apis_playstore".into(),
                revision: "7".into(),
                device: "pixel_8".into(),
                ram: "2G".into(),
                size: "6.4G".into(),
                saves: "-".into(),
                groups: String::new(),
            }],
        }
    }

    #[test]
    fn columns_are_even_and_aligned() {
        let text = render_text(&card());
        let lines: Vec<&str> = text.lines().collect();
        // Right column: title at one offset, its rows indented by 2 at the same offset each.
        let col = |needle: &str| lines.iter().find_map(|l| l.find(needle)).unwrap();
        let title = col("Toolchain");
        assert_eq!(col("emulator"), title + 2, "{text}");
        assert_eq!(col("cmdline-tools"), title + 2, "{text}");
        // Left keys padded to the longest left key ("disk free").
        assert!(text.contains("  version    0.3.0"), "{text}");
        assert!(text.contains("  disk free  303G"), "{text}");
        assert!(text.contains("IMAGE      STATE"), "{text}");
        assert!(text.contains("adx_api37  running :5554"), "{text}");
    }

    #[test]
    fn empty_image_table() {
        assert!(render_images(&[]).contains("(no images)"));
    }
}
