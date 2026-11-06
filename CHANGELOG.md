# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

Status of everything below and of planned work: [ROADMAP.md](ROADMAP.md).

### Upgrading from 0.2.0
- `./adx up` now installs `rust` (rustup toolchain 1.95.0, a few hundred MB;
  an existing rustup is reused) and `devtools` (ShellCheck, cargo-deny), fetches
  the `diagram-design` skill and installs the `deploy-reviewer` agent: it needs
  the network. A failed download no longer stops the run: the rest of the
  profile continues and `adx up` ends with the list of incomplete modules.
- Rust links with the system C toolchain: `system` preflight now requires
  `build-essential` (`cc`).
- `adx up` now also builds `aodm` (`devices` module): `cargo build --release
  --locked` downloads crates from crates.io on first run (~65 MB cache in
  `~/.cargo/registry`), writes ~90 MB to `tools/aodm/target/` and takes about a
  minute. If `~/.local/bin/aodm` already exists and wasn't installed by adx, it
  is left alone and `devices` reports it (move it away, or `ADX_FORCE=1`).
- Rolling back to v0.2.0: `./adx devices remove` (deletes only adx's
  `~/.local/bin/aodm`; v0.2.0 would never notice it), `./adx devtools hook remove` (the hook is a no-op on
  v0.2.0 anyway), `rm -rf tools/aodm/target`, `git checkout v0.2.0`. Left
  behind, remove by hand if wanted: Rust toolchain 1.95.0
  (`~/.cargo/bin/rustup toolchain uninstall 1.95.0`; never delete `~/.cargo`
  or `~/.rustup` if they predate adx), `~/.local/opt/{shellcheck,cargo-deny}-*`
  and their `~/.local/bin` links, `~/.claude/agents/deploy-reviewer.md`,
  `~/.diagram-design/profiles/aod-flat-dark.md`.

### Planned for 0.3.0 (approved, not yet built)
- `aodm`: lifecycle for one, a group or all images (ports, budget, health),
  full-copy saves, clone/rename/delete, groups, bulk settings profiles, bulk
  app deploy, broadcast/capture, terminal UI.
- Build templates with a built-in `barebones` default; identity policies
  (`private`/`standard`/`fixed`); per-image security locks; spin-offs into
  standalone phones (`aod-phone`) with a calls/SMS switchboard.
- `adx release`, `adx upgrade` with migrations and rollback, `adx new-module`.
- Remaining technical document sections and diagrams.

### Added
- aodm P2 (inventory and version card): `aodm list [--group G]` (state,
  port, API, ABI, tag, system image revision, device, RAM, size, groups),
  `aodm card` (two even columns: aodm/adx versions, host, toolchain,
  emulation; then the image table); `aod-core` modules `avd` (scan, broken
  images, name rules), `proc` (emulators from `/proc`, never matching shells),
  `registry` (`images.toml`, unknown keys kept, atomic save), `card`.
- `devices` module (default profile): builds `aodm` with the pinned toolchain
  and installs it to `~/.local/bin/aodm`; status reports stale builds;
  `adx devices run <aodm args>`; `remove` deletes only the binary.
- `adx check [--staged|--full]` (`lib/check.sh`): one quality gate for local,
  pre-commit and CI: shell (`bash -n`, ShellCheck), Rust (fmt, clippy, test,
  cargo deny), docs (relative links, documented commands exist, stale terms),
  process (CHANGELOG with code changes, HANDOFF current), security (secrets in
  the diff incl. new files, masked; no pipe-to-shell; every download
  checksum-verified; permission/rules/agent changes flagged for owner approval).
- `devtools` module: ShellCheck 0.11.0 and cargo-deny 0.20.2 (SHA-256 pinned),
  `adx devtools hook` for a pre-commit `adx check --staged`.
- `rust` module: Rust 1.95.0 via rustup (rustup-init 1.29.3 SHA-256 pinned,
  reuses an existing rustup, never self-updates).
- `deploy-reviewer` agent (installed by `claude-rules`) and the
  `review-change` workflow (`.claude/workflows/review-change.js`).
- `tools/aodm` Cargo workspace: `aod-core` (`env`, `log`, `ini`, `error`, 12
  tests) and `aodm` (`--version`, `env`); `deny.toml` dependency policy.
- `docs/phone-image-manager.md`: technical document (filled per phase).
- `ROADMAP.md`: shipped, pending (in progress, planned, decision needed) and
  future features in one place.
- `diagram-design` skill (cathrynlavery/diagram-design, pinned f24d98b) for
  documentation diagrams; project profile `aod-flat-dark`
  (`config/skill-profiles/diagram-design/`, installed to `~/.diagram-design/profiles/`
  by the skills module, selected by the `.diagram-design` marker, also in the app
  template). All project diagrams are flat dark, system fonts, WCAG-checked tokens.
- `docs/diagrams/aodm-architecture.{html,svg}`: architecture of the planned phone
  image manager.
- `docs/claude-integration.md`: every skill, MCP server, agent, global rule,
  permission rule and workflow, with source, pin, scope and update steps.
- `docs/research/aodm-spike.md`: what the emulator really allows (settings,
  boot flags, identity fields, copies, locks, calls/SMS), measured on API 37.

### Fixed
- P2 reviews (code: 13, deployment: 10 findings), among them:
  - One junk `.ini` (directory, dangling link, non-UTF-8) made `aodm list`
    fail; it is now listed as broken with the reason.
  - AVD home follows the emulator (`ANDROID_AVD_HOME`, `ANDROID_EMULATOR_HOME`,
    `ANDROID_USER_HOME`); `path.rel` resolves against the user home and a
    stale `path=` falls back correctly; corrupt `path=` values outside `*.avd`
    are never walked.
  - Other users' emulators no longer count as running; `@name` and `-ports`
    launches are recognised.
  - Config loading passes only an allowlisted environment to bash (no exported
    functions, `PS4`/`SHELLOPTS` injection, forged values); `aodm env` notes
    ignored environment values; `ADX_AOD_HOME`/`ADX_AOD_SAVES_DIR` accept
    environment overrides (sandbox).
  - `devices` never overwrites or removes a `~/.local/bin/aodm` it didn't
    install (SHA-256 marker); rebuilds when HEAD changes; pinned target dir and
    toolchain; checks for `cc`; never installs a stale binary.
  - Errors are logged and printed once; a broken `images.toml` warns instead of
    hiding all images; `--group` matches whole group names and says when a
    group is empty; registry error lines are correct; size counts hard links
    once and stays on one filesystem.
  - CI also runs the release build and `aodm card`.
- P1 code review (21 findings), among them:
  - `adx check --staged` checked the working tree instead of the index; it now
    checks a snapshot of exactly what is staged (and stays offline).
  - Secret scan missed Anthropic/OpenAI-project/Google keys and keystore
    passwords and printed neighbouring content; it now reports file + kind
    only, and refuses keystores, `local.properties`, `google-services.json`, `.env`.
  - A failing docs check aborted the gate before the security section; every
    section now always runs. No crash without `origin/main`/`HEAD~1`;
    `ADX_CHECK_BASE` (set by CI per event) is honoured.
  - "Checksum-verified" is enforced: an empty hash fails unless
    `ADX_ALLOW_UNVERIFIED=1`; gradle and Node use pinned checksums
    (`ADX_GRADLE_SHA256`, new `ADX_NODE_SHA256`); raw downloads are flagged.
  - `adx up` runs each install as its own process, so `set -e` stays active
    inside modules (the 0.2.x fix had disabled it).
  - aodm config kept `XDG_*`/`USER`/`LANG`, reports config errors, falls back to
    the baked-in root; the INI editor rejects values that would inject keys,
    keeps CRLF/BOM, saves with fsync, keeps file mode and symlinks.
  - Pre-commit hook: ownership marker, real hooks path (`core.hooksPath`,
    worktrees), quoted root; `remove` only deletes adx's own hook.
  - Gate uses the pinned ShellCheck/cargo-deny; checks the Rust toolchain pins
    agree; docs checker validates actions and handles titles/absolute links;
    CHANGELOG rule requires a line under [Unreleased].
  - review-change workflow: range in every prompt, unverified findings kept
    apart, Confirmed/Plausible kept; deploy-reviewer default range fixed.
- `adx up` aborted at the first failed install (skipping every later module)
  and exited silently on an unknown module in a profile; it now warns,
  continues, and fails at the end listing incomplete modules.
- `adx check --full` ignored its own `ADX_APP_SKILLS=false` (defaults.env
  overwrote it); `ADX_APP_SKILLS` and `ADX_APP_GIT_INIT` now honour the
  environment.
- Skill profiles: a locally edited copy is kept as `.bak` before replacement.
- Pre-commit hook is a no-op on a checkout without `adx check` (rollback-safe).
- New apps from the template failed `verifyRoborazziDebug` (baselines moved
  into git in 0.2.0, but a new app has none yet). The scaffolder now says to
  record and commit baselines first; `adx check --full` records then verifies.
- `adx emulator stop` killed every running emulator; it now stops only
  `emulator-$ADX_EMU_PORT` (other images keep running).
- `adx emulator screenshot` failed with more than one device; it now targets
  its own serial and fails with a clear message.

### Changed
- aodm reads configuration with adx's precedence: config files win over the
  process environment (as in adx); only keys written as `${VAR:-default}`
  honour the environment. `aodm env` warns about ignored environment values.
  To sandbox aodm, set `ADX_AOD_HOME` (and/or `ADX_AOD_SAVES_DIR`), which are
  now `${VAR:-default}` keys for adx and aodm alike, or `ANDROID_AVD_HOME`.
- CI triggers once per change (push to `main` and tags, pull requests) and
  cancels superseded runs.
- `origin` pushes to MensuraMedia only; LinearGenetics is a separate remote
  (`linear`) until it grants write access.
- CI runs `adx check` (same gate as local) with a read-only token,
  `actions/checkout` pinned by commit SHA and tools from the framework's own
  pinned modules.
- Default profile adds `rust` and `devtools`; `render-only` adds `devtools`.
- New config keys: `ADX_RUST_*`, `ADX_RUSTUP_*`, `ADX_CARGO_HOME`,
  `ADX_SHELLCHECK_*`, `ADX_CARGO_DENY_*`, `ADX_AOD_*`.
- Vendored skills in the framework repo are git-ignored (`adx skills apply .`
  recreates them from the pinned manifest).

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
