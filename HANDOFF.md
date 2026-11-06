# Handoff — updated 2026-11-06

The state of the project at the end of the first session, written for
whoever (human or agent) picks it up next.

## State

| Area | Status |
| --- | --- |
| Framework (`adx`, 10 modules, 2 profiles) | Done; `./adx list` shows all OK on this machine |
| Render loop (Roborazzi) | Verified end to end with `~/projects/android-apps/HelloAdx` |
| Live loop (emulator + adb) | Verified: headless boot, install, launch, screenshot |
| mobile-mcp | Registered (user scope), stdio handshake verified; **not yet used from a live Claude session** (needs a Claude Code restart) |
| Skills | 9 skills pinned and vendored into HelloAdx; plugin hooks deliberately excluded |
| Android Studio | Installed at `~/.local/opt/android-studio`, dormant by request |
| Repo | MIT licensed, pushed to `MensuraMedia/android-on-debian` (private) |

## Machine-specific facts

- Linux Mint 22.3 (Ubuntu 24.04 base), Cinnamon on X11, kernel 7.0.0-38,
  i7-13620H, Intel UHD graphics only, 31 GiB RAM.
- `/dev/kvm` is reachable through a logind ACL, but the user is **not in the
  `kvm` group**. Run `sudo usermod -aG kvm $USER` for a permanent fix.
- sudo needs a password, so all installs avoid root.
- The project folder was renamed from `android-studio` to
  `android-on-debian`. A compatibility symlink
  `~/projects/android-studio → android-on-debian` was left for the running
  session; delete it once Claude Code has been restarted from the new path.

## Second install: hybrid-GPU laptop (2026-11-05)

`./adx up` was run on a clean second machine to check that the framework
reproduces. It did, after the fixes below.

- **Machine:** Linux Mint 22.2, Cinnamon on X11, kernel 7.0.0-38, 8 cores,
  23 GiB RAM, ASUS laptop with Intel Iris Xe (display) + NVIDIA RTX 3050 Ti
  (driver 580, PRIME on-demand).
- **Result:** all 9 profile modules OK (`studio` is manual and absent). Roborazzi
  render verified (`HomeScreen_light/dark.png`). Live loop verified: headless
  boot ~6 s warm / ~24 s first boot, `installDebug`, launch, tap → counter 2,
  screenshot.
- **Node.js was missing.** Node 26.13.0 LTS was installed at user level
  (`~/.local/opt/node`, SHA-256 checked, symlinked into `~/.local/bin`). Without
  it, `adx up` stops at `mobile-mcp`.
- **First-boot segfault.** On this hybrid-GPU laptop, every first cold boot of
  an AVD crashed (4/4) in the emulator's bundled `libtcmalloc_minimal.so.4`
  (glibc `_dl_update_slotinfo`). It booted 4/4 with NVIDIA PRIME offload. Fixed
  through the new `ADX_EMU_ENV` in this machine's `config/local.env`. The crash
  also left the AVD `unauthorized` to adb until a `--wipe`.
- **`adx emulator start` hardening.** It now fails fast when the emulator dies,
  detects `unauthorized`, and takes `--wipe`.
- **KVM.** Access currently comes only from the logind seat ACL; the user is
  not yet in the `kvm` group (needs sudo plus a full logout).

## Session 2026-11-06 (hybrid-GPU laptop)

- **Live mobile-mcp verified.** From Claude Code in HelloAdx: list devices,
  list elements, tap "Press me", counter 0 → 1, screenshot. Next step 1 below
  is done apart from the re-login for the `kvm` group (user is in the group in
  `/etc/group`, but the current session still uses the logind ACL).
- **HelloAdx became a weather app** with live Open-Meteo data, device location
  and `AppLog` logging. See `~/projects/android-apps/HelloAdx/HANDOFF.md`.
  Pushed to `MensuraMedia/HelloAdx` (private).
- **Emulator crashed twice with `-gpu host`** (windowed and headless) a few
  minutes after boot: gfxstream `context was preempted` / `null ctx` errors,
  then the process exited and adb showed no device. This machine's
  `config/local.env` now sets `ADX_EMU_GPU="swiftshader_indirect"`; stable
  since. Worth a troubleshooting entry, and maybe making `doctor` suggest it
  on PRIME hybrids alongside `ADX_EMU_ENV`.
- **mobile-mcp `mobile_set_location` does not move the emulator GPS.** It
  injects a `mobilecli` agent into the app process, and the app's location
  requests then hung. Use `adb emu geo fix <lon> <lat>`.
- **Desktop menu entry** (outside the repo): `~/.local/share/applications/adx-emulator.desktop`
  (Programming → Android Emulator; right-click actions Stop and Start with
  Wiped Data) calling `~/.local/bin/adx-emulator-menu start|stop|wipe`. Could
  become an `adx emulator desktop-entry` action.
- **Template** now includes CHANGELOG/HANDOFF/AppLog; verified by scaffolding
  a throwaway app and compiling it.
- **Global rules** in `~/.claude/CLAUDE.md`: every project keeps
  `CHANGELOG.md` and `HANDOFF.md` current and has structured logging.

## Next steps

1. **Resume here (hybrid-GPU machine).** The session ended so the user could
   run `sudo usermod -aG kvm $USER` and log out of the desktop session. First
   check `id | grep kvm` and `./adx doctor`. Then, in Claude Code started from
   `~/projects/android-apps/HelloAdx`, confirm `/mcp` lists mobile-mcp and do a
   live mobile-mcp session: `adx emulator start`, take a screenshot, list
   elements, tap "Press me", and check the counter reads 1. The session's
   changes to this repo are not committed yet (see `git status`).
2. Optional sudo items, all documented in the Studio install doc:
   - `kvm` group membership.
   - The 32-bit libraries (`libncurses6:i386`, `lib32z1`), only needed by
     Android Studio.
   - `fs.inotify.max_user_watches=524288`.
3. Migrate the `sdk` and `emulator` modules from the deprecated
   `sdkmanager`/`avdmanager` to the new `android` CLI (`android sdk`,
   `android emulator`) once its syntax has been checked.
4. Add a Compose Preview scanner (ComposablePreviewScanner +
   `roborazzi-compose-preview-scanner-support`) so every `@Preview` renders
   without a hand-written test.
5. Make `adx emulator accel` (or `doctor`) detect PRIME hybrid graphics
   (`prime-select query` = `on-demand`, or an NVIDIA plus an Intel/AMD VGA in
   `lspci`) and suggest `ADX_EMU_ENV`. Consider reporting the tcmalloc crash
   upstream to the Android Emulator issue tracker (emulator 37.2.13.0).
6. Add an optional `physical-device` module (udev rules, scrcpy 5.x from
   GitHub) and an `ftl` module (Firebase Test Lab via `gcloud`).
7. Consider a `ci` profile plus a GitHub Actions workflow template for apps
   (`render-only` with `verifyRoborazziDebug`).

## Open questions

- Should `ADX_APPS_DIR` (`~/projects/android-apps`) live inside this repo
  instead? It's kept separate for now, so each app has its own git history.
- The SwiftUI skill is iOS-only. Keep it vendored into Android apps, or move
  it to a future iOS template?
