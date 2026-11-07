//! Contract tests for the CLI conventions (docs/aodm-cli.md §3): the `--json`
//! envelope, exit codes, `--dry-run`, `--sandbox`, and the P5a commands, run as the
//! real binary against fake images in a sandbox (no emulator, real homes untouched).

use std::fs;
use std::path::Path;
use std::process::Command;

const CONFIG: &str = include_str!("../../aod-core/tests/fixtures/config.ini");

fn aodm(sb: &Path, args: &[&str]) -> (i32, String, String) {
    // A fake home next to the sandbox: anything aodm writes outside DIR lands here,
    // and the test asserts it stays empty.
    let home = sb.parent().unwrap().join("home");
    let o = Command::new(env!("CARGO_BIN_EXE_aodm"))
        .env("HOME", &home)
        .env("XDG_STATE_HOME", home.join("state"))
        .env("XDG_DATA_HOME", home.join("data"))
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_CACHE_HOME", home.join("cache"))
        .arg("--sandbox")
        .arg(sb)
        .args(args)
        .output()
        .unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into(),
        String::from_utf8_lossy(&o.stderr).into(),
    )
}

fn json(sb: &Path, args: &[&str]) -> (i32, serde_json::Value) {
    let mut a = vec!["--json"];
    a.extend(args);
    let (code, out, err) = aodm(sb, &a);
    let v = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out} / {err}"));
    (code, v)
}

fn make_image(sb: &Path, name: &str) {
    let home = sb.join("avd");
    let dir = home.join(format!("{name}.avd"));
    fs::create_dir_all(dir.join("snapshots/default_boot")).unwrap();
    fs::write(dir.join("config.ini"), CONFIG).unwrap();
    fs::write(dir.join("userdata-qemu.img.qcow2"), "user data").unwrap();
    fs::write(dir.join("cache.img"), "cache").unwrap();
    fs::create_dir_all(dir.join("modem_simulator")).unwrap();
    fs::write(dir.join("modem_simulator/modem_nvram.json"), "{}").unwrap();
    fs::write(
        home.join(format!("{name}.ini")),
        format!(
            "avd.ini.encoding=UTF-8\npath={}\ntarget=android-37.0\n",
            dir.display()
        ),
    )
    .unwrap();
}

#[test]
fn conventions_and_p5a_commands() {
    let tmp = tempfile::tempdir().unwrap();
    let sb = &tmp.path().join("sb");
    fs::create_dir_all(sb.join("avd")).unwrap();
    fs::create_dir_all(tmp.path().join("home")).unwrap();
    make_image(sb, "phone-a");
    make_image(sb, "phone-b");

    // JSON envelope.
    let (code, v) = json(sb, &["list"]);
    assert_eq!(code, 0);
    assert_eq!(v["schema"], 1);
    assert_eq!(v["command"], "list");
    assert_eq!(v["ok"], true);
    assert_eq!(v["results"].as_array().unwrap().len(), 2);
    let (code, v) = json(sb, &["image", "show", "nope"]);
    assert_eq!((code, &v["ok"]), (1, &serde_json::json!(false)));
    assert!(v["errors"][0]["message"].as_str().unwrap().contains("nope"));

    // Exit codes: usage 2 (no selector, --dry-run on a command without it), refused 3.
    assert_eq!(aodm(sb, &["start"]).0, 2);
    assert_eq!(aodm(sb, &["--dry-run", "stop", "--all"]).0, 2);
    assert_eq!(
        aodm(sb, &["delete", "phone-b"]).0,
        3,
        "no terminal, no --yes"
    );
    assert_eq!(aodm(sb, &["config", "set", "ADX_AOD_MAX_SAVES", "4"]).0, 3);

    // Groups and tags, and selecting by them.
    assert_eq!(aodm(sb, &["group", "add", "qa", "phone-a", "phone-b"]).0, 0);
    assert_eq!(aodm(sb, &["group", "add", "qa", "ghost"]).0, 1);
    assert_eq!(aodm(sb, &["group", "add", "../x", "phone-a"]).0, 1);
    assert_eq!(aodm(sb, &["tag", "set", "phone-a", "owner=qa-team"]).0, 0);
    let (_, v) = json(sb, &["list", "--tag", "owner=qa-team"]);
    assert_eq!(v["results"].as_array().unwrap().len(), 1);
    assert_eq!(v["results"][0]["name"], "phone-a");
    let (_, v) = json(sb, &["group", "list"]);
    assert_eq!(
        v["results"][0]["images"],
        serde_json::json!(["phone-a", "phone-b"])
    );
    let (_, v) = json(sb, &["show", "phone-a"]);
    let r = &v["results"][0];
    assert_eq!(r["tags"]["owner"], "qa-team");
    assert_eq!(r["groups"], serde_json::json!(["qa"]));
    assert_eq!(r["running"], false);
    assert_eq!(aodm(sb, &["group", "delete", "qa"]).0, 0);
    assert_eq!(aodm(sb, &["tag", "unset", "phone-a", "owner"]).0, 0);
    let reg = fs::read_to_string(sb.join("aod/images.toml")).unwrap();
    assert!(!reg.contains("[images."), "empty entries dropped: {reg}");

    // Wipe: dry run changes nothing; the real one keeps config and identity.
    let dir = sb.join("avd/phone-a.avd");
    let (code, v) = json(sb, &["--dry-run", "wipe", "phone-a"]);
    assert_eq!(code, 0);
    assert_eq!(v["results"].as_array().unwrap().len(), 3);
    assert!(dir.join("userdata-qemu.img.qcow2").exists());
    assert_eq!(aodm(sb, &["wipe", "phone-a", "--yes"]).0, 0);
    for gone in ["userdata-qemu.img.qcow2", "cache.img", "snapshots"] {
        assert!(!dir.join(gone).exists(), "{gone}");
    }
    assert!(
        dir.join("config.ini").exists() && dir.join("modem_simulator/modem_nvram.json").exists()
    );
    assert!(
        !fs::read_dir(sb.join("avd"))
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with('.')),
        "no hidden work folder left"
    );

    // gc removes a dead run's leftover; disk sees the images.
    let left = sb.join("avd/.phone-b.avd.partial-999999999");
    fs::create_dir_all(&left).unwrap();
    let (_, v) = json(sb, &["disk"]);
    assert!(
        v["results"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "leftover")
    );
    let (code, v) = json(sb, &["--dry-run", "gc"]);
    assert_eq!(
        (code, v["results"][0]["removed"].as_bool()),
        (0, Some(false))
    );
    assert!(left.exists());
    assert_eq!(aodm(sb, &["gc"]).0, 0);
    assert!(!left.exists());

    // Deletes with --yes; the audit trail (sandbox log) has every change.
    assert_eq!(aodm(sb, &["--dry-run", "delete", "phone-b"]).0, 0);
    assert!(sb.join("avd/phone-b.ini").exists());
    assert_eq!(aodm(sb, &["delete", "phone-b", "--yes"]).0, 0);
    let (_, v) = json(sb, &["audit"]);
    let actions: Vec<&str> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["action"].as_str().unwrap())
        .collect();
    for a in [
        "group-add",
        "tag-set",
        "group-delete",
        "wipe",
        "gc",
        "delete",
    ] {
        assert!(actions.contains(&a), "{a} in {actions:?}");
    }
    let (_, v) = json(sb, &["audit", "--image", "phone-b"]);
    assert_eq!(
        v["results"].as_array().unwrap().len(),
        2,
        "group-add + delete"
    );

    let log = fs::read_to_string(sb.join("log/adx.log")).unwrap();
    assert!(log.contains("audit: wipe phone-a"));
    // Nothing was written outside the sandbox.
    let stray: Vec<_> = fs::read_dir(tmp.path().join("home"))
        .unwrap()
        .flatten()
        .collect();
    assert!(stray.is_empty(), "written outside the sandbox: {stray:?}");
}

/// Review findings: the sandbox must not reach a real folder through `path=` or
/// `path.rel=`; a dry run must not sweep; all-refused keeps exit 3; gc leaves files
/// it didn't make.
#[test]
fn sandbox_confinement_and_exit_codes() {
    let tmp = tempfile::tempdir().unwrap();
    let sb = &tmp.path().join("sb");
    fs::create_dir_all(sb.join("avd")).unwrap();
    fs::create_dir_all(tmp.path().join("home")).unwrap();
    // A "real" image outside the sandbox, and sandbox .ini files pointing at it.
    let real = tmp.path().join("real/victim.avd");
    fs::create_dir_all(&real).unwrap();
    fs::write(real.join("config.ini"), CONFIG).unwrap();
    fs::write(real.join("userdata-qemu.img.qcow2"), "precious").unwrap();
    fs::write(
        sb.join("avd/abs.ini"),
        format!("path={}\ntarget=android-37.0\n", real.display()),
    )
    .unwrap();
    fs::write(
        sb.join("avd/rel.ini"),
        "path.rel=avd/victim.avd\ntarget=android-37.0\n",
    )
    .unwrap();
    // path= outside: not an image of the sandbox at all.
    assert_eq!(aodm(sb, &["wipe", "abs", "--yes"]).0, 1);
    assert_eq!(aodm(sb, &["delete", "abs", "--yes"]).0, 1);
    // path.rel= resolves against the sandbox's own Android home: a broken image there.
    assert_eq!(aodm(sb, &["wipe", "rel", "--yes"]).0, 3);
    assert_eq!(aodm(sb, &["delete", "rel", "--yes"]).0, 0);
    assert!(
        real.join("userdata-qemu.img.qcow2").exists(),
        "real image untouched"
    );

    // A dry run leaves a dead run's work folder alone.
    make_image(sb, "phone");
    let left = sb.join("avd/.phone.avd.partial-999999999");
    fs::create_dir_all(&left).unwrap();
    assert_eq!(aodm(sb, &["--dry-run", "wipe", "phone"]).0, 0);
    assert_eq!(aodm(sb, &["--dry-run", "save", "prune", "phone"]).0, 0);
    assert!(left.exists());

    // Every target refused: exit 3, and per-image codes in JSON.
    fs::write(sb.join("avd/broken.ini"), "target=android-37.0\n").unwrap();
    let (code, v) = json(sb, &["start", "broken"]);
    assert_eq!(code, 3, "{v}");
    assert_eq!(v["results"][0]["code"], 3);

    // gc removes only aodm's own garbage.
    fs::create_dir_all(sb.join("saves/My Photos")).unwrap();
    fs::create_dir_all(sb.join("saves/phone")).unwrap();
    fs::write(sb.join("log/notes.log.9"), "mine").unwrap();
    fs::write(sb.join("log/adx.log.9"), "old").unwrap();
    assert_eq!(aodm(sb, &["gc"]).0, 0);
    assert!(sb.join("saves/My Photos").exists() && sb.join("log/notes.log.9").exists());
    assert!(!sb.join("saves/phone").exists() && !sb.join("log/adx.log.9").exists());
    assert!(!left.exists());

    // Usage errors.
    assert_eq!(aodm(sb, &["--json", "completion", "bash"]).0, 2);
    assert_eq!(aodm(sb, &["list", "--tag", "a/b=c"]).0, 2);
    assert_eq!(aodm(sb, &["audit", "--since", "tomorrow"]).0, 2);
    assert_eq!(aodm(sb, &["config", "get", "NOPE"]).0, 2);
}
