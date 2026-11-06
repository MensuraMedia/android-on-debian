# Container-based Android runtimes on Linux (Waydroid, Anbox / Anbox Cloud, redroid, Docker emulator images), state as of Nov 2026

Target host: Linux Mint 22.3 (Ubuntu 24.04 base), Cinnamon on **X11**, kernel 7.0.0-38-generic, i7-13620H, 31 GB RAM, Docker, KVM.

**Local host facts (checked on the target machine, 2026-11-05):**
- `/boot/config-7.0.0-38-generic` contains `CONFIG_ANDROID_BINDER_IPC=m`, `CONFIG_ANDROID_BINDERFS=m`, `CONFIG_ANDROID_BINDER_DEVICES=""`, and `# CONFIG_ANDROID_BINDER_IPC_RUST is not set`. The module ships at `/lib/modules/7.0.0-38-generic/kernel/drivers/android/binder_linux.ko.zst`. It was **not loaded** when checked.
- The config has **no ASHMEM entry** (ashmem was removed from mainline Linux years ago). Any runtime that still expects `ashmem_linux` therefore has to fall back to memfd.
- `XDG_SESSION_TYPE=x11`. The session is X11, not Wayland.

GitHub and Docker Hub figures below come from the GitHub REST API and Docker Hub API, queried on 2026-11-05.

| Project | Stars | Last push / commit | Archived | License |
|---|---|---|---|---|
| waydroid/waydroid | 12,492 | 2026-10-16 | no | GPL-3.0 |
| anbox/anbox (legacy) | 9,024 | 2024-02-06 | **yes** | GPL-3.0 |
| remote-android/redroid-doc | 7,101 | 2026-05-17 (last commit: "remove slack link") | no | none declared |
| budtmo/docker-android | 16,095 | 2026-11-05 | no | NOASSERTION (custom LICENSE.md) |
| HQarroum/docker-android | 7,395 | 2026-05-07 | no | MIT |
| google/android-emulator-container-scripts | 2,111 | 2026-10-06 (merge: "Bump actions") | no | Apache-2.0 |

## Waydroid: Android version, requirements (Wayland, binder, X11 Cinnamon), ADB, GApps, dev suitability, maintenance

### Takeaway
Waydroid is actively maintained (v1.6.3, 2026-05-28; commits as recent as Oct 2026). The official default images are still LineageOS 20 (Android 13); the host tool added initial support for Android 16 images in 1.6.3. It works on an X11 Cinnamon session only inside a nested Wayland compositor (Weston). It also needs real GPU acceleration: Intel iGPU is fine, the proprietary NVIDIA driver is not. You get ADB over TCP. Play Store needs the GAPPS image plus manual registration as an uncertified device. Android Studio sees it only as a generic network ADB device.

### Cited Findings
- **Release history:** 1.5.0 (29 Mar 2025), 1.5.4 (Jun 2025), 1.6.0 (21 Nov 2025), 1.6.1 (2025-12-29), 1.6.2 (2026-02-22), 1.6.3 (2026-05-28). The last commit on the main repo was 2026-10-16, and the repo has 12,492 stars. Source: GitHub API and [Waydroid releases](https://github.com/waydroid/waydroid/releases).
- **1.6.3 changes:** "Initial support for Android 16 images", plus handling of Android-initiated reboot/shutdown. — [Waydroid releases](https://github.com/waydroid/waydroid/releases)
- **1.6.0 changes:** turns **ADB secure mode on by default**, adds a `bugreport` command and notification forwarding. Notification forwarding requires an updated Android system image. 1.5.3 declares OpenGL ES 3.2 support. 1.5.0 added "initial support for future Android 13 images". — [Waydroid releases](https://github.com/waydroid/waydroid/releases)
- **Default image (unconfirmed):** an Android 13 / LineageOS-derived base. One article cites a build named `lineage-20.0-20260428-MAINLINE-waydroid_x86_64-vendor.zip` on SourceForge. This is a secondary, less authoritative source that I could not check against the official image server. — [tech-insider.org](https://tech-insider.org/waydroid-setup-linux-steam-deck-2026/)
- **Android TV variant:** the community WayDroid-ATV project ships LineageOS 20.0 (Android 13) Android TV images, with tags into 2026 (20260106, 20260403). — [WayDroid-ATV releases](https://github.com/WayDroid-ATV/waydroid-androidtv-builds/releases/tag/20250327)
- **Image types:** `waydroid init -s` takes VANILLA (the default), FOSS or GAPPS. `show-full-ui` shows the Android full screen in a window. — [Waydroid CLI docs](https://docs.waydro.id/usage/waydroid-command-line-options)
- **Install on Debian/Ubuntu:** `sudo apt install waydroid`. Debian 13 needs backports. Older releases use the repo script, `curl -s https://repo.waydro.id | sudo bash`. The official docs say a **Wayland session** is required on Ubuntu 22.04+ and give no X11 workaround. — [Waydroid install docs](https://docs.waydro.id/usage/install-on-desktops)
- **X11 Cinnamon on Mint 22.3 (community guide, 2026-07-27, tested with Waydroid 1.6.2):**
  - Install Weston, then run `weston --backend=x11 --width=1280 --height=720 --socket=waydroid &` followed by `WAYLAND_DISPLAY=waydroid waydroid show-full-ui`.
  - The Weston window size becomes the Android screen size. Weston's bar takes about 32 px.
  - For full screen, use `--fullscreen` together with an explicit resolution.
  - Do not run the session with sudo.
  - With multi-window mode (`persist.waydroid.multi_windows true`) on X11, apps still stay inside the Weston window.
  
  Source: [Linux Mint discussion #1617](https://github.com/orgs/linuxmint/discussions/1617)
- **Hardware requirements from the same guide:**
  - `/dev/dri/renderD*` must exist, and the renderer must not be llvmpipe. Software rendering prevents startup.
  - The proprietary NVIDIA driver is not supported. Nouveau starts but performs poorly.
  - Needs at least 4 GB RAM and about 3 GB disk.
  - The image is x86_64 with no ARM translation, so ARM-only apps won't run.
  - A black window while the status says RUNNING usually means no 3D acceleration, multi-window mode is on, or the container needs a restart.
  
  Source: [Linux Mint discussion #1617](https://github.com/orgs/linuxmint/discussions/1617)
- **Wayland alternative:** Mint's "Cinnamon (Wayland)" session can host Waydroid directly but is still experimental. Nested compositors give a single Android window, with no native multi-window. — [community thread summary, Mint/Lemmy/Manjaro](https://forum.manjaro.org/t/how-to-install-run-waydroid-on-xfce-or-other-non-wayland-desktops/152925)
- **GApps / Play Store:**
  - Run `sudo waydroid init -f -s GAPPS`. This wipes data. The image is about 1.2 GB, versus about 700 MB for VANILLA.
  - Restart with `sudo systemctl restart waydroid-container`.
  - Google refuses sign-in until the `android_id` (read from `gservices.db` via `waydroid shell`) is registered on Google's uncertified-device page.
  
  Source: [Linux Mint discussion #1617](https://github.com/orgs/linuxmint/discussions/1617)
- **ADB:** get the IP from Android Settings > About, then run `adb connect <IP>:5555`. — [Waydroid ADB FAQ](https://docs.waydro.id/faq/using-adb-with-waydroid)
- **Full-screen sessions:** official docs show how to run Waydroid full screen under Cage, Mutter, Weston or Wayfire sessions (`cage waydroid show-full-ui`, etc.). — [Waydroid docs: Waydroid-only sessions](https://docs.waydro.id/faq/setting-up-waydroid-only-sessions)

### Inferences
- On the target machine (Intel Raptor Lake iGPU with Mesa, kernel binder modules present as `=m`), Waydroid should be workable through nested Weston. If the laptop also has an NVIDIA dGPU, Waydroid must render on the Intel iGPU.
- Because 1.6.0 enabled ADB secure mode, the first `adb connect` probably needs an on-screen authorization prompt, as on a physical device. The release note says "ADB secure mode on by default"; I did not confirm the exact UX.
- **Dev suitability:** Waydroid is good for fast, near-native app smoke testing on x86_64 and for `adb install`/logcat over TCP.
- It is weak as a primary debug target:
  - Android Studio shows it only as a generic network ADB device.
  - The default image is Android 13, below current targetSdk levels.
  - It cannot run ARM-only native libs.
  - It is not Google-certified.
  - There are no emulator extended controls (sensors, GPS, camera simulation).

### Gaps
- I could not confirm the exact LineageOS version, or the date, of the current official OTA image (ota.waydro.id). I also don't know whether an official Android 16 (LineageOS 23) image has shipped yet, or only host-side support.
- ArchWiki was blocked by bot protection, so kernel/binderfs details come from community sources.
- I found no source on how well Android Studio's debugger (JDWP) attaches over Waydroid's ADB. It is assumed to work like any TCP ADB device.

## Anbox legacy vs Anbox Cloud: status, pricing/licensing, use cases

### Takeaway
Legacy Anbox is dead: deprecated in Feb 2023, and the GitHub repo was archived on 13 Feb 2024. Its own page points desktop users to Waydroid. Anbox Cloud is Canonical's commercial, closed-source platform for streaming and CI at scale. It needs a paid Ubuntu Pro (or Pro apps-only) subscription; the free personal Pro tier does not work. It is not a realistic local dev tool for a single Mint workstation.

### Cited Findings
- **Legacy Anbox:**
  - Deprecated 3 Feb 2023 as no longer maintained. — [Wikipedia: Anbox](https://en.wikipedia.org/wiki/Anbox)
  - The repo was archived 13 Feb 2024 with no further maintenance. The project page recommends Waydroid for desktop and Anbox Cloud for cloud. — [github.com/anbox](https://github.com/anbox); [anbox/anbox](https://github.com/anbox/anbox/)
  - Per the GitHub API: archived, last push 2024-02-06, 9,024 stars.
  - It is reportedly absent from the Debian 12/13 repositories. — [Kicksecure forum](https://forums.kicksecure.com/t/is-anonbox-no-longer-relevant-and-not-in-the-repository/1275)
- **Anbox Cloud use cases:** app/game streaming to browsers, TVs and thin clients; Android testing and CI/CD; cloud device labs; Android Automotive / IVI; OEM platform development. — [canonical.com/anbox-cloud](https://canonical.com/anbox-cloud)
- **Deployment options:** Appliance (evaluation/PoC), Charmed Anbox Cloud (production HA), and Managed Anbox Cloud.
  - Runs on AWS, GCP, Azure, Oracle, private cloud, bare metal and edge, on both Arm and x86.
  - Offers containerized Android plus virtualized Android (Cuttlefish workflows, custom kernels) and bring-your-own-image (AOSP, AAOS).
  - Offers GPU acceleration and low-latency streaming.
  - Named "trusted by" logos: Vodafone, Umony, Clario.
  
  Source: [canonical.com/anbox-cloud](https://canonical.com/anbox-cloud)
- **Licensing:** "Anbox Cloud is a paid offering." It needs Ubuntu Pro or Ubuntu Pro (apps only). "The Ubuntu Pro (infra only) subscription every Ubuntu user gets for free for personal use does NOT work." — [canonical/aar-operator](https://github.com/canonical/aar-operator)
- **Enabling it:** `sudo pro enable anbox-cloud`, then `sudo snap install anbox-cloud-appliance`. Supported on Ubuntu LTS 20.04 and later. — [Ubuntu Pro client docs](https://documentation.ubuntu.com/pro-client/en/docs/howtoguides/enable_anbox/)
- **Price levels:**
  - Ubuntu Pro list price is $500 per machine per year for servers. On public clouds it is metered hourly, at roughly 3–4.5% of compute cost. — [Ubuntu Pro pricing](https://ubuntu.com/pricing/ubuntu-pro)
  - AWS Marketplace Anbox Cloud Appliance listings are usage-metered. — [AWS Marketplace listing](https://aws.amazon.com/marketplace/pp/prodview-r5svkxfq63p6u)
- The snap was at version 1.29.x according to a snap tracker. — [snapupdates.popey.com](https://snapupdates.popey.com/snap/anbox-cloud-appliance)

### Inferences
- For an individual Mint developer, Anbox Cloud makes sense only if an employer already licenses it for streaming or device-farm CI.
- The snap installs on non-Ubuntu distros, but licensing still applies. Mint is not an officially supported host OS.

### Gaps
- I found no published per-instance Anbox Cloud price.
- I could not confirm the Android versions in the current provided images; the docs page returned HTTP 502. From memory (unverified), earlier releases shipped Android 12–14 images.
- The pricing page anbox-cloud.io/pricing redirects to canonical.com/anbox-cloud/pricing, which returned 404.

## redroid: Android versions, kernel modules (binderfs), Docker usage, scrcpy/ADB, testing/CI, maintenance

### Takeaway
redroid runs AOSP (Android 8.1–16, with 17 listed by a fork organization) as a privileged Docker container that shares the host kernel. It needs binder support on the host and should use memfd, because modern kernels like 7.0 have no ashmem. Access is through `adb connect localhost:5555` and scrcpy. It is popular (7.1k stars, about 1.78M Docker pulls) and good for headless CI and automation.

Maintenance is slowing. The newest image pushes on Docker Hub are 16.0.0 (2025-06-29) and 10.0.0 (2025-09-07), and the docs repo's last commit was 2026-05-17. Binderfs-only setups have known issues.

### Cited Findings
- **Image tags:**
  - `redroid/redroid:{16,15,14,13,12}.0.0-latest` and `_64only-latest` variants.
  - `11.0.0`, `10.0.0`, `9.0.0` and `8.1.0-latest`.
  
  Source: [redroid-doc README](https://github.com/remote-android/redroid-doc)
- Android 17 and 17 64-bit-only are listed by the Nredroid org README. — [github.com/Nredroid](https://github.com/Nredroid)
- **Quick start (Ubuntu 20.04 era):**
  - Load modules: `apt install linux-modules-extra-$(uname -r)`, then `modprobe binder_linux devices="binder,hwbinder,vndbinder"` and `modprobe ashmem_linux`.
  - Start the container: `docker run -itd --rm --privileged --pull always -v ~/data:/data -p 5555:5555 redroid/redroid:12.0.0_64only-latest`.
  - Connect: `adb connect localhost:5555`, then `scrcpy -s localhost:5555`.
  - The README warns: don't expose the ADB port on a public network.
  
  Source: [redroid-doc](https://github.com/remote-android/redroid-doc)
- **Boot options:**
  - `androidboot.use_memfd` replaces the deprecated ashmem. It defaults to false, with a plan to make it the default.
  - `androidboot.redroid_gpu_mode=auto|host|guest`. The default is `guest` (software rendering); `host` gives GPU acceleration.
  - Default FPS is 30 with GPU and 15 without.
  
  Source: [redroid-doc](https://github.com/remote-android/redroid-doc)
- **README limitations:**
  - WebRTC streaming is only planned.
  - ARM apps on x86 need a native bridge. The published images include `libndk_translation`.
  - GMS requires building custom images.
  - The container exits immediately if kernel modules are missing.
  
  Source: [redroid-doc](https://github.com/remote-android/redroid-doc)
- **Modern kernels:**
  - A community gist uses `androidboot.use_memfd=true` to run without ashmem. — [gist](https://gist.github.com/aznoisib/d6f7ead962e030528f6cf05f480f1205)
  - A redroid issue on a 6.8 kernel says binderfs is the intended path and that images (10–15) didn't handle a binderfs-only setup well. It also says the legacy `binder_linux` module can be unstable on some hardware. — [redroid-doc issue #859](https://github.com/remote-android/redroid-doc/issues/859)
  - Arch notes that kernels from 6.12.7 onward include binderfs. — [AUR binder_linux-dkms](https://aur.archlinux.org/packages/binder_linux-dkms?O=20)
- **Proxmox forum:** redroid is used for Android 8.1–16 in Docker, with GPU passthrough. — [Proxmox forum](https://forum.proxmox.com/threads/android%E2%80%91ipc-%E2%80%94-binder_linux-ashmem_linux-modules-for-android-containers.183708/)
- **Activity metrics (Docker Hub / GitHub API, 2026-11-05):**
  - `redroid/redroid` has 1,777,120 pulls. The repo was last updated 2025-09-07, the newest tags are `10.0.0-250907` and `16.0.0_64only-250627`, and `latest` dates from 2025-06-29.
  - redroid-doc has 7,101 stars and a last commit on 2026-05-17.

### Inferences
- **Recipe for the target kernel 7.0** (binder `=m`, binderfs `=m`, no ashmem), unverified on this host:
  1. `sudo modprobe binder_linux devices=binder,hwbinder,vndbinder`.
  2. Run the container with `--privileged` and `androidboot.use_memfd=true`, plus `androidboot.redroid_gpu_mode=host` for Intel iGPU acceleration.
- **CI fit:**
  - It needs no KVM and boots faster and lighter than an emulator.
  - Hosted runners (e.g. GitHub Actions ubuntu images) need the binder module loaded on the host kernel. That is possible on Ubuntu runners but not on all CI providers.
  - It needs a privileged container.
- **Not a full replacement for an emulator:**
  - It is a non-certified AOSP build with no Play Services by default.
  - It has no emulator console, so no sensor, GPS or telephony simulation.
  - Android Studio sees it only as a network ADB device.

### Gaps
- I found no official 2026 release notes for redroid images. The cadence of Docker Hub pushes suggests limited 2026 image activity, but private or alternative registries may exist.
- I could not verify that Android 17 images are published under `redroid/redroid`; only the Nredroid README lists them.

## Docker emulator images (budtmo/docker-android, Google's android-emulator-container-scripts, HQarroum/docker-android)

### Takeaway
These images wrap the official Android Emulator (QEMU/goldfish) and need `/dev/kvm`. budtmo/docker-android is the most popular and most active of them: 16.1k stars, about 1.87M pulls, releases v3.7–v3.9 between Sep and Oct 2026. Its normal image covers Android 9–14. Android 15–17, headless mode and some other features sit behind a GitHub-sponsor "pro" image. Google's scripts remain "experimental" but get light maintenance; they offer WebRTC streaming and are aimed at cloud and CI.

### Cited Findings
- **budtmo/docker-android, images and devices:**
  - The normal image ships Android 9.0 (API 28) to 14.0 (API 34). Pro-only images cover 15.0 (35), 16.0 (36) and 17.0 (37); headless variants exist for 9–17.
  - Device skins include Samsung Galaxy S6–S10, Nexus models and Pixel C.
  
  Source: [budtmo/docker-android](https://github.com/budtmo/docker-android)
- **budtmo/docker-android, features:**
  - noVNC on port 6080 (`WEB_VNC=true`).
  - Appium (plus Appium MCP); Espresso support.
  - `adb connect` from outside the container.
  - Log sharing through a web UI.
  - Genymotion Cloud integration and SMS simulation.
  - A beta MCP server and AI agent (Ollama/vLLM).
  
  Source: [budtmo/docker-android](https://github.com/budtmo/docker-android)
- **budtmo/docker-android, usage:**
  - Run: `docker run -d -p 6080:6080 -e EMULATOR_DEVICE="Samsung Galaxy S10" -e WEB_VNC=true --device /dev/kvm --name android-container budtmo/docker-android:emulator_11.0`.
  - Requires KVM (check with `kvm-ok`) and an Ubuntu host.
  - Emulator data is lost on restart unless `/home/androidusr` is mounted as a volume.
  
  Source: [budtmo/docker-android](https://github.com/budtmo/docker-android)
- **budtmo/docker-android, pro image:**
  - Available only to active GitHub sponsors, as `budtmo2/docker-android-pro`.
  - Pro adds: newer Android versions, proxy and language setup, root commands, headless mode, Selenium 4.x.
  - Listed as "soon" for pro: multiple emulators per container, Google Play Store, video recording.
  
  Source: [budtmo/docker-android](https://github.com/budtmo/docker-android)
- **budtmo/docker-android, activity:** GitHub tags v3.7.0-p1 (2026-09-18), v3.8.0-p1 (2026-10-09) and v3.9.0-p1 (2026-10-27). Last push 2026-11-05, 16,095 stars, 1,867,890 Docker Hub pulls (last updated 2026-10-27). Source: GitHub/Docker Hub API.
- **google/android-emulator-container-scripts:**
  - **Status:** 2,111 stars, not archived. Last commit 2026-10-06 ("Bump actions", a maintenance PR #419). The README still calls it "an experimental feature". Source: GitHub API and [repo](https://github.com/google/android-emulator-container-scripts).
  - **Tooling:** the `emu-docker` CLI builds images from chosen system-image and emulator versions. Pre-built images are in Google Artifact Registry. WebRTC streaming uses a gRPC gateway and a React front end. NVIDIA GPU is supported via the container toolkit. cloud-init is provided for VMs. — [repo](https://github.com/google/android-emulator-container-scripts)
  - **Requirements:** Linux with KVM (nested virtualization in cloud VMs reduces performance), Python ≥3.10, Docker and Compose. Tested on Debian/Ubuntu. Docker Desktop on macOS/Windows is not supported. — [repo](https://github.com/google/android-emulator-container-scripts)
- **HQarroum/docker-android:** 7,395 stars, MIT license, last push 2026-05-07. Source: GitHub API. I did not review its features.

### Inferences
- On the target host (KVM, Docker, i7-13620H), budtmo images should perform close to a native emulator, since it is the same emulator binary. GPU is probably software or SwiftShader unless configured otherwise.
- For CI, these images are the most "official-emulator-faithful" option. They need KVM on runners: GitHub-hosted Linux runners expose KVM; nested virtualization elsewhere is slower.

### Gaps
- I did not verify which emulator and system-image versions the budtmo v3.9.0 images bundle.
- I did not verify whether the noVNC/Appium features work in headless pro builds.

## Limitations vs the official emulator (Android Studio Device Manager)

### Takeaway
None of these runtimes plugs into Android Studio's Device Manager as a managed AVD. All of them appear only as network ADB devices (`adb connect`). The container runtimes (Waydroid, redroid) lack the emulator's extended controls and Google-certified Play images. The emulator-in-Docker images retain emulator internals but hide them behind a container and VNC/WebRTC.

### Cited Findings
- **Waydroid:** network ADB only. It is uncertified, so Google login needs manual registration. It has no ARM translation and depends on host GPU/Wayland. — [Waydroid ADB FAQ](https://docs.waydro.id/faq/using-adb-with-waydroid); [Mint discussion #1617](https://github.com/orgs/linuxmint/discussions/1617)
- **redroid:** GMS needs custom builds. It uses software rendering by default and needs privileged containers and host kernel modules. ARM apps run through `libndk_translation`. — [redroid-doc](https://github.com/remote-android/redroid-doc)
- **budtmo:** an Ubuntu host and KVM are required. Play Store is only "soon" for pro. Newer API levels (35–37) are pro-only. — [budtmo/docker-android](https://github.com/budtmo/docker-android)
- **Anbox Cloud:** a paid enterprise platform that is not aimed at desktop dev. — [aar-operator](https://github.com/canonical/aar-operator)

### Inferences
- **Recommended split for this host:**
  - The official emulator (KVM) for day-to-day Android Studio debugging, sensors, and Play-image testing.
  - Waydroid (via nested Weston) for quick near-native trials.
  - redroid or budtmo images for headless, multi-instance CI and automation.
- Compared with the emulator's extended controls, Waydroid and redroid lack: emulator console/telnet (`adb emu`), sensor/GPS/battery/telephony simulation, snapshots/Quick Boot, and foldable/Wear/TV/Automotive AVD profiles. This is inferred from architecture (no goldfish/QEMU layer); I found no single source that enumerates it.

### Gaps
- I found no r/androiddev threads or benchmarks (2025–2026) that quantitatively compare boot time, frame rate or memory across Waydroid, redroid and the emulator on Intel hosts.
- I found no official statement from Google on Device Manager support for remote or container devices beyond generic ADB/Wi-Fi pairing.
