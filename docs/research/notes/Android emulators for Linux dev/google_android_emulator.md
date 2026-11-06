# Official Google Android Emulator (SDK "emulator" package) on Linux x86_64: profile as of Nov 2026

Context: target host is Linux Mint 22.3 / Ubuntu 24.04 base, Intel i7-13620H (VT-x/EPT), 31 GB RAM, KVM. Research date: 2026-11-05. Sources are mostly official developer.android.com pages fetched on that date. Older material is dated inline.

## 1. Current version, release cadence, API levels, and system image types

### Takeaway
As of early Nov 2026 the current emulator is **37.2.13** (patch, Oct 28 2026), with **37.2.12** as the latest stable (Oct 1 2026). Google ships a stable release about every 1-2 months, with patches in between. A **next-generation engine based on upstream QEMU v11+** is in preview for API 37+ Phone AVDs, starting with Android Studio Rabbit 3 Canary 1. On x86_64 hosts, x86_64 images get KVM acceleration. ARM code runs only through per-process translation in Google APIs and Play images (API 30+). Full arm64 images are not accelerated on Intel.

### Cited Findings
- Recent releases: 37.2.13 Patch (Oct 28 2026); 37.2.12 Stable (Oct 1 2026); 37.1.11 Stable (Jul 30 2026); 36.6.11 Stable (Jun 2 2026); 36.5.11 Patch (Apr 23 2026); 36.5.10 Stable (Apr 2 2026); 36.4.10 Patch (Mar 9 2026); 36.4.9 Stable (Feb 10 2026); 36.4.3 Canary (Dec 8 2025); 36.3.10 Stable (Dec 4 2025). — [Emulator release notes](https://developer.android.com/studio/releases/emulator)
- 37.2.13 fixed a host memory leak in OpenGL ES emulation. Graphics contexts, surfaces, and framebuffers were not released when guest processes exited (Issue #568480829). — [Emulator release notes](https://developer.android.com/studio/releases/emulator)
- 37.1.11 (Jul 2026) added the Pixel 10a AVD and the new Vulkan extensions that API 37 system images require. `config.ini` now accepts unit suffixes for RAM and VM heap (e.g. `6G`). — [Emulator release notes](https://developer.android.com/studio/releases/emulator)
- 36.6.11 (Jun 2026): starting at API 37, Phone AVDs need at least 4 GB of guest RAM, and lower values are raised automatically. — [Emulator release notes](https://developer.android.com/studio/releases/emulator)
- 36.5.10 (Apr 2026) added a new networking stack. AVDs on the same host discover each other on a virtual network, and Wi-Fi Direct and Network Service Discovery work between emulators. It also added Pixel 10 AVDs. On Linux, all connected webcams are now detected and shown with friendly names. — [Emulator release notes](https://developer.android.com/studio/releases/emulator)
- Next-generation emulator (preview): "Starting with Android Studio Rabbit 3 Canary 1, eligible AVDs use a new engine based on upstream QEMU (v11+)". To use it, create a Phone AVD on API 37.0+ and launch it from Android Studio Canary. Existing AVDs stay on the classic engine. — [Emulator release notes](https://developer.android.com/studio/releases/emulator). Linaro's wiki describes the classic engine as a downstream QEMU fork whose last upstream merge was QEMU 2.12. — [Linaro: Run Android using QEMU](https://linaro.atlassian.net/wiki/spaces/QEMU/pages/29464068097/Run+Android+using+QEMU)
- Current stable Android Studio: **Rabbit 2 | 2026.2.2 Patch 1**. The Linux download is `android-studio-rabbit2-patch1-linux.tar.gz`. — [Android Studio download/install](https://developer.android.com/studio/install)
- API 37 (Android 17) images: since June 16 2026, unpatched API 37 AVDs may fail Google account login or get "not Play Protect certified" blocks. The fix is system image API 37 **revision 5+**. — [Emulator troubleshooting](https://developer.android.com/studio/run/emulator-troubleshooting)
- VM acceleration on an x86_64 host needs x86 or x86_64 system images (API 10+). "ARM- or MIPS-based system images on Intel or AMD CPUs, can't use the VM acceleration". — [Configure hardware acceleration](https://developer.android.com/studio/run/emulator-acceleration)
- ARM translation on x86 (Mar 2020): Android 11 x86 images translate ARM code only inside the app process that needs it, and the rest of the system runs natively. This is available only on Google APIs and Play Store images, and is intended for development and debugging, not commercial hosted services. — [Android Developers Blog, Mar 2020](https://android-developers.googleblog.com/2020/03/run-arm-apps-on-android-emulator.html?m=1); [XDA](https://www.xda-developers.com/android-11-x86-system-images-android-emulator-arm-abis/)
- System image path layout is `system-images/android-<api>/<variant>/<arch>/`, where variant is e.g. `google_apis` or `android-wear` and arch is e.g. `x86`. — [Emulator command line](https://developer.android.com/studio/run/emulator-commandline)
- Image source types for Gradle Managed Devices: `aosp`, `google`, `aosp-atd`, `google-atd`. ATD (Automated Test Device) images remove Google apps (Play Store, Chrome, Maps, and others), SystemUI, the Settings app, and many AOSP apps. They run headless with hardware rendering disabled, and screenshot tests that depend on hardware rendering are not supported. — [Gradle Managed Devices](https://developer.android.com/studio/test/gradle-managed-devices)
- Built-in device profiles cover phone, tablet, Wear OS, Android Automotive OS, and Android TV. — [Run apps on the Android Emulator](https://developer.android.com/studio/run/emulator). 36.4.3 Canary (Dec 2025) added experimental AI Glasses and XR Glasses emulators. — [Emulator release notes](https://developer.android.com/studio/releases/emulator)

### Inferences
- The major version roughly follows the newest API level (36.x for API 36, 37.x for API 37 in mid-2026). Expect about 6-8 stable releases a year plus patches.
- For an Intel i7-13620H the practical choice is **x86_64 Google APIs or Google Play images**. Do not use arm64-v8a images, which would run under slow full emulation.
- The next-gen QEMU engine is the main thing to watch in 2027. It is preview-only (Canary, API 37+), so a stable Linux workflow in late 2026 still runs on the classic engine.

### Gaps
- No official matrix of which API levels currently ship ATD images. The GMD doc's code comment still says "ATDs currently support only API level 30", which is probably stale (ATD images for later APIs exist in sdkmanager, but this was not verified here).
- Not verified whether ARM-translation support has been extended to newer API levels (31-37) or how it performs.
- The full list of API levels with current system images was not fetched. API 37 is the newest stable confirmed here.

## 2. Linux acceleration: KVM, GPU modes, Vulkan, known Linux issues

### Takeaway
On Linux the emulator uses **KVM**. You need VT-x and EPT (second-level address translation), access to `/dev/kvm`, and to be on bare metal rather than inside another VM. Since emulator **36.4.9 (Feb 2026)** the GPU options are reorganized. `-gpu auto`, `host`, and `software` are the main choices. **Lavapipe is the default software renderer**. `swiftshader_indirect`, `guest`, and `swangle_indirect` are **deprecated**, yet much CI tooling still uses them. Linux-specific bugs in 2026 release notes are few (webcam detection, an Intel GPU black screen).

### Cited Findings
- Linux uses the KVM package. For Intel the requirements are VT-x, Intel 64, and the XD bit enabled. Check with `emulator -accel-check` (expected output: "KVM (version 12) is installed and usable.") or with `kvm-ok` from `cpu-checker`. The docs give the Ubuntu install line `sudo apt-get install qemu-kvm libvirt-daemon-system libvirt-clients bridge-utils`. "Running KVM requires specific user permissions." — [Configure hardware acceleration](https://developer.android.com/studio/run/emulator-acceleration)
- Intel and AMD CPUs need second-level address translation (Intel EPT / AMD RVI). The accelerated emulator "can't run inside another VM, such as a VM hosted by VirtualBox, VMWare, or Docker", and conflicts with other virtualization software are possible but mostly seen on Windows. — [Configure hardware acceleration](https://developer.android.com/studio/run/emulator-acceleration)
- `-accel auto|on|off`: `off` (TCG software emulation) is "not supported and is very slow" and requires `ANDROID_I_WANT_MY_TCG=yes`. — [Emulator command line](https://developer.android.com/studio/run/emulator-commandline)
- Current `-gpu` modes are `auto` (recommended), `host`, `software`, `lavapipe` (Mesa), `swiftshader`, and `swangle`. Deprecated modes: `mesa` (25.3.0), `angle` (27.0.2), `angle_indirect` (34.2.14), and `guest`, `swiftshader_indirect`, `swangle_indirect` (all 36.4.9). — [Configure hardware acceleration](https://developer.android.com/studio/run/emulator-acceleration)
- 36.4.9 (Feb 2026) added Hardware and Software GPU options in Extended Controls. `-gpu software` picks the best software GLES and Vulkan backend, and Lavapipe became the default software renderer on all platforms. The release also updated the Vulkan loader, added SkiaVk support with graphics queue emulation, and added Vulkan composition (default on XR images, otherwise `-feature VulkanNativeSwapchain`). — [Emulator release notes](https://developer.android.com/studio/releases/emulator)
- 36.5.11 (Apr 2026) fixed a black screen with Hardware or Auto rendering on Intel GPUs (Issue #492228020). This matters for Intel iGPU laptops. — [Emulator release notes](https://developer.android.com/studio/releases/emulator)
- 36.1.9 (2025) fixed a crash on btrfs filesystems. The docs suggest software rendering for X11 launch problems. — [Emulator release notes](https://developer.android.com/studio/releases/emulator) (as summarized by the fetch tool; exact wording not verified)
- Vulkan caveats: snapshots of an emulator using Vulkan "is not supported", and the workaround is `-feature -Vulkan`. Chrome on API 30+ renders with Vulkan and may misrender, also fixed with `-feature -Vulkan`. If you get a "GPU Driver Warning - Falling Back To Software", force `hw.gpu.mode=host` in `config.ini` and `hardware-qemu.ini` (Issue 229642759). — [Emulator troubleshooting](https://developer.android.com/studio/run/emulator-troubleshooting)
- If the emulator is slow after an update on hybrid Intel plus discrete GPU systems, the docs suggest preferring the discrete GPU, updating Intel drivers, or trying `-gpu swiftshader`. — [Emulator troubleshooting](https://developer.android.com/studio/run/emulator-troubleshooting)
- `-noaudio`: "Some Linux and Windows computers have faulty audio drivers ... preventing the emulator from starting". — [Emulator command line](https://developer.android.com/studio/run/emulator-commandline)
- The emulator refuses to start with less than 5 GB of free disk. — [Emulator troubleshooting](https://developer.android.com/studio/run/emulator-troubleshooting)
- A Wayland/NVIDIA search for 2026 returned only Waydroid results, nothing specific to the Google emulator. — [Arch forum (Waydroid NVIDIA, Dec 2025)](https://bbs.archlinux.org/viewtopic.php?id=311370)

### Inferences
- An i7-13620H (Raptor Lake H) has VT-x with EPT. Once the user is in the `kvm` group (or `/dev/kvm` permissions allow it), KVM acceleration should just work. Linux Mint 22.x (Ubuntu 24.04 base) ships a recent enough kernel and Mesa.
- Make sure the emulator is at 36.5.11 or later to avoid the Intel iGPU black-screen bug. If the laptop also has an NVIDIA dGPU (common with the 13620H), compare `-gpu host` on each GPU (PRIME offload) against `-gpu auto`.
- The Linux release notes say nothing about the Wayland vs X11 status of the Qt emulator window. Running inside the Android Studio tool window (the default, `-qt-hide-window`) avoids most windowing-system dependence.

### Gaps
- I found no official 2026 statement on native Wayland support, as opposed to XWayland, for the standalone emulator window.
- No official list of NVIDIA proprietary driver versions known to cause problems on Linux.

## 3. Performance data: boot times, snapshots, RAM/CPU, benchmarks

### Takeaway
Official guidance says the first launch "might take a minute or so", and later launches are faster through Quick Boot snapshots. Official minimums are 16 GB host RAM and 16 GB disk, and API 37 Phone AVDs need at least 4 GB guest RAM. I found **no rigorous, current (2025-2026) benchmark** of the Google emulator against Genymotion or containers on Linux. The available comparisons are 2014-2018 anecdotes.

### Cited Findings
- Recommended specs for the emulator: 16 GB RAM, a 64-bit OS (Linux supported), and 16 GB disk space. "Running higher API levels and/or virtual devices with larger screens may require higher system specs". — [Run apps on the Android Emulator](https://developer.android.com/studio/run/emulator)
- Android Studio on Linux: any 64-bit distribution supporting Gnome, KDE, or Unity, glibc 2.31+, 8 GB RAM minimum with 16 GB+ recommended. The recommended GPU is one with 8 GB VRAM (e.g. NVIDIA GeForce 20 series+ or AMD RX 6600+). — [Android Studio install / system requirements](https://developer.android.com/studio/install) (figures from the search snippet; exact wording not re-fetched)
- First launch "might take a minute or so", and subsequent launches use a snapshot and "should launch faster". — [Run apps on the Android Emulator](https://developer.android.com/studio/run/emulator)
- Quick Boot flags: `-no-snapshot-load` (cold boot, save on exit), `-no-snapshot-save` (quick boot without saving), and `-no-snapshot` (disable both). Named snapshots are taken with `-snapshot name` or the console command `avd snapshot save`. `-no-boot-anim` "can significantly speed up the boot sequence". `-memory` accepts 1536-8192 MB. — [Emulator command line](https://developer.android.com/studio/run/emulator-commandline)
- Gradle Managed Devices use snapshots "to improve device startup time and memory usage and restore devices to a clean state between tests". ATDs are "optimized to reduce CPU and memory resources". — [Gradle Managed Devices](https://developer.android.com/studio/test/gradle-managed-devices)
- Host memory leak fixes in 2026: 36.4.9 (opening and closing activities, Issue #471008659) and 37.2.13 (GLES contexts not freed). — [Emulator release notes](https://developer.android.com/studio/releases/emulator)
- Genymotion vs Google emulator (dated anecdotes only):
  - 2014: Genymotion started in about 5 s versus at least 30 s for the emulator. — [Stack Overflow archive](https://kiwix.lema.org/stackoverflow.com_en_all_2023-11/questions/25424721/why-genymotion-emulator-is-a-lot-faster-than-android-emulator)
  - 2017: the emulator is faster when configured correctly. — [DEV Community](https://dev.to/sumitgpt/genymotion-vs-android-emulator-2i6o)
  - 2018: one emulator instance used 50-60% CPU and about 1200 MB RAM, versus 5-10% CPU and about 100 MB for Genymotion, per one user. — [Stack Overflow archive (2021 dump)](https://dev.library.kiwix.org/content/stackoverflow_en_nopic_2021-08/a/18684017)
- CI: android-emulator-runner states Ubuntu runners are "2-3 times faster" than macOS runners and cheaper. — [ReactiveCircus/android-emulator-runner](https://github.com/ReactiveCircus/android-emulator-runner)

### Inferences
- With 31 GB RAM, the user's host comfortably runs 2-3 concurrent AVDs at 4 GB guest RAM each alongside Android Studio and Gradle. This is an estimate, not a measured figure.
- On a recent KVM host, quick-boot resume is typically a few seconds and cold boot tens of seconds. No authoritative 2026 numbers were found, so measure locally (e.g. `adb wait-for-device` plus polling `sys.boot_completed`).

### Gaps
- No reliable 2025-2026 benchmark of boot time, frame rate, or memory for the Google emulator against Genymotion, Waydroid, or Redroid on Linux.
- No official quick-boot timing figure in current docs. Older Google marketing figures (e.g. 2017 "under 6 seconds") were not re-verified.

## 4. CLI / headless / CI use

### Takeaway
You can drive the emulator fully headless with `emulator -no-window`, with `sdkmanager` and `avdmanager` from cmdline-tools, or with Gradle Managed Devices. **As of 2026, Google's docs mark the standalone `emulator` CLI as "deprecated" in favor of the new agent-oriented Android CLI (`android emulator create|start|stop|list|remove`)**, though the `emulator` binary is still documented and shipped. On GitHub Actions Linux runners, `reactivecircus/android-emulator-runner@v2` is the de facto standard after a udev rule enables KVM.

### Cited Findings
- "The `emulator` tool is deprecated. Instead, use the Android CLI `android emulator` command". Syntax: `android emulator [create <profile-name>|start <device-name>|stop <device-serial-number>|list]`. — [Emulator command line](https://developer.android.com/studio/run/emulator-commandline)
- Android CLI `android emulator` subcommands are create (from hardware profiles such as `medium_phone` or `large_desktop`), list, remove, start ("returns when the emulator is fully started and ready to use"), and stop. Global options include `--sdk`. — [Android CLI: emulator](https://developer.android.com/tools/agents/android-cli/commands/emulator)
- `-no-window` "is useful when running the emulator on servers that have no display. You can access the emulator through `adb` or the console". Studio's embedded launch uses `-qt-hide-window -grpc-use-token -idle-grpc-timeout 300`. — [Emulator command line](https://developer.android.com/studio/run/emulator-commandline)
- Concurrency: console and adb ports run 5554-5682, "allowing for 64 concurrent virtual devices". `-report-console` exists for automation. — [Emulator command line](https://developer.android.com/studio/run/emulator-commandline)
- Gradle Managed Devices (API 27+): configured under `android.testOptions.managedDevices.localDevices` with `device`, `apiLevel`, and `systemImageSource`. Device groups are supported, and tests run with `./gradlew <device><Variant>AndroidTest`. Sharding uses `android.experimental.androidTest.numManagedDeviceShards=<n>`. Firebase Test Lab devices are also supported (plugin `com.google.firebase.testlab` 0.0.1-alpha05). — [Gradle Managed Devices](https://developer.android.com/studio/test/gradle-managed-devices)
- GMD on servers without hardware rendering, such as GitHub Actions, needs `-Pandroid.testoptions.manageddevices.emulator.gpu=swiftshader_indirect`. — [Gradle Managed Devices](https://developer.android.com/studio/test/gradle-managed-devices). That mode is listed as deprecated since emulator 36.4.9. — [Configure hardware acceleration](https://developer.android.com/studio/run/emulator-acceleration). The two official docs are inconsistent.
- android-emulator-runner details:
  - Uses `@v2`. Inputs: `api-level` (min 15), `target` (`default`, `google_apis`, `android-automotive`, and others), `arch` (`x86`, `x86_64`, `arm64-v8a`; default `x86`), `emulator-options` (default `-no-window -gpu swiftshader_indirect -no-snapshot -noaudio -no-boot-anim`), `disable-animations` (default true), and `emulator-build` (pin a build number).
  - Linux KVM enablement step:
    `echo 'KERNEL=="kvm", GROUP="kvm", MODE="0666", OPTIONS+="static_node=kvm"' | sudo tee /etc/udev/rules.d/99-kvm4all.rules; sudo udevadm control --reload-rules; sudo udevadm trigger --name-match=kvm`
  - Recommended caching: cache the AVD and adb with `actions/cache@v5`, generate a snapshot on cache miss, then run with `-no-snapshot-save`.
  - [ReactiveCircus/android-emulator-runner](https://github.com/ReactiveCircus/android-emulator-runner)
- Nested-VM restriction: the accelerated emulator cannot run inside VirtualBox, VMware, or Docker VMs. This explains why CI must use runners that expose `/dev/kvm`. — [Configure hardware acceleration](https://developer.android.com/studio/run/emulator-acceleration)

### Inferences
- A typical headless local flow on Mint/Ubuntu:
  1. `sdkmanager "emulator" "platform-tools" "system-images;android-36;google_apis;x86_64"`
  2. `avdmanager create avd -n test -k "<pkg>" -d pixel_8`
  3. `emulator -avd test -no-window -no-audio -no-boot-anim -gpu software &`
  4. `adb wait-for-device`

  sdkmanager and avdmanager syntax was not re-fetched in this session but is long-standing. Expect the docs to steer toward `android emulator ...` going forward.
- CI configs should migrate from `swiftshader_indirect` to `-gpu software`/`swiftshader`/`lavapipe` eventually. The deprecated flag still appears in official GMD docs and the runner's defaults, so it presumably still works.

### Gaps
- I did not determine whether `-gpu swiftshader_indirect` is still accepted with a warning, silently mapped to software, or rejected in 37.x.
- I did not verify android-emulator-runner's latest release tag or maintenance cadence in 2026, because the README fetch showed no release list.
- I did not research other CI systems (GitLab, CircleCI, Bitrise) for KVM availability.

## 5. Integration: adb, Android Studio, Compose previews, Espresso/UI tests, screenshots

### Takeaway
The emulator is bundled with Android Studio. By default it runs embedded in the IDE tool window and appears to `adb` as a normal device. Extended controls provide screenshots, sensors, camera, network shaping, and more. Espresso and instrumented tests run on it directly or through GMD and ATD. Compose preview screenshot testing is **host-side** (no emulator) and is moving to AGP test suites (AGP 9.5+).

### Cited Findings
- "The emulator comes with Android Studio, so you don't need to install it separately". It can simulate calls and SMS, location, network speeds, rotation and sensors, and Play Store access. Toolbar actions include a screenshot button. Multi-touch does not work in the tool window, so use a standalone window. — [Run apps on the Android Emulator](https://developer.android.com/studio/run/emulator); [Emulator troubleshooting](https://developer.android.com/studio/run/emulator-troubleshooting)
- `adb` treats the virtual device as a physical device. The user data partition (`userdata-qemu.img`) persists across restarts, and `-wipe-data` resets it. Camera modes include `webcamN`, `imagefile:`, `videofile:`, `image360:` (36.6.4+), and `environment`. `-tcpdump` captures packets. — [Emulator command line](https://developer.android.com/studio/run/emulator-commandline)
- gRPC: 37.2.12 fixed gRPC `streamScreenshot` with MMAP image transport (on macOS), confirming a gRPC screenshot streaming API exists. — [Emulator release notes](https://developer.android.com/studio/releases/emulator)
- ATD images are aimed at Espresso-style instrumented tests. The docs suggest Espresso-Intents to stub other apps that ATD removes, and hardware-rendering screenshot tests are unsupported on ATD. — [Gradle Managed Devices](https://developer.android.com/studio/test/gradle-managed-devices)
- Compose Preview Screenshot Testing "runs host-side tests" with tasks `updateDebugScreenshotTest` and `validateDebugScreenshotTest`. The standalone `com.android.compose.screenshot` plugin (0.0.1-alpha16) is "no longer maintained", and Google recommends AGP test suites with AGP 9.5+. IDE integration requires Studio Panda 1 Canary 4+ and AGP 9.0+. The Android CLI offers `android studio render-compose-preview`. — [Compose Preview Screenshot Testing](https://developer.android.com/studio/preview/compose-screenshot-testing)

### Inferences
- For Compose-heavy apps on Linux, much visual verification can skip the emulator (host-side previews and screenshot tests). Use the emulator for instrumented, Espresso, and end-to-end tests and interactive runs.

### Gaps
- I did not re-verify the current details of "Run Preview on device" (deploying a @Preview to an emulator) or Studio's Device Mirroring, Running Devices window, or Layout Inspector integration in 2026.

## 6. Licensing/cost, maintenance status, adoption

### Takeaway
The emulator is free and distributed through the SDK Manager under the Android SDK license terms. It is actively maintained in 2026 (releases every few weeks, new device profiles, a QEMU engine rewrite in preview) and is Google's default virtual device for Android development. I found no reliable figure for the share of developers who use it.

### Cited Findings
- The emulator ships with Android Studio and is updated through the SDK Manager's "Android Emulator" component. — [Run apps on the Android Emulator](https://developer.android.com/studio/run/emulator)
- The Android Studio download page carries the SDK license agreement. Gemini and Crashlytics services require Studio versions released within the previous 10 months, and the oldest supported is Panda 2. — [Android Studio install](https://developer.android.com/studio/install)
- Signs of active maintenance in 2026:
  - 8+ releases between Dec 2025 and Oct 2026.
  - New networking stack (Apr 2026).
  - Lavapipe and Vulkan rework (Feb 2026).
  - Next-gen upstream-QEMU engine preview.
  - Platform housekeeping: HAXM removed in 36.2.11 (Oct 2025), and AEHD on Windows sunsets Dec 31 2026 in favor of WHPX. Neither affects Linux.
  - [Emulator release notes](https://developer.android.com/studio/releases/emulator); [Configure hardware acceleration](https://developer.android.com/studio/run/emulator-acceleration)
- Google's framing: "In most cases, the emulator is the best option for your testing needs". — [Run apps on the Android Emulator](https://developer.android.com/studio/run/emulator)
- Adoption proxy: the Stack Overflow 2025 survey (49,000+ respondents) top IDEs were VS Code 75.9%, Visual Studio 29%, Notepad++ 27.4%, IntelliJ 27.1%, and Vim 24.3%. — [Stack Overflow 2025 survey](https://survey.stackoverflow.co/2025/); [IT Brief NZ](https://itbrief.co.nz/story/developers-adopt-ai-tools-but-trust-issues-persist-survey-finds). One blogger reports Android Studio at about 15% of all respondents. This is unverified against the official data. — [DEV Community](https://dev.to/davidshq/thoughts-on-stackoverflow-2025-developer-survey-part-1-3d5o)

### Inferences
- Because the emulator is bundled with Android Studio, its user base is approximately Android Studio's (likely the majority of native Android developers), but no source quantifies emulator use specifically.
- Long-term risk on Linux is low. Linux and KVM are first-class, and Windows/macOS hypervisor changes do not affect Linux.

### Gaps
- I did not fetch the exact text of the SDK license as it applies to the emulator binary. The emulator source lives in AOSP (`platform/external/qemu`, see [android.googlesource.com DEVELOPMENT.md](https://android.googlesource.com/platform/external/qemu/+/emu-master-dev/android/docs/DEVELOPMENT.md)), but I did not verify its open-source license. Google Play images carry additional Google terms (not verified).
- No developer survey reports the share of developers using the emulator versus physical devices or third-party emulators. The Stack Overflow survey has no emulator question, and I did not search JetBrains' State of Developer Ecosystem 2025/2026.
