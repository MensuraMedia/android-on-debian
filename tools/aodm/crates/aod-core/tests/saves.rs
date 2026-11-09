//! Saves, clone, rename and delete end to end on a fake SDK in a temp dir (no emulator).

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use aod_core::avd::{self, Image};
use aod_core::env::Env;
use aod_core::ini::IniDoc;
use aod_core::lifecycle::Ctx;
use aod_core::log::Logger;
use aod_core::registry::Registry;
use aod_core::{images, saves};

const CONFIG: &str = include_str!("fixtures/config.ini");

fn quiet(_: &str) {}

fn env(root: &Path) -> Env {
    let p = |s: &str| root.join(s).to_string_lossy().to_string();
    let config: HashMap<String, String> = [
        ("ANDROID_HOME", p("sdk")),
        ("ANDROID_USER_HOME", p("dot-android")),
        ("ANDROID_AVD_HOME", p("dot-android/avd")),
        ("ADX_AOD_HOME", p("aod")),
        ("ADX_AOD_SAVES_DIR", p("aod/saves")),
        ("ADX_AOD_MAX_SAVES", "2".into()),
        ("ADX_AVD_NAME", "adx_api37".into()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    Env::resolve(&HashMap::new(), &config, None, &p("home"))
}

/// An image as avdmanager + one emulator run leave it.
fn make_image(env: &Env, name: &str) {
    let home = env.path("ANDROID_AVD_HOME");
    let dir = home.join(format!("{name}.avd"));
    fs::create_dir_all(dir.join("snapshots/default_boot")).unwrap();
    fs::write(dir.join("config.ini"), CONFIG).unwrap();
    fs::write(dir.join("userdata-qemu.img.qcow2"), "user data v1").unwrap();
    fs::write(dir.join("snapshots/default_boot/ram.bin"), "ram").unwrap();
    for f in [
        "multiinstance.lock",
        "hardware-qemu.ini",
        "bootcompleted.ini",
    ] {
        fs::write(dir.join(f), "").unwrap();
    }
    fs::write(
        home.join(format!("{name}.ini")),
        format!(
            "avd.ini.encoding=UTF-8\npath={}\npath.rel=avd/{name}.avd\ntarget=android-37.0\n",
            dir.display()
        ),
    )
    .unwrap();
}

fn scan(env: &Env) -> Vec<Image> {
    avd::scan(&env.path("ANDROID_AVD_HOME"), &env.android_user_home()).unwrap()
}

fn get<'a>(imgs: &'a [Image], n: &str) -> &'a Image {
    imgs.iter().find(|i| i.name == n).unwrap()
}

#[test]
fn save_restore_open_clone_rename_delete() {
    let root = tempfile::tempdir().unwrap();
    let env = env(root.path());
    let log = Logger::to_file(root.path().join("adx.log"), "test");
    let ctx = Ctx::new(&env, &log, quiet);
    let saves_dir = env.path("ADX_AOD_SAVES_DIR");
    // Every path aodm writes to stays inside the temp dir.
    for k in ["ANDROID_AVD_HOME", "ADX_AOD_HOME", "ADX_AOD_SAVES_DIR"] {
        assert!(env.path(k).starts_with(root.path()), "{k}");
    }
    make_image(&env, "phone");
    let imgs = scan(&env);
    let phone = get(&imgs, "phone");
    assert!(phone.problem.is_none(), "{:?}", phone.problem);

    // Refused while running (the emulator holds multiinstance.lock).
    {
        let _running = images::lock("phone", &phone.dir).unwrap();
        let e = saves::create(&ctx, phone, None).err().unwrap();
        assert!(e.to_string().contains("stop it first"), "{e}");
        assert!(images::clone(&ctx, phone, "x").is_err());
    }

    // Save: per-run files dropped, snapshot kept, checksums recorded.
    let s1 = saves::create(&ctx, phone, Some("clean")).unwrap();
    assert!(s1.id.ends_with("-clean"));
    let m = s1.manifest.as_ref().unwrap();
    let files: Vec<&str> = m.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(
        files,
        [
            "config.ini",
            "snapshots/default_boot/ram.bin",
            "userdata-qemu.img.qcow2"
        ]
    );
    assert_eq!(
        (m.image.as_str(), m.target.as_str()),
        ("phone", "android-37.0")
    );
    assert!(saves::verify(&ctx, &s1).unwrap().is_empty());
    assert!(s1.path.join("image.ini").exists());

    // Retention: a second save fits, the third is refused until one is pinned.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let s2 = saves::create(&ctx, phone, None).unwrap();
    let e = saves::create(&ctx, phone, Some("third")).err().unwrap();
    assert!(e.to_string().contains("ADX_AOD_MAX_SAVES=2"), "{e}");
    saves::pin(&ctx, &s1, true).unwrap();

    // A damaged save is detected and never restored.
    fs::write(s2.avd().join("userdata-qemu.img.qcow2"), "bit rot").unwrap();
    let s2 = saves::find(&saves_dir, "phone", &s2.id).unwrap();
    assert_eq!(
        saves::verify(&ctx, &s2).unwrap(),
        ["changed: userdata-qemu.img.qcow2"]
    );
    assert!(saves::restore(&ctx, &s2, phone).is_err());
    assert_eq!(
        fs::read_to_string(phone.dir.join("userdata-qemu.img.qcow2")).unwrap(),
        "user data v1",
        "failed restore changed nothing"
    );
    let freed = saves::delete(&ctx, &s2);
    assert!(freed.is_ok());
    assert!(
        saves::delete(&ctx, &saves::find(&saves_dir, "phone", "latest").unwrap()).is_err(),
        "pinned"
    );

    // Restore in place: data back, snapshot kept (same folder), nothing left over.
    fs::write(phone.dir.join("userdata-qemu.img.qcow2"), "user data v2").unwrap();
    let s1 = saves::find(&saves_dir, "phone", "latest").unwrap();
    let msg = saves::restore(&ctx, &s1, phone).unwrap();
    assert!(!msg.contains("cold"), "{msg}");
    assert_eq!(
        fs::read_to_string(phone.dir.join("userdata-qemu.img.qcow2")).unwrap(),
        "user data v1"
    );
    assert!(phone.dir.join("snapshots/default_boot/ram.bin").exists());
    let leftovers: Vec<_> = fs::read_dir(env.path("ANDROID_AVD_HOME"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with('.'))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");

    // Open as new: own folder and .ini, renamed config, cold first boot, origin recorded.
    saves::open(&ctx, &s1, "copy").unwrap();
    let imgs = scan(&env);
    let copy = get(&imgs, "copy");
    assert!(copy.problem.is_none(), "{:?}", copy.problem);
    assert_eq!(copy.ini.get("path.rel"), Some("avd/copy.avd"));
    assert_eq!(copy.config.as_ref().unwrap().get("avd.name"), Some("copy"));
    assert!(!copy.dir.join("snapshots/default_boot").exists());
    let reg = Registry::load(&env.path("ADX_AOD_HOME")).unwrap();
    assert_eq!(
        reg.entry("copy").unwrap().origin,
        Some(format!("phone@{}", s1.id))
    );
    assert!(saves::open(&ctx, &s1, "copy").is_err(), "name taken");

    // Clone, then rename the clone (with a save of its own and a group).
    images::clone(&ctx, get(&imgs, "phone"), "twin").unwrap();
    let imgs = scan(&env);
    let twin = get(&imgs, "twin");
    assert!(!twin.dir.join("snapshots/default_boot").exists());
    assert!(!twin.dir.join("multiinstance.lock").exists());
    saves::create(&ctx, twin, None).unwrap();
    Registry::update(&env.path("ADX_AOD_HOME"), |r| {
        r.images.get_mut("twin").unwrap().groups.push("qa".into());
    })
    .unwrap();
    images::rename(&ctx, twin, "twin2").unwrap();
    let imgs = scan(&env);
    assert!(imgs.iter().all(|i| i.name != "twin"));
    let twin2 = get(&imgs, "twin2");
    assert!(twin2.problem.is_none(), "{:?}", twin2.problem);
    assert_eq!(
        IniDoc::load(&twin2.dir.join("config.ini"))
            .unwrap()
            .get("avd.id"),
        Some("twin2")
    );
    assert_eq!(saves::list(&saves_dir, Some("twin2")).unwrap().len(), 1);
    let reg = Registry::load(&env.path("ADX_AOD_HOME")).unwrap();
    assert_eq!(reg.groups("twin2"), ["qa"]);
    assert!(reg.entry("twin").is_none());

    // adx's own image is never renamed or deleted by aodm.
    make_image(&env, "adx_api37");
    let imgs = scan(&env);
    assert!(images::rename(&ctx, get(&imgs, "adx_api37"), "y").is_err());
    assert!(images::delete(&ctx, get(&imgs, "adx_api37"), &imgs).is_err());

    // A corrupt .ini pointing at another image's folder: delete never removes that folder.
    fs::write(
        env.path("ANDROID_AVD_HOME").join("evil.ini"),
        format!(
            "path={}\ntarget=android-37.0\n",
            get(&imgs, "phone").dir.display()
        ),
    )
    .unwrap();
    let imgs = scan(&env);
    images::delete(&ctx, get(&imgs, "evil"), &imgs).unwrap();
    assert!(get(&imgs, "phone").dir.join("config.ini").exists());

    // Delete: folder, .ini and registry entry gone; saves kept.
    let msg = images::delete(&ctx, get(&imgs, "twin2"), &imgs).unwrap();
    assert!(msg.contains("1 save(s) kept"), "{msg}");
    let imgs = scan(&env);
    assert!(imgs.iter().all(|i| i.name != "twin2"));
    assert!(!env.path("ANDROID_AVD_HOME").join("twin2.avd").exists());
    assert!(
        Registry::load(&env.path("ADX_AOD_HOME"))
            .unwrap()
            .entry("twin2")
            .is_none()
    );

    // Prune keeps pinned saves.
    let all = saves::list(&saves_dir, Some("phone")).unwrap();
    assert!(
        saves::prune_plan(&all, 0).is_empty(),
        "only a pinned save left"
    );
}

#[test]
fn refuses_unsafe_inputs_and_sweeps_leftovers() {
    let root = tempfile::tempdir().unwrap();
    let env = env(root.path());
    let log = Logger::to_file(root.path().join("adx.log"), "test");
    let ctx = Ctx::new(&env, &log, quiet);
    let home = env.path("ANDROID_AVD_HOME");
    let saves_dir = env.path("ADX_AOD_SAVES_DIR");

    // A save "image" argument is never a path: nothing outside the saves folder is reachable.
    let victim = root.path().join("victim/data");
    fs::create_dir_all(&victim).unwrap();
    for bad in ["", "..", root.path().join("victim").to_str().unwrap()] {
        assert!(saves::find(&saves_dir, bad, "data").is_err(), "{bad:?}");
        assert!(saves::list(&saves_dir, Some(bad)).is_err(), "{bad:?}");
    }
    assert!(victim.exists());

    // An image folder that is a symlink (moved to another disk) is never copied or
    // edited through the link; delete removes only the link.
    make_image(&env, "real");
    let big = root.path().join("big");
    fs::create_dir_all(&big).unwrap();
    fs::rename(home.join("real.avd"), big.join("sym.avd")).unwrap();
    fs::remove_file(home.join("real.ini")).unwrap();
    std::os::unix::fs::symlink(big.join("sym.avd"), home.join("sym.avd")).unwrap();
    fs::write(
        home.join("sym.ini"),
        format!(
            "path={}\ntarget=android-37.0\n",
            home.join("sym.avd").display()
        ),
    )
    .unwrap();
    let imgs = scan(&env);
    let sym = get(&imgs, "sym");
    assert!(sym.problem.is_none(), "{:?}", sym.problem);
    assert!(saves::create(&ctx, sym, None).is_err());
    assert!(images::clone(&ctx, sym, "twin").is_err());
    assert!(images::rename(&ctx, sym, "other").is_err());
    assert!(
        big.join("sym.avd/hardware-qemu.ini").exists(),
        "live folder untouched"
    );
    let msg = images::delete(&ctx, sym, &imgs).unwrap();
    assert!(msg.contains("symlink"), "{msg}");
    assert!(big.join("sym.avd/config.ini").exists(), "target left alone");

    // A registry from a newer aodm: refused before anything is copied.
    make_image(&env, "phone");
    let aod = env.path("ADX_AOD_HOME");
    fs::create_dir_all(&aod).unwrap();
    fs::write(aod.join("images.toml"), "schema = 2\n").unwrap();
    let imgs = scan(&env);
    assert!(images::clone(&ctx, get(&imgs, "phone"), "twin").is_err());
    assert!(!home.join("twin.avd").exists() && !home.join("twin.ini").exists());
    fs::remove_file(aod.join("images.toml")).unwrap();

    // Leftovers of dead runs are swept; a live run's (our own pid) is kept.
    let dead = home.join(".x.avd.partial-999999999");
    let live = home.join(format!(".y.avd.restore-{}", std::process::id()));
    let dead_save = saves_dir.join("phone/.partial-999999999");
    let dead_del = saves_dir.join("phone/.deleting-20261106-999999999");
    for d in [&dead, &live, &dead_save, &dead_del] {
        fs::create_dir_all(d).unwrap();
    }
    images::sweep_all(&ctx);
    assert!(!dead.exists() && !dead_save.exists() && !dead_del.exists());
    assert!(live.exists());
    assert!(
        fs::read_to_string(root.path().join("adx.log"))
            .unwrap()
            .contains("leftover")
    );

    // A relative saves folder would land under the current directory: refused.
    let mut cfg = HashMap::new();
    cfg.insert("ADX_AOD_SAVES_DIR".to_string(), "rel/saves".to_string());
    let rel = Env::resolve(&HashMap::new(), &cfg, None, "/h");
    assert!(images::cfg_dir(&rel, "ADX_AOD_SAVES_DIR").is_err());
}
