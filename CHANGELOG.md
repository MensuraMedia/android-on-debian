# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.2.0] - 2026-11-06

### Added
- Framework log: every `adx` run (START/END, exit code, cwd) and every
  message, timestamped and tagged by module, in `$ADX_LOG_DIR/adx.log`
  (`~/.local/state/adx/`), rotated by size (`ADX_LOG_MAX_KB`, `ADX_LOG_KEEP`).
  Emulator logs keep the previous runs (`emulator-<avd>.log.1…5`).
- `adx logs [adx|emulator|dir] [-n N]` and `adx env` (machine, toolchain and
  path report).
- `system` module (first in every profile): preflight of apt packages, CPU
  virtualisation, `/dev/kvm` and `kvm` group (incl. "re-login needed"),
  architecture, RAM, disk; prints the one `sudo` command needed.
- `node` module: Node.js LTS at user level (nodejs.org, SHA-256 from
  SHASUMS256.txt), before `mobile-mcp` in the default profile.
- `claude-rules` module: installs the global Claude Code rules
  (`config/claude-global-rules.md`: changelog, handoff, logging, Android notes;
  paths rendered per machine) as a marked block in `~/.claude/CLAUDE.md`, and
  the `adversarial-reviewer` agent into `~/.claude/agents/` (edited copies
  kept as `.bak`). In the default profile only (not `render-only`).
- `adx emulator autoconfig`: detects hybrid Intel/AMD + NVIDIA GPUs (PCI
  vendor IDs) and writes `ADX_EMU_ENV` (NVIDIA offload) and
  `ADX_EMU_GPU=software` to `config/local.env` unless already set; run by
  `emulator install`.
- `adx emulator desktop [remove]`: menu entry Programming → Android Emulator
  with Stop and Start-with-wiped-data actions; `start`/`stop --notify` for
  desktop notifications.
- `adx emulator status`: process, adb state, GPU mode, hybrid-GPU and crash
  hints, menu entry.
- `ADX_EMU_PORT` (5554): the emulator's console port is pinned and all adb
  calls target `emulator-<port>`, so an attached phone no longer breaks boot.
- `ADX_CMDLINE_TOOLS_SHA256` and `ADX_GRADLE_SHA256`: cmdline-tools and app
  Gradle wrappers (`distributionSha256Sum`) are now checksum-verified.
- `adx doctor [profile]`: only modules in the profile affect the exit code.
- Docs: `docs/environment.md` (full environment reference), `docs/dependencies.md`
  (dependency catalogue), `docs/install-issues.md` (every installation issue
  with cause, fix and status); modularity + ponytail rules in
  `docs/architecture.md`.
- App template: `CHANGELOG.md`, `HANDOFF.md`, `AppLog.kt` (debug/info lines only
  in debug builds, `buildConfig = true`), design-and-review section in
  `CLAUDE.md`, `templates/claude/agents/adversarial-reviewer.md`; screenshot
  baselines in `app/src/test/screenshots/` (committed) instead of `build/`.
- App template (`compose-roborazzi`) now ships `CHANGELOG.md`, `HANDOFF.md`
  and `AppLog.kt` (logging tagged `<AppName>/<area>` with `AppLog.timed`), and
  its `CLAUDE.md` makes logging, changelog and handoff upkeep mandatory.
- `adx emulator start --wipe`: factory-reset the AVD's user data on boot.
- `ADX_EMU_ENV` (`config/defaults.env`): extra environment for the emulator
  process only, e.g. NVIDIA PRIME offload on hybrid Intel + NVIDIA laptops.
- Troubleshooting entries: first-boot segfault on hybrid GPUs, `unauthorized`
  devices, user-level Node.js install, non-interactive shells, KVM via logind
  ACL only, and tap coordinates.
- `docs/examples/meridian-weather/`: first example app (Compose weather app built
  with the framework) with six Roborazzi mockups.

### Changed
- Logs moved from `~/Downloads/adx/` to `ADX_LOG_DIR` (`~/.local/state/adx/`).
- Software rendering is `ADX_EMU_GPU=software` (Lavapipe);
  `swiftshader_indirect` is deprecated since emulator 36.4.9.
- `adx up` re-checks each module after installing it and fails if it is still
  not healthy; profile lines may carry `#` comments.
- `adx` puts `~/.local/bin` on PATH for its modules (fresh Node installs).
- `mobile-mcp` status checks the pinned version; install re-registers.
- `adx emulator start` rejects unknown options.

### Fixed
- `adx emulator start` fails within seconds if the emulator process dies
  during boot (it used to wait the full `ADX_EMU_BOOT_TIMEOUT`), and stops
  with a `--wipe` hint when the device stays `unauthorized` for 60 s.
- Hybrid-GPU detection matched "ati" inside "compatible"/"Corporation", so any
  NVIDIA machine counted as hybrid; now uses PCI vendor IDs from sysfs.
- A corrupt cached download failed the checksum on every rerun; it is now
  deleted with a "rerun" message.
- A half-fetched skills cache or interrupted Node extract is repaired on rerun.
- `adx emulator start` kills a half-booted emulator when it fails, restarts the
  adb server when adb lost a running emulator, and validates `ADX_EMU_ENV`.
- `adx_local_set` recognises `export KEY=` / indented keys and quotes values.
- `claude-permissions apply` fails loudly on invalid JSON instead of logging success.
- Log rotation can no longer abort a command; `adx help` no longer prints
  `set -euo pipefail`; desktop `Exec` paths are quoted.
- CI: add `.shellcheckrc` (bash dialect; SC2034 disabled for module contract
  variables read by the dispatcher).

## [0.1.0] - 2026-11-05

### Added
- `adx` entry point with `list`, `doctor`, `up [profile]` and
  `<module> <action>` dispatch.
- `lib/common.sh`: logging, config loading, resumable downloads, SHA-256
  checks, managed file blocks and `@@VAR@@` template rendering.
- `lib/shell-env.sh`: exports `ANDROID_HOME`, `JAVA_HOME` and SDK tool paths
  from config, sourced through a managed `~/.bashrc` block.
- `config/defaults.env` with pinned versions, plus a git-ignored
  `config/local.env` for overrides.
- Modules:
  - `jdk`: Temurin 21, user-level, checksum-verified.
  - `shell-env`.
  - `sdk`: cmdline-tools 16111833, platform 37.0, build-tools 37.0.0.
  - `emulator`: AVD `adx_api37` on the API 37 Google Play image; headless
    start/stop, screenshot and accel check.
  - `gradle`: 9.8.2 bootstrap, used for wrappers.
  - `mobile-mcp`: 1.0.16, user-scope registration.
  - `claude-permissions`.
  - `skills`.
  - `app-template`.
  - `studio`: dormant.
- Profiles: `default` and `render-only`.
- `compose-roborazzi` app template: AGP 9.5.1, Kotlin 2.4.21, Compose BOM
  2026.09.00, Roborazzi 1.77.0, Robolectric 4.17, light and dark screenshot
  tests, `CLAUDE.md`.
- Claude Code permission template. It auto-allows Gradle, read-only and
  interactive adb, and non-destructive mobile-mcp tools. It asks before
  uninstalling, clearing data, on-device `adb shell rm` / `adb push`, `root` and cloud-device tools.
- Pinned skills (vendored per app, no plugin hooks): hallmark, swiftui-pro,
  autoresearch, and the ponytail set of six.
- Docs:
  - README, architecture, skills, Android Studio, troubleshooting.
  - Emulator research report and notes.
  - HANDOFF.md, CONTRIBUTING.md and a shellcheck CI workflow.

### Verified
- `./adx up`: all modules OK on Linux Mint 22.3, kernel 7.0, i7-13620H.
- Scaffolded `HelloAdx`. `recordRoborazziDebug` rendered light and dark
  PNGs in about 11 s once dependencies were cached.
- `assembleDebug`, then `installDebug` on the headless emulator: the app
  launches, and the emulator screenshot matches the Roborazzi render.
- mobile-mcp 1.0.16 stdio handshake: 35 tools listed, and it sees
  `adx_api37` online.

### Fixed during bring-up
- System OpenJDK 21 was a JRE with no `javac`. The `jdk` module now installs
  a full Temurin JDK.
- Added the Robolectric `--add-opens` JVM flag for SDK 36 under JDK 21.
- `adx_render_tree` no longer aborts under `set -e` on non-executable files.
- The emulator boot timer now counts from launch.
