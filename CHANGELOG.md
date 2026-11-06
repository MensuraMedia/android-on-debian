# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
  uninstalling, clearing data, `rm`, `push`, `root` and cloud-device tools.
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
