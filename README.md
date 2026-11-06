# android-on-debian

A minimal, modular starting framework for building Android apps on
Debian-family Linux (Debian, Ubuntu, Linux Mint). It is CLI-first and works
with [Claude Code](https://claude.com/claude-code) as the primary development agent.

One command (`./adx up`) provisions a reproducible toolchain without root.
A second (`./adx app-template new`) scaffolds a Jetpack Compose app. In that
app, screens render to PNG on the JVM in seconds with **Roborazzi**, and run on
a headless **official Android Emulator** driven by **mobile-mcp** when you need
live behaviour.

## Purpose

Android development on Linux usually starts with a 1.6 GB IDE and a GUI setup
wizard. This project replaces that with a small, scriptable, versioned
framework. Every tool, version and setting is declared in plain text, so the
environment can be rebuilt, reviewed, retrofitted for a new use case and
extended module by module.

## Goals

1. **Reproducible:** pinned versions in `config/defaults.env`, checksummed
   downloads, idempotent installs. `./adx up` on a clean machine gives the same
   result every time.
2. **No root for the core toolchain:** JDK, SDK, Gradle and emulator all live
   under `$HOME`. The only sudo steps (32-bit libraries, the `kvm` group) are
   optional and documented.
3. **Modular:** each capability is a self-contained module with a fixed
   contract. Adding a feature means adding a folder, not editing a monolith.
4. **Fast UI loop without an emulator:** Compose screens render to PNG through
   Roborazzi + Robolectric, so an agent can see its UI changes in seconds.
5. **Real-device fidelity when it matters:** the official emulator (KVM,
   headless) plus mobile-mcp for tapping, typing, screenshots and the
   accessibility tree.
6. **Agent-ready by default:** every app ships with `CLAUDE.md`, a scoped
   permission allowlist and pinned design and code-quality skills.
7. **Minimal:** nothing is included that the workflow doesn't use. Android
   Studio stays installed but dormant until it's needed.

## Quick start

```bash
git clone https://github.com/MensuraMedia/android-on-debian.git
cd android-on-debian
./adx up                 # provision everything in profiles/default.profile
exec bash                # reload ~/.bashrc (adds ANDROID_HOME, PATH, adx)

adx app-template new HelloApp com.example.helloapp
cd ~/projects/android-apps/HelloApp
./gradlew recordRoborazziDebug          # PNGs in app/build/outputs/roborazzi/

adx emulator start                      # headless API 37 emulator (KVM)
./gradlew installDebug
adb shell am start -n com.example.helloapp/.MainActivity
adx emulator stop
```

Restart Claude Code once after `./adx up` so it loads the mobile-mcp server.

## Requirements

| Item | Requirement | Notes |
| --- | --- | --- |
| OS | Debian 12+, Ubuntu 22.04+, Linux Mint 21+ (x86_64) | Developed on Linux Mint 22.3 (Ubuntu 24.04 base), kernel 7.0 |
| CPU | x86_64 with VT-x/AMD-V | Needed for emulator acceleration |
| RAM | 16 GB recommended | Emulator + Gradle daemon |
| Disk | ~15 GB | SDK, system image, Gradle caches |
| Base tools | `bash curl wget unzip git jq perl` | Usually preinstalled |
| Node.js | 18+ with `npx` | Only for mobile-mcp |
| Claude Code | `claude` CLI | For the MCP server, skills and permissions |
| KVM | read/write access to `/dev/kvm` | `sudo usermod -aG kvm $USER`, then log out and back in |

## What gets installed

| Module | Installs | Location |
| --- | --- | --- |
| `jdk` | Eclipse Temurin 21 JDK (SHA-256 verified) | `~/.local/opt/jdk-21` |
| `shell-env` | Managed `~/.bashrc` block: `ANDROID_HOME`, `JAVA_HOME`, PATH | `~/.bashrc` → `lib/shell-env.sh` |
| `sdk` | cmdline-tools, platform-tools, platform 37.0, build-tools 37.0.0 | `~/Android/Sdk` |
| `emulator` | Emulator + API 37 Google Play x86_64 image + AVD `adx_api37` | `~/Android/Sdk`, `~/.android/avd` |
| `gradle` | Gradle 9.8.2, used only to generate app wrappers | `~/.local/opt/gradle` |
| `mobile-mcp` | `@mobilenext/mobile-mcp` registered in Claude Code | `~/.claude.json` (user scope) |
| `claude-permissions` | Permission allowlist template | `<project>/.claude/settings.json` |
| `skills` | Pinned design and code skills (below) | `<app>/.claude/skills/` |
| `app-template` | Compose + Roborazzi app scaffolder | `~/projects/android-apps/<Name>` |
| `studio` | Nothing; reports on the dormant Android Studio install | `~/.local/opt/android-studio` |

## Using `adx`

```text
adx list                         modules, descriptions and status
adx doctor                       detailed status of every module
adx up [profile]                 install every module in a profile (default: default)
adx <module> [action] [args...]  run one module action (default: status)
adx <module> help                list a module's actions
```

Common actions:

| Command | Effect |
| --- | --- |
| `adx app-template new <Name> <package> [dir]` | Scaffold an app (permissions, skills, wrapper, git init) |
| `adx emulator start [--window]` / `stop` | Boot or stop the AVD (headless unless `--window`) |
| `adx emulator screenshot [file]` | Save the emulator screen to a PNG |
| `adx emulator accel` | Check KVM acceleration |
| `adx sdk add "platforms;android-36"` | Install extra SDK packages |
| `adx skills apply <dir>` | Vendor or refresh pinned skills in a project |
| `adx claude-permissions apply <dir>` | Merge the permission allowlist into a project |
| `adx shell-env show` | Print the environment new shells get |
| `adx studio launch` | Open Android Studio when it's needed |

Profiles in `profiles/` choose which modules `adx up` installs.
`render-only` skips the emulator and MCP server, for CI or low-resource machines.

## Development workflow

1. **Render loop (seconds, no device).** Claude edits composables, runs
   `./gradlew recordRoborazziDebug` and reads the PNGs. Each screen and state
   is a `@Test` calling `captureRoboImage(...)`.
   `./gradlew verifyRoborazziDebug` then guards against visual regressions.
2. **Live check (when behaviour matters).** `adx emulator start`, then
   `./gradlew installDebug`. Claude uses the mobile-mcp tools: take a
   screenshot, list the elements on screen, tap, type, read the logs.
3. **Release checks.** Before shipping, test on a physical phone (scrcpy) and
   on a device matrix such as Firebase Test Lab's free tier. Neither is
   automated here yet.

See [docs/architecture.md](docs/architecture.md) for the diagram and module contract.

## UI/UX and code-quality resources

Every scaffolded app gets these Claude Code skills in `.claude/skills/`,
pinned to exact commits in [`config/skills.manifest`](config/skills.manifest).
Only the skills themselves are installed. The upstream plugins' automatic
hooks are deliberately left out, so nothing runs unless a skill is invoked.
Details are in [docs/skills.md](docs/skills.md).

| Resource | Skills | Role in this framework | Source |
| --- | --- | --- | --- |
| **Hallmark** (also serves as the "AntiSlop" design skill) | `hallmark` | UI/UX quality: build, audit, redesign and study modes against anti-AI-slop gates. Written for web UI; apply its principles to Compose. | [Nutlope/hallmark](https://github.com/Nutlope/hallmark) (MIT) |
| **SwiftUI Agent Skill** | `swiftui-pro` | SwiftUI best practices. iOS-specific: a reference for cross-platform design parity and a future iOS template, not for Compose code. | [twostraws/SwiftUI-Agent-Skill](https://github.com/twostraws/SwiftUI-Agent-Skill) (MIT) |
| **autoresearch** | `autoresearch` | Advanced features: autonomous modify → verify → keep/discard loops toward a measurable goal. | [uditgoenka/autoresearch](https://github.com/uditgoenka/autoresearch) (MIT) |
| **ponytail** | `ponytail`, `ponytail-audit`, `-debt`, `-gain`, `-help`, `-review` | Code control: simplest-working-solution discipline, audits, debt tracking and reviews. | [dietrichgebert/ponytail](https://github.com/dietrichgebert/ponytail) (MIT) |

To change the set, edit `config/skills.manifest`, then run
`adx skills apply <app-dir>`.

## Repository layout

```text
adx                      entry point (dispatcher)
lib/common.sh            shared helpers: logging, config, downloads, templating
lib/shell-env.sh         sourced by ~/.bashrc; exports env from config
config/defaults.env      all versions and paths (committed)
config/local.env         machine overrides (git-ignored; see local.env.example)
config/skills.manifest   pinned third-party skills
profiles/*.profile       ordered module lists for `adx up`
modules/<name>/module.sh one capability each (see docs/architecture.md)
templates/               app templates + Claude permission template
docs/                    architecture, skills, Android Studio, troubleshooting, research
CHANGELOG.md             notable changes (Keep a Changelog format)
HANDOFF.md               current state, open items, next steps
```

## Extending

- **New module:** create `modules/<name>/module.sh` with `MOD_DESC`,
  `mod_status` and `mod_install`. Any other `mod_<action>` function becomes
  `adx <name> <action>` automatically. Add it to a profile if `adx up` should
  install it.
- **New app template:** add `templates/<name>/` using `@@VAR@@` placeholders
  (any config variable, plus `APP_NAME`, `APP_PACKAGE`, `APP_PACKAGE_PATH`),
  then run `adx app-template new <Name> <pkg> <dir> <name>`.
- **New profile:** copy a file in `profiles/` and list the modules you want.
- **Version bump:** edit `config/defaults.env`, or override per machine in
  `config/local.env`.

## Research

The choice of emulator and approach is backed by a sourced comparison of
10 options: the official emulator, Genymotion, Waydroid, redroid,
docker-android, Anbox, Android-x86/BlissOS, cloud device farms and physical
devices. See
[docs/research/reports/Android emulators for Linux dev.md](docs/research/reports/Android%20emulators%20for%20Linux%20dev.md).

## License

[MIT](LICENSE). Each vendored skill keeps its own MIT license file inside its folder.
