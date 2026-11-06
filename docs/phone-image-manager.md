# aodm: phone image manager (technical document)

Design and development record of `aodm`, the android-on-debian phone image
manager. Filled in phase by phase (status per section); the feature status is
in [ROADMAP.md](../ROADMAP.md), measured emulator behaviour in
[research/aodm-spike.md](research/aodm-spike.md).

![aodm architecture](diagrams/aodm-architecture.svg)

## 1. Purpose and terms *(P0, done)*

- **Image**: one Android Virtual Device (AVD): `~/.android/avd/<name>.ini` + `<name>.avd/`.
- **Save**: a full copy of an image at a point in time, reopenable in place or as a new image.
- **Template**: a versioned recipe (or golden save) that builds new images.
- **Identity policy**: how an image's identity fields are set (`private`, `standard`, `fixed`).
- **Lock**: a per-image security control the manager enforces.
- **Spin-off**: an image turned into a standalone phone (`aod-phone`), linked or portable.

## 2. Architecture *(P1, in progress)*

Cargo workspace `tools/aodm/` (edition 2024, Rust 1.95 pinned in
`rust-toolchain.toml`, `Cargo.lock` committed, `--locked` everywhere,
`unsafe_code = "forbid"`, clippy `-D warnings`, `cargo deny` policy in `deny.toml`).

| Crate | Status | Role |
| --- | --- | --- |
| `aod-core` | P1: `env`, `log`, `ini`, `error`; P2: `avd`, `proc`, `registry`, `card` | Library, no terminal I/O; shared by every front end |
| `aodm` | P2: `list [--group G]`, `card`, `env`, `--version` | CLI (clap); TUI from P8 |
| `aod-phone` | P7 | Standalone phone runtime + mechanics panel |
| `aod-gui` | future | eframe front end |

### Built in P1

- **`env`**: one source of truth, adx's precedence. `config/defaults.env` +
  `config/local.env` are sourced through bash with the real environment (the
  framework is found via `ADX_ROOT`, else the root baked in at build time), so
  config files win and only `${VAR:-default}` keys take environment overrides
  (sandbox: `ADX_AOD_HOME`, `ADX_AOD_SAVES_DIR`, `ANDROID_AVD_HOME`); then the
  environment for keys the config doesn't set; then built-in fallbacks (a test
  keeps them equal to defaults.env). `aodm env` shows each value, its source,
  and warns about environment values that are ignored.
- **`log`**: appends to `$ADX_LOG_DIR/adx.log` in adx's exact format
  (`YYYY-mm-dd HH:MM:SS LEVEL [aodm #run] msg`), one write per line, size
  rotation and generation shift identical to adx; never fails a command.
- **`ini`**: lossless `config.ini` editor: byte-identical round trip on the
  real AVD file, `set` changes one value in place (keeps spacing, order,
  comments, trailing newline), last occurrence wins, atomic save.
- **`error`**: typed errors (`Io`, `Parse`, `Config`; more per phase) so front
  ends can react.

## 3. Data on disk *(P2, done for inventory)*

| Path | What | Read by |
| --- | --- | --- |
| `<avd home>/<name>.ini` | `path=` (absolute), `path.rel=` (relative to the Android user home), `target=android-<api>` | `avd::scan`: folder = first existing `*.avd` dir of `path=`, `<user home>/path.rel`, `<avd home>/<name>.avd`. AVD home as the emulator resolves it: `ANDROID_AVD_HOME`, else `$ANDROID_EMULATOR_HOME/avd`, else `$ANDROID_USER_HOME/avd`, else `~/.android/avd` |
| `<name>.avd/config.ini` | hardware/device/image keys (`hw.*`, `abi.type`, `tag.id`, `image.sysdir.1`); `avd.id`/`avd.name` are literal `<build>` placeholders | `avd::Image` accessors, lossless `ini` editor |
| `<name>.avd/` | qcow2 disks (relative backing files), `snapshots/` (absolute paths), lock files while running | size via allocated blocks (`du`), copies in P4 |
| `$ANDROID_HOME/<image.sysdir.1>/source.properties` | system image `Pkg.Revision` | version card / list `REV` |
| `/proc/<pid>/cmdline` | running emulators: qemu process name + `-avd`/`-port` | `proc::emulators` (never matches shells) |
| `$ADX_AOD_HOME/images.toml` | registry: groups, pinned port, template, origin; unknown keys preserved; atomic save | `registry::Registry` |

Images whose folder or `config.ini` is missing, and junk entries (a directory
or dangling link named `*.ini`, an unreadable or non-UTF-8 `.ini`) are still
listed as `broken`, with the reason, so one bad entry never hides the rest.
Only the current user's emulators count as running; `-avd NAME` and the
launcher's `@NAME`, `-port N` and `-ports N,M` are recognised. Sizes count
allocated blocks, stay on one filesystem, and count hard links once. Names aodm accepts: letters, digits, `.`,
`_`, `-`, 1–64 characters, not starting with `.` or `-`.
## 4. Saves *(P4)*
## 5. Running many emulators *(P3, done)*

`aodm start|stop|restart|health TARGETS`: TARGETS are image names, `--all`,
`--group G`, and/or `--running` (alone: every running image). `health` with
no TARGETS checks every image.

- **Ports.** adb discovers emulators only on console ports 5554–5584 (even;
  adb port = console + 1): 16 slots from `ADX_AOD_BASE_PORT`. Reserved ports
  (a `port` pin in `images.toml`, and `ADX_EMU_PORT` for adx's own
  `ADX_AVD_NAME`) belong to their image only, so `adx emulator start` always
  finds its port. Otherwise the first slot that no running emulator uses and
  where both ports bind on 127.0.0.1. `$ADX_AOD_HOME/ports.lock` (flock) is
  held from port choice until every emulator of the run has booted, so two
  aodm runs never pick the same port.
- **Running check**: an image is running when an own-user qemu process runs
  `-avd <name>` (`/proc` scan) **and** that image's `multiinstance.lock` is
  held (exclusive flock by the emulator, verified on 37.2.13). A same-named
  emulator from another AVD home (a sandbox) is therefore never stopped,
  restarted or wiped; a held lock with no own emulator is refused. All of it
  is re-read after taking `ports.lock`.
- **Budget.** Before anything starts: running + new ≤ `ADX_AOD_MAX_INSTANCES`;
  the guest RAM of the new images (`hw.ramSize`) + `ADX_AOD_RAM_RESERVE_MB` ≤
  `MemAvailable` (running guests are already counted there). Emulator cores
  above host threads only warn. One refusal stops the whole start.
- **Launch.** Same flags as `adx emulator start`: `-avd N -port P -gpu
  $ADX_EMU_GPU -no-boot-anim`, `-no-window -no-audio` when headless. The
  emulator gets the `ANDROID_HOME`, `ANDROID_AVD_HOME`, `ANDROID_USER_HOME`,
  `ANDROID_EMULATOR_HOME` aodm resolved, then `ADX_EMU_ENV`; log in
  `$ADX_LOG_DIR/emulator-<image>.log` (`ADX_LOG_KEEP` previous runs kept). `--cold`
  adds `-no-snapshot-load`, `--wipe` adds `-wipe-data` after a typed
  confirmation (or `--yes`). The emulator runs in its own process group, so
  Ctrl-C or closing aodm's terminal leaves it running.
- **Boot.** All images of one run boot in parallel; each waits for
  `sys.boot_completed=1` and fails fast if the emulator exits, if adb still
  says `unauthorized` after 60 s (hint: `--wipe`), or after
  `ADX_EMU_BOOT_TIMEOUT`. A failed boot that is still alive is stopped like
  `stop` does (identity check, console kill, SIGTERM), so it doesn't hold a
  port; one that exited is only reaped.
- **Stop.** Console `avd name` must equal the image (a reused port can't stop
  another emulator), then `kill` (the emulator saves its quick-boot snapshot)
  and up to 30 s to exit; then SIGTERM and 15 s, after re-checking that the pid is
  still that emulator. Never SIGKILL (can corrupt the image): a stuck emulator
  is reported with the pid.
- **Clients.** `adb` runs with `-s emulator-<port>` and a timeout every time;
  a timed-out tool is killed with its process group, and the deadline also
  covers output pipes a grandchild keeps open. The console client speaks
  the emulator's protocol directly (banner, `auth` with
  `~/.emulator_console_auth_token`, replies ending in `OK` / `KO:`), so health
  has a signal independent of adb, and P7's mechanics panel reuses it. The
  token is never logged.
- **Health.** Per image: process, serial, adb state, boot, console (`avd
  name` must match the image, then `avd status`), error lines in the emulator
  log. Exit 1 when a running image isn't booted and answering, or an image is
  broken or locked by something else.
- **Restart** stops the targets in parallel and starts every one that is now
  stopped, even if another's stop failed (reported, exit 1). `start --wipe`
  refuses running images (use `restart --wipe`).
- **Several emulators and plain adb:** `adb` without `-s` fails once two
  emulators run; set `ANDROID_SERIAL` (global rules, app template).
- **Measured** (this laptop, software GPU): quick-boot start 8–10 s, cold
  boot of a fresh copy next to a running emulator 18 s, console stop 11 s,
  crash during boot reported in 2 s.
## 6. Settings profiles *(P5)*
## 7. Templates *(P6)*
## 8. Identity policies *(P5)*
## 9. Security locks *(P5)*
## 10. Spin-offs, phone mechanics, switchboard *(P7)*
## 11. The ten multi-device features *(P2–P8)*
## 12. Version card *(P2, done)*

`aodm card` (CLI; TUI pane in P8; GUI grid later) renders one data model
(`card::VersionCard`): two even columns, each padded to its longest key, then
the image table (same as `aodm list`).

| Column | Section | Rows (source) |
| --- | --- | --- |
| left | aodm | version + git (built in), adx release + git (`CHANGELOG.md` first released heading, `git describe`), config root |
| left | Host | OS (`/etc/os-release`), kernel, CPU + threads (`/proc/cpuinfo`), RAM total/available (`/proc/meminfo`), KVM (open `/dev/kvm`), free disk on the saves filesystem (`statvfs`) |
| right | Toolchain | emulator, adb, cmdline-tools (`source.properties`), SDK platforms, build-tools (directories), JDK (`release`), Rust (compiler that built aodm) |
| right | Emulation | GPU mode, emulator environment (NVIDIA offload / none / custom), running vs `ADX_AOD_MAX_INSTANCES` |
| table | Images | name, state (running :port / stopped / broken), API, ABI, tag, system image revision, device, RAM, size on disk, groups |

The card itself only starts `git describe`; loading the configuration runs
bash once (sourcing `defaults.env`/`local.env` with an allowlisted environment).
## 13. Safety *(every phase)*

## 14. Dependencies and best practices *(P1)*

| Crate | Where | Why |
| --- | --- | --- |
| `clap` (derive, default features off) | aodm | CLI with nested help |
| `anyhow` | aodm | Error context in the binary |
| `thiserror` | aod-core | Typed errors |
| `jiff` | aod-core | Local-time log timestamps (sound across threads) |
| `serde` + `toml` | aod-core | `images.toml` registry (unknown keys preserved) |
| `rustix` (fs) | aod-core | `statvfs` (free disk); flock/kill in later phases |
| `tempfile` | dev | Test fixtures |

Bar for a new dependency: it replaces more code than it costs, has a stated
reason here and in [dependencies.md](dependencies.md), passes `cargo deny`
(licences allowlist: MIT, Apache-2.0, Unicode-3.0), and comes from crates.io.

## 15. Testing *(every phase)*

P3 review: 55 unit tests; new: port-pin validation, emulator environment,
panicked worker thread, grandchild holding pipes, token with a line break.
Replayed end to end (on a throwaway copy only): concurrent `start` waits on
the lock and reports "already running"; a sandbox `ANDROID_AVD_HOME` with a
same-named image never stops the real one (`stop`, `stop --all`, `start`
refused); crashing boot reaped with no process left.

P3: 51 unit tests in total; new: tool deadline and process-group kill,
console protocol against a fake console (auth, `KO:`, `kill` hang-up, no
line-break injection, bad token), adb states and a fake serial-scoped adb,
port slots / reservation / busy pin / 16-slot exhaustion, port lock
exclusivity, budget (instances, RAM reserve, CPU warning, `hw.ramSize`
syntaxes), launch arguments equal to adx's, `ADX_EMU_ENV` parsing,
`multiinstance.lock`, log error lines. End to end on the real emulator:
stop/start of `adx_api37`, a second image on 5556 next to it, `adx emulator
stop` leaving the other image running, restart, `stop --running`, a broken
image failing fast with no process left.

P2: 30 unit tests in total; new: AVD scan of a fake home (stale `path=`,
broken images, non-AVD files), name rules, `.properties` parsing, `du`, human
sizes, `/proc` parsing (real emulator line, shells, launcher, `-headless`),
fake `/proc` tree scan, registry round trip with unknown keys and parse error
lines, version card column alignment, config precedence with a stale
environment, XDG and `${VAR:-}` semantics.

P1: 18 unit tests (config precedence, rejected root, config errors reported,
fallbacks equal defaults.env, log format/sanitising/rotation incl. keep=0,
lossless ini round trip / set / injection rejected / CRLF+BOM / durable save
keeping mode and symlinks). Run:
`cd tools/aodm && cargo test --locked`, or `adx check`.

## 16. Change pipeline *(P1, in progress)*

- **Gate:** `adx check [--staged|--full]` (`lib/check.sh`): shell (`bash -n`,
  ShellCheck), Rust (fmt, clippy, test, cargo deny), docs (links, documented
  commands, stale terms), process (CHANGELOG with code, HANDOFF current),
  security (secrets in the diff incl. new files, masked in output; no
  pipe-to-shell; every download checksum-verified; permission/rules/agent
  changes flagged for owner approval). `--full` renders the app template and
  runs its tests. CI runs the same command.
- **CI:** read-only token, `actions/checkout` pinned by SHA, tools installed by
  the framework's own pinned modules (`devtools`, `rust`).
- **Agents:** `adversarial-reviewer` (per segment) and `deploy-reviewer`
  (upgrade, migration, rollback, clean install, blast radius, CI parity).
- **Workflow:** `.claude/workflows/review-change.js`: scope → segment reviews +
  deployment review → two skeptics per finding → confirmed list.
- **Hook:** `adx devtools hook` installs a pre-commit hook running `adx check --staged`.
- Later (P9): `adx release`, `adx upgrade` with migrations and rollback, `adx new-module`.

## 17. Development history

| Date | Phase | Decision / result |
| --- | --- | --- |
| 2026-11-06 | Plan | Rust; CLI+TUI first, GUI later; full-copy saves; identity policies; spin-offs; templates; change pipeline |
| 2026-11-06 | P0 | Spike results: see research/aodm-spike.md (IMEI/serial/SIM fixed by emulator; Android ID must be regenerated on copies; console-driven calls/SMS) |
| 2026-11-06 | P1 | Gate's own first run found and fixed: new files not scanned for secrets; secrets echoed into the log |
| 2026-11-06 | P3 | Console client instead of `adb emu` for stop/health (independent of adb); reserved ports so `adx emulator` and aodm never collide; no SIGKILL |
| 2026-11-06 | P3 review | code (14) + deployment (11) findings, all fixed: no SIGKILL on failed boots, identity check before console kill, running = process + image lock (sandbox-safe), resolved Android paths passed to the emulator, checks under `ports.lock`, restart and `--wipe` semantics, pin validation, gate's staged target dir |
| 2026-11-06 | P2 review | code review (13) + deployment review (10), all fixed: junk `.ini` no longer hides images, emulator AVD-home lookup order, own-user emulators only, `@name`/`-ports`, allowlisted config environment, `devices` ownership marker and rebuild on new commit; reproductions replayed |
| 2026-11-06 | P1 review | adversarial-reviewer (21 findings) and deploy-reviewer (9 findings), all fixed; reproductions replayed in a clean clone: staged-snapshot gate, secret kinds, section continuation, shallow clone, pipe-to-shell forms, hook ownership |

## 18. Known limits (`shortcut:` ledger)

- `budget`: an emulator started seconds ago hasn't used its RAM yet, so
  separate back-to-back `aodm start` runs can overcommit; start them in one run.
- Lifecycle orchestration (`lifecycle::start/stop`) is tested end to end on
  the real emulator, not by an automated fake-SDK test yet (planned with P4's
  integration tests).
- `health` doesn't restart a lost adb server (that would drop every device,
  mobile-mcp included); it says so.

- `registry`: no `schema` key yet; the first writer (P3/P4) adds `schema = 1`
  and refuses to save over a newer schema. `list`/`card` warn and continue on a
  broken `images.toml`.

- `env`: built-in fallbacks duplicate `defaults.env` (only used without the framework).
- `log`: rotation can race a concurrent adx run (harmless, one extra generation).
