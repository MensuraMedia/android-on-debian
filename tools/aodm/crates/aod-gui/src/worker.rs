//! Background work for the window. The image list is read in-process through aod-core
//! (cheap, and it writes nothing to adx.log); screenshots of running phones come from
//! adb; actions run the `aodm` CLI, so they get its checks, locks, logging and audit.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use aod_core::adb::Adb;
use aod_core::card::{self, ImageRow};
use aod_core::env::Env;
use aod_core::log::Logger;
use aod_core::registry::Registry;
use aod_core::{avd, proc};
use serde_json::Value;

/// Thumbnails are at most this wide.
pub const THUMB_W: usize = 200;
/// How often the image list is read.
const LIST_EVERY: Duration = Duration::from_secs(3);
/// How often each awake phone is photographed (a raw screencap is ~10 MB through adb).
const SHOT_EVERY: Duration = Duration::from_secs(5);
/// A phone whose screen is off is retried this rarely.
const DARK_EVERY: Duration = Duration::from_secs(30);
/// Thumbnails are kept on disk so a stopped phone shows its last screen.
const SAVE_EVERY: Duration = Duration::from_secs(30);

pub fn running(r: &ImageRow) -> bool {
    r.state.starts_with("running")
}

/// adb serial of a running image whose console port is known.
pub fn serial(r: &ImageRow) -> Option<String> {
    let port = r.state.strip_prefix("running :")?;
    Some(format!("emulator-{port}"))
}

/// RGBA pixels, `w * h * 4` bytes.
pub struct Thumb {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
}

pub enum Msg {
    Cards(Result<Vec<ImageRow>, String>),
    /// A fresh screen; `None`: the phone's screen is off (black frame).
    Thumb(String, Option<Thumb>),
    /// An action finished: image, text for the activity list, whether it worked.
    Done {
        name: String,
        text: String,
        ok: bool,
    },
}

/// The `aodm` installed next to this binary, else the one on PATH.
pub fn aodm_bin() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| Some(p.parent()?.join("aodm")))
        .filter(|p| p.exists())
        .unwrap_or_else(|| "aodm".into())
}

/// Runs `aodm --json --quiet ARGS` (no terminal: aodm refuses anything that would
/// ask, never hangs on a prompt); returns (ok, message).
pub fn aodm(args: &[&str]) -> (bool, String) {
    let out = match Command::new(aodm_bin())
        .args(["--json", "--quiet"])
        .args(args)
        .stdin(Stdio::null())
        .output()
    {
        Ok(o) => o,
        Err(e) => return (false, format!("cannot run aodm: {e}")),
    };
    let Ok(doc) = serde_json::from_slice::<Value>(&out.stdout) else {
        let err = String::from_utf8_lossy(&out.stderr);
        let line = err
            .lines()
            .find(|l| l.starts_with("error"))
            .or(err.lines().next())
            .unwrap_or("aodm failed");
        return (false, line.to_string());
    };
    let ok = doc["ok"].as_bool().unwrap_or(false);
    let msg = match doc["errors"][0]["message"].as_str() {
        Some(m) => m.to_string(),
        None if ok => "done".into(),
        None => "failed".into(),
    };
    (ok, msg)
}

/// The same rows as `aodm list`.
pub fn list(env: &Env) -> Result<Vec<ImageRow>, String> {
    let images = avd::scan(&env.path("ANDROID_AVD_HOME"), &env.android_user_home())
        .map_err(|e| format!("scanning phone images: {e}"))?;
    let reg = Registry::load(&env.path("ADX_AOD_HOME")).unwrap_or_default();
    Ok(card::image_rows(env, &images, &proc::emulators(), &reg))
}

/// Raw `screencap` output: width, height, format (u32 LE), on newer Android a colour
/// space (u32), then pixels. Only RGBA_8888 (1) and RGBX_8888 (2) are read. Shrunk to
/// at most `THUMB_W` wide by averaging blocks; alpha is forced opaque.
pub fn thumb_from_raw(raw: &[u8]) -> Option<Thumb> {
    let u = |i: usize| -> Option<usize> {
        Some(u32::from_le_bytes(raw.get(i..i + 4)?.try_into().ok()?) as usize)
    };
    let (w, h, format) = (u(0)?, u(4)?, u(8)?);
    let px = w.checked_mul(h)?.checked_mul(4)?;
    let header = raw.len().checked_sub(px)?;
    if w == 0 || h == 0 || !(header == 12 || header == 16) || !(format == 1 || format == 2) {
        return None;
    }
    let pix = &raw[header..];
    let f = w.div_ceil(THUMB_W).max(1);
    let (tw, th) = (w / f, h / f);
    if tw == 0 || th == 0 {
        return None;
    }
    let mut rgba = Vec::with_capacity(tw * th * 4);
    for ty in 0..th {
        for tx in 0..tw {
            let mut sum = [0u32; 3];
            for y in ty * f..ty * f + f {
                let row = (y * w + tx * f) * 4;
                for p in pix[row..row + f * 4].chunks_exact(4) {
                    for c in 0..3 {
                        sum[c] += u32::from(p[c]);
                    }
                }
            }
            let n = (f * f) as u32;
            rgba.extend([
                (sum[0] / n) as u8,
                (sum[1] / n) as u8,
                (sum[2] / n) as u8,
                255,
            ]);
        }
    }
    Some(Thumb { w: tw, h: th, rgba })
}

/// All but black: what `screencap` returns while the display is asleep.
pub fn dark(t: &Thumb) -> bool {
    t.rgba
        .chunks_exact(4)
        .all(|p| p[0] < 8 && p[1] < 8 && p[2] < 8)
}

fn shot(adb: &Adb, serial: &str) -> Result<Thumb, String> {
    // Raw bytes (aod_core::cmd keeps text only); coreutils `timeout` bounds a hung adb.
    let o = Command::new("timeout")
        .args(["-k", "2", "10"])
        .arg(&adb.bin)
        .args(["-s", serial, "exec-out", "screencap"])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        let e = String::from_utf8_lossy(&o.stderr).trim().to_string();
        return Err(if e.is_empty() {
            format!("adb exited with {}", o.status)
        } else {
            e
        });
    }
    thumb_from_raw(&o.stdout).ok_or_else(|| "unreadable screenshot".into())
}

/// Stored thumbnail: `AODT`, width and height (u32 LE), RGBA.
pub fn load(path: &Path) -> Option<Thumb> {
    let b = fs::read(path).ok()?;
    let rest = b.strip_prefix(b"AODT")?;
    let w = u32::from_le_bytes(rest.get(0..4)?.try_into().ok()?) as usize;
    let h = u32::from_le_bytes(rest.get(4..8)?.try_into().ok()?) as usize;
    let rgba = rest.get(8..)?.to_vec();
    (rgba.len() == w.checked_mul(h)?.checked_mul(4)?).then_some(Thumb { w, h, rgba })
}

/// Atomic write, private (a screen can show messages or codes): folder 0700, file 0600.
fn store(path: &Path, t: &Thumb) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    let write = || -> std::io::Result<()> {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(b"AODT")?;
        f.write_all(&(t.w as u32).to_le_bytes())?;
        f.write_all(&(t.h as u32).to_le_bytes())?;
        f.write_all(&t.rgba)?;
        fs::rename(&tmp, path)
    };
    write().inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

/// Polls until the window closes (`wake` disconnects); a message on `wake` lists now.
pub fn poll(env: Env, log: Logger, tx: Sender<Msg>, wake: Receiver<()>, repaint: impl Fn()) {
    let adb = Adb::new(&env.path("ANDROID_HOME"));
    // When each phone was last photographed / its thumbnail last kept.
    let mut shot_at: HashMap<String, Instant> = HashMap::new();
    let mut saved: HashMap<String, Instant> = HashMap::new();
    let mut dark_now: HashMap<String, bool> = HashMap::new();
    // Last screenshot error per phone, so a booting phone logs once, not every poll.
    let mut failing: HashMap<String, String> = HashMap::new();
    let mut rows: Vec<ImageRow> = Vec::new();
    let mut listed: Option<Instant> = None;
    loop {
        if listed.is_none_or(|t| t.elapsed() >= LIST_EVERY) {
            let r = list(&env);
            if let Ok(v) = &r {
                rows = v.clone();
            }
            if tx.send(Msg::Cards(r)).is_err() {
                return;
            }
            listed = Some(Instant::now());
            repaint();
        }
        // The phone due longest ago, if any is due.
        let due = rows
            .iter()
            .filter_map(|r| Some((r.name.clone(), serial(r)?)))
            .filter(|(n, _)| {
                let every = if dark_now.get(n) == Some(&true) {
                    DARK_EVERY
                } else {
                    SHOT_EVERY
                };
                shot_at.get(n).is_none_or(|t| t.elapsed() >= every)
            })
            .min_by_key(|(n, _)| shot_at.get(n).copied());
        if let Some((name, serial)) = due {
            shot_at.insert(name.clone(), Instant::now());
            match shot(&adb, &serial) {
                Ok(t) => {
                    failing.remove(&name);
                    let is_dark = dark(&t);
                    dark_now.insert(name.clone(), is_dark);
                    // A black frame never replaces the stored last screen.
                    if !is_dark
                        && saved.get(&name).is_none_or(|t| t.elapsed() >= SAVE_EVERY)
                        && let Some(p) = aod_core::images::thumb(&env, &name)
                    {
                        if let Err(e) = store(&p, &t) {
                            log.warn(&format!("gui: cannot keep thumbnail of {name}: {e}"));
                        }
                        saved.insert(name.clone(), Instant::now());
                    }
                    let msg = Msg::Thumb(name, (!is_dark).then_some(t));
                    if tx.send(msg).is_err() {
                        return;
                    }
                    repaint();
                }
                Err(e) => {
                    if failing.get(&name) != Some(&e) {
                        log.info(&format!("gui: no screenshot of {name} ({serial}): {e}"));
                        failing.insert(name, e);
                    }
                }
            }
            continue;
        }
        match wake.recv_timeout(Duration::from_millis(500)) {
            Ok(()) => {
                while wake.try_recv().is_ok() {}
                listed = None;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(w: u32, h: u32, header: usize, format: u32, px: [u8; 4]) -> Vec<u8> {
        let mut b = w.to_le_bytes().to_vec();
        b.extend(h.to_le_bytes());
        b.extend(format.to_le_bytes());
        b.resize(header, 0);
        for _ in 0..w * h {
            b.extend(px);
        }
        b
    }

    #[test]
    fn screencap_is_shrunk() {
        let t = thumb_from_raw(&raw(1080, 2400, 16, 1, [10, 20, 30, 0])).unwrap();
        assert_eq!((t.w, t.h), (180, 400));
        assert_eq!(&t.rgba[..4], &[10, 20, 30, 255]);
        assert_eq!(t.rgba.len(), t.w * t.h * 4);
        assert!(!dark(&t));
        let land = thumb_from_raw(&raw(2400, 1080, 16, 1, [0; 4])).unwrap();
        assert!(land.w <= THUMB_W && land.h > 0 && dark(&land));
        let odd = thumb_from_raw(&raw(1081, 2401, 12, 2, [1; 4])).unwrap();
        assert!(odd.w <= THUMB_W);
        let small = thumb_from_raw(&raw(100, 50, 12, 1, [1, 2, 3, 4])).unwrap();
        assert_eq!((small.w, small.h), (100, 50));
        assert!(
            thumb_from_raw(&raw(10, 10, 16, 5, [0; 4])).is_none(),
            "BGRA unsupported"
        );
        assert!(thumb_from_raw(&raw(10, 10, 20, 1, [0; 4])).is_none());
        assert!(thumb_from_raw(b"short").is_none());
    }

    #[test]
    fn thumbnail_round_trip_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let d = std::env::temp_dir().join(format!("aod-gui-test-{}", std::process::id()));
        let p = d.join("thumbs/p.thumb");
        let t = Thumb {
            w: 2,
            h: 1,
            rgba: vec![1, 2, 3, 255, 4, 5, 6, 255],
        };
        store(&p, &t).unwrap();
        let back = load(&p).unwrap();
        assert_eq!((back.w, back.h, back.rgba), (2, 1, t.rgba));
        assert_eq!(
            fs::metadata(&p).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::read_dir(p.parent().unwrap()).unwrap().count(),
            1,
            "no temp left"
        );
        fs::remove_dir_all(d).unwrap();
    }
}
