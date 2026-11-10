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
- P3 adds on-disk state only when used: `$ADX_AOD_HOME/ports.lock` (creates
  `~/.local/share/aod/` on the first `aodm start`) and
  `$ADX_LOG_DIR/emulator-<image>.log` (+ `.1`…) per image. New config keys
  `ADX_AOD_BASE_PORT`, `ADX_AOD_MAX_INSTANCES`, `ADX_AOD_RAM_RESERVE_MB` have
  defaults in `defaults.env`; nothing to migrate. With two emulators running,
  plain `adb` (no `-s`) fails: set `ANDROID_SERIAL` (see the global rules).
- P4 adds `$ADX_AOD_SAVES_DIR` (default `~/.local/share/aod/saves`; each
  save is about the size of the image, 5–10 GB) and `$ADX_AOD_HOME/images.lock`;
  `images.toml` gains `schema = 1` (the P3 build reads it fine and never
  writes the file). Uses GNU coreutils `cp` and `sha256sum`; the saves folder
  must be on a POSIX filesystem (ext4, btrfs, xfs; not exFAT/NTFS).
- Rolling back from P4: saves (5–10 GB each) are invisible to the P3 build and
  to v0.2.0; delete the unwanted ones first (`aodm save list`, `save delete`)
  or remove `$ADX_AOD_SAVES_DIR` afterwards. Images made by `clone`/`save open`
  are ordinary AVDs and keep working (`aodm delete` them first if not wanted).
  `schema = 1`, `origin` keys and `images.lock` can stay. Check `ls -a
  ~/.android/avd "$ADX_AOD_SAVES_DIR"/*/` for hidden `.…partial-*`,
  `.…restore-*` or `.deleting-*` folders of interrupted runs and remove them
  by hand.
- P5a: the first `adx devices install` after updating downloads five new
  crates (`clap_complete`, `serde_json` and three small dependencies): it
  needs the network once. `images.toml` may gain `[images.NAME.tags]` tables
  (still `schema = 1`; the P4 build keeps them). Change history starts with
  P5a: `aodm audit` doesn't show changes made by earlier builds. aodm's
  version stays `0.3.0-dev`; scripts can't tell the new exit codes from
  `--version` until 0.3.0.
- P10a (window `aod-gui`): `adx devices install` now also builds and installs
  `~/.local/bin/aod-gui` and the menu entry "Phone Images". The first build
  downloads about 300 crates (eframe; network needed once) and takes a few
  minutes; the build folder grows to several GB with `adx check`. No `-dev`
  packages: X11/Wayland, EGL and xkbcommon are loaded when the window opens
  (`adx devices status` warns if missing). An `aod-gui` you put in
  `~/.local/bin` yourself stops the install (move it away, or `ADX_FORCE=1`,
  which also replaces a foreign `aodm`). Headless machines build it too.
- Rolling back from P10a to P5a: on the new code `./adx devices remove`
  (binaries, markers and menu entry), `rm -rf ~/.local/share/aod/thumbs`
  (the window's last screens), then `git checkout 5110aa3 && ./adx devices
  install`. The older code doesn't know the window and would leave it behind.
- Rolling back from P5a to P4: `git checkout acbb91a && ./adx devices
  install`. Not undone: `aodm config set` edits in `config/local.env` (plain
  `KEY="value"` lines both builds read), wiped images, logs removed by `aodm
  gc`, a completion script you installed (`aodm completion …`; remove it by
  hand). Tags stay in `images.toml` and come back on upgrade.
- Rolling back: first `aodm stop` every image except `adx_api37` (older adx
  and v0.2.0's `adx emulator stop` would kill all emulators, and its
  screenshot fails with several devices); `ports.lock` and the extra
  emulator logs can stay or be deleted. To v0.2.0: `./adx devices remove` (deletes only adx's
  `~/.local/bin/aodm`; v0.2.0 would never notice it), `./adx devtools hook remove` (the hook is a no-op on
  v0.2.0 anyway), `rm -rf tools/aodm/target`, `git checkout v0.2.0`. Left
  behind, remove by hand if wanted: Rust toolchain 1.95.0
  (`~/.cargo/bin/rustup toolchain uninstall 1.95.0`; never delete `~/.cargo`
  or `~/.rustup` if they predate adx), `~/.local/opt/{shellcheck,cargo-deny}-*`
  and their `~/.local/bin` links, `~/.claude/agents/deploy-reviewer.md`,
  `~/.diagram-design/profiles/aod-flat-dark.md`.

### Planned for 0.3.0 (approved, not yet built)
- `aodm`: bulk settings profiles, bulk
  app deploy, broadcast/capture, terminal UI.
- Build templates with a built-in `barebones` default; identity policies
  (`private`/`standard`/`fixed`); per-image security locks; spin-offs into
  standalone phones (`aod-phone`) with a calls/SMS switchboard.
- `adx release`, `adx upgrade` with migrations and rollback, `adx new-module`.
- Remaining technical document sections and diagrams.

### Added
- Console **Assets** page (`tools/console/assets.py`): your folders in
  `ADX_CONSOLE_ASSETS_DIR` (wallpapers, photos & videos, ringtones & sounds,
  APKs, screenshots) with upload (button or drag and drop; type and size
  checked, `ADX_CONSOLE_UPLOAD_MAX_MB`), and the top 10 features for the
  running emulator phone: 1 wallpapers (load, remove, Set opens the phone's
  set-wallpaper preview), 2 photos & videos (`DCIM/adx`), 3 ringtones & sounds
  (`Ringtones|Notifications|Alarms/adx`), 4 APK install/uninstall, 5 location
  (10 cities or lat, lon), 6 dark mode, 7 battery level and charging, 8
  airplane / Wi-Fi / mobile data, 9 text size, animations, show taps, 10 clean
  status bar (demo mode) and screenshot. Only emulators aodm reports as running
  are targets; removal only deletes the console's own `adx` folders on the
  phone; each action is recorded as a job.
- `console` module and `tools/console/`: a local web console (`adx console
  start|stop|open|status`, menu entry "adx console", port `ADX_CONSOLE_PORT`,
  default 8766), Python standard library only. Pages: Images (emulator phones
  through `aodm`: Launch / Stop / Save; SDK system images: Open folder), Apps
  (screenshot thumbnail and gallery; Render, Run, Open folder), Modules
  (`adx list`, Install, Run doctor), Skills, Activity (jobs + `adx.log`) and
  Settings (`config/*.env`, `adx env`). Same horizontal-card design and
  Phosphor icons (MIT, `tools/console/static/icons/`) as LinSmartPhone, without
  its LineageOS parts. Binds to 127.0.0.1; actions need same-origin + a
  per-process token, Host header checked; every action is a job with a log in
  `~/.local/state/adx/console/`. Added to `profiles/default.profile`.
- Desktop window `aod-gui` (P10a, moved ahead of P5b on request), menu entry
  "Phone Images": one card per phone image with a live thumbnail of its screen
  (running phones every 4 s over adb; stopped phones show their last screen,
  dimmed, kept in `$ADX_AOD_HOME/thumbs/`), state, port, device, size, saves,
  groups, and Start / Stop… (asks first) / Save / Clone… buttons that run
  `aodm` (same checks, logging and audit trail as the CLI). The list is read
  in-process (nothing logged per poll); a phone whose screen is off shows
  "screen off" and never overwrites its last screen. Thumbnails are private
  (0600), moved by `rename`, dropped by `delete` and `wipe`, and swept by `gc`
  when their image is gone. `aod-gui --version`. Installed by `adx devices
  install`; adx only replaces or removes a menu entry it wrote.
- aodm P5a (command library, part 1; reference: `docs/aodm-cli.md`):
  - Conventions for every command: `--json` (one document on stdout:
    `schema`, `command`, `ok`, `results`, `errors`), `--dry-run` (on
    `delete`, `wipe`, `save restore/delete/prune`, `gc`, `config set/unset`;
    any other command refuses it with exit 2 instead of running for real),
    `-q`/`-v` (no progress / every log line on stderr), `--no-color`,
    `--sandbox DIR` (images, registry, saves and log under DIR), global
    `--yes`; exit codes 3 refused, 4 partial, 5 timeout; the `aodm image …`
    group (same commands as the short forms).
  - `aodm show NAME` (everything about one image), `aodm wipe NAME`
    (factory reset without booting; config and identity kept), `aodm group
    list|add|remove|delete`, `aodm tag set|unset` and the `--tag K=V`
    selector, `aodm save show IMAGE ID` (the manifest).
  - Manager administration: `aodm doctor` (config, framework revision, SDK,
    KVM, registry, broken images, held locks, saves filesystem, free space,
    leftovers; exit 1 on a failed check), `aodm config
    list|get|set|unset|validate|path` (`set` writes `config/local.env`,
    checked first, other lines kept), `aodm logs [-n N] [-f] [--level L]
    [--image NAME]`, `aodm audit [--since DATE] [--image NAME]`, `aodm disk`,
    `aodm gc`, `aodm completion bash|zsh|fish`.
  - Audit trail: every change (save, restore, open, pin, delete, clone,
    rename, wipe, group/tag, config set/unset, gc) is logged as an `audit:`
    line in `adx.log` with the user.
  - Docs/CLI gate: a test fails when a command is missing from
    `docs/aodm-cli.md` or a command documented as done doesn't exist; every
    `--help` is rendered in a test; `tests/cli.rs` runs the binary against a
    sandbox (JSON envelope, exit codes, groups, tags, wipe, gc, audit).
- `docs/aodm-cli.md`: the complete command library (specification): the
  twenty management and architecture features (inventory, lifecycle,
  provisioning, clone/rename/delete, backup/restore, groups, configuration
  management, identity, policy locks, app deployment, remote execution,
  monitoring, resource management, audit, portability, system image
  catalogue, scripting, self-administration, upgrades/rollback, storage/GC),
  CLI conventions (grammar, selectors, `--json`, `--dry-run`, `--sandbox`,
  exit codes), every command for images, the manager and its settings with
  status and phase, the settings reference and the deployment pipeline
  commands. ROADMAP phases P5–P9 extended accordingly.
- aodm P4 (saves, clone, rename, delete): `aodm save create IMAGE [--label L]`
  (full copy of a stopped image, quick-boot snapshot included, SHA-256 of every
  file in `manifest.toml`), `save list [IMAGE]`, `save verify`, `save restore
  IMAGE ID [--backup-first]` (verified first, swapped in with two renames),
  `save open IMAGE ID --as NEW`, `save pin [--off]`, `save delete`, `save prune
  IMAGE [--keep N]`; `aodm clone SRC NEW`, `rename OLD NEW` (saves and registry
  entry move along), `delete NAME` (saves kept). IDs: exact, unique prefix or
  `latest`. Restore, delete and prune ask you to type a word (or `--yes`).
  Each of them holds the image's `multiinstance.lock`, the flock the emulator
  itself takes, so an image can't run or start while it is copied, moved or
  removed. Retention `ADX_AOD_MAX_SAVES` (pinned saves exempt), a free-space
  check with 1 GiB margin, never automatic pruning. adx's own image
  (`ADX_AVD_NAME`) can't be renamed or deleted. `aodm list`/`card` show a
  SAVES column.
- `aod-core` modules `images` and `saves`; `Registry::update` (flock on
  `images.lock`, `schema = 1`); integration test on a fake SDK
  (`tests/saves.rs`).
- aodm P3 (lifecycle): `aodm start|stop|restart|health` for names, `--all`,
  `--group G` and/or `--running`. Start picks a free console port (5554–5584,
  16 slots: adb's range; registry pins and adx's own AVD on `ADX_EMU_PORT`
  are reserved), checks the budget (`ADX_AOD_MAX_INSTANCES`, host RAM minus
  `ADX_AOD_RAM_RESERVE_MB`; CPU oversubscription warns), launches with the
  same flags, environment and log file as `adx emulator start` (detached),
  boots several images in parallel and fails fast on a crash or an
  `unauthorized` device. `--window`, `--cold`, `--wipe` (asks you to type
  `wipe`, or `--yes`). Stop uses the emulator console `kill` (quick-boot
  snapshot saved), then SIGTERM, never SIGKILL. Health checks process, adb
  state, boot, console (`avd name` must match) and emulator-log errors, and
  exits 1 when something needs attention. `ports.lock` keeps concurrent
  aodm starts off each other's ports; an image whose `multiinstance.lock` is
  held by a process aodm can't attribute is refused.
- `aod-core` modules `cmd` (external tools with a deadline; kills the whole
  process group), `adb` (serial-scoped), `console` (emulator console client
  with auth token), `ports`, `budget`, `emu`, `lifecycle`.
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
- Host keyboard did not type into the emulator: `avdmanager` creates AVDs with
  `hw.keyboard=no`. `adx emulator create` (and `install`) now sets
  `hw.keyboard=$ADX_AVD_HW_KEYBOARD` (default `yes`) in the AVD's `config.ini`,
  idempotently, also on an existing AVD; it applies from the next start.
- aodm P5a reviews (code 13, deployment 10 findings):
  - `--sandbox` could reach a real image through a sandbox `.ini` whose
    `path=` or `path.rel=` pointed outside it (a sandboxed `wipe`/`delete`
    could erase it): the Android user home now lives in the sandbox and
    images whose folder resolves outside it are ignored. A sandboxed `gc` no
    longer looks in the framework's real `config/` folder.
  - `--dry-run` still removed leftovers of crashed runs; it now changes nothing.
  - `gc` could race a `save create` or a `rename` onto an empty saves folder
    and delete saves recursively: empty folders are removed non-recursively,
    every item is re-checked just before removal, and only aodm's own folders
    (image names) and adx's own logs (`adx.log.N`, `emulator-*.log.N`) count.
  - Exit codes 3/5 were lost when every target failed the same way (e.g.
    `start` of a broken image exited 1); per-image JSON results now carry a
    `code`. Real failures (console auth, a tool's error, the emulator dying
    during boot) exit 1 instead of 3.
  - `aodm audit` could be fed fake entries through text quoted in another log
    line; it now only reads lines in aodm's exact audit format, records the
    uid next to `$USER`, and other messages can't start like an audit line.
  - `aodm config set` of a data path (`ANDROID_AVD_HOME`, `ANDROID_USER_HOME`,
    `ADX_AOD_HOME`, `ADX_AOD_SAVES_DIR`) refuses while the old folder holds
    data, unless `--yes`. `local.env` edits keep a symlinked file's target and
    the file's permissions, recognise `export<TAB>KEY=`, and refuse keys whose
    value spans several lines; `config unset` only takes known keys.
  - `--json completion` produced invalid JSON (now a usage error); `logs
    --json` lines are objects; `show` results have `ok`. Malformed `--tag`,
    unknown `config get` keys and bad `--since` dates are usage errors (2).
  - `wipe` removes the quick-boot snapshot first (an interrupted wipe can't
    leave it over fresh data) and sweeps leftovers next to images outside
    the AVD home. `doctor`/`card` run `git describe` without taking git's
    index lock; doctor's held-lock check works on btrfs.
  - The docs/CLI gate also fails when a built command is still documented as
    planned; the CLI test asserts nothing is written outside the sandbox.
- aodm P4 reviews (code 12, deployment 12 findings):
  - **Critical:** the `save` commands took the image argument as a path, so
    `save delete ""`, `..` or an absolute path removed folders outside the
    saves dir. It must now be an image name, and delete removes only
    `<saves>/<image>/<id>`.
  - **Critical:** an image folder that is a symlink (moved to another disk) was
    "saved" as a link, and clone edited the live folder through it. Symlinked
    folders are now refused; `delete` removes only the link.
  - Ctrl-C mid-copy left `cp` running after aodm and its lock were gone: `cp`
    and `sha256sum` now run in aodm's process group.
  - Leftovers of interrupted runs (`.…partial-PID`, `.restore-PID`,
    `.deleting-…-PID`) are removed, and logged, by the next P4 command.
  - Restore swaps the folders in one atomic `RENAME_EXCHANGE`, so a crash can
    no longer leave an image without its folder.
  - Copies are synced to disk before the old state is removed.
  - An emulator started with `-read-only` (which skips the lock) is detected
    from `/proc`.
  - Save ids use UTC (`20261106-221520Z`), so `latest` and prune order survive
    DST and time-zone changes.
  - The registry is checked before any disk change, so a newer `schema` no
    longer leaves half-done clones.
  - Rename reports what is left to do by hand if a late step fails.
  - Delete renames the folder before removing it.
  - `ADX_AOD_SAVES_DIR`, `ADX_AOD_HOME` and `ANDROID_AVD_HOME` must be
    absolute paths.
  - Without the framework config, saves follow `ADX_AOD_HOME`.
  - `sha256sum` output is validated; manifests carry `format = 1`.
  - The delete prompt shows the folder it removes.
- P3 reviews (code 14 + deployment 11 findings, all fixed):
  - a failed boot is shut down with console `kill` and SIGTERM, never
    SIGKILL, and only after the console confirms it's the same image; `stop`
    checks `avd name` too, so a reused port can't stop another emulator;
  - an image counts as running only if its emulator holds that image's
    `multiinstance.lock`: a sandbox (`ANDROID_AVD_HOME`) copy with the same
    name never stops, restarts or wipes the real one;
  - the emulator gets the `ANDROID_HOME`/`ANDROID_AVD_HOME`/… aodm resolved,
    so it boots the image aodm checked even from a shell without them;
  - running state, locks and the budget are read after taking `ports.lock`
    (a concurrent start says it is waiting and then sees "already running");
  - `restart` starts every image that stopped even if one stop fails;
    `start --wipe` refuses running images instead of reporting success;
  - registry port pins must be adb slots and unique; a bad
    `ADX_AOD_BASE_PORT`/`ADX_EMU_PORT` is a config error, not truncated;
  - tool timeouts also cover pipes held by grandchildren; a panicked worker
    thread is reported as a failure; console fallbacks and spawned pids are
    logged; tokens with control characters are rejected and never echoed;
  - emulator logs keep `ADX_LOG_KEEP` generations like adx;
    `health --running` with nothing running exits 0;
  - `adx emulator start` checks `unauthorized` only for its own serial (another
    image's state no longer aborts it);
  - `adx check --staged` builds in its own target dir (`target/staged`): a
    snapshot's test binary was reused by the next working-tree run and failed.
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
- `rustix` dependency gains its `process` feature (kill, process groups); no
  new crates.
- aodm (P5a): per-command `--yes` is now a global flag (same spelling, still
  accepted after the command). Exit codes are more specific: a declined or
  impossible confirmation and other refusals exit 3 (was 1), partial failures
  4, timeouts 5, a missing selector 2. Confirmation prompts go to stderr.
  `aodm list` adds `--tag`. New crates `serde_json` (`--json`) and
  `clap_complete` (`completion`); see docs/dependencies.md §5b.
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
