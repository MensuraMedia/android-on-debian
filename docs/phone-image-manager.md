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
## 5. Running many emulators *(P3)*
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
| 2026-11-06 | P2 review | code review (13) + deployment review (10), all fixed: junk `.ini` no longer hides images, emulator AVD-home lookup order, own-user emulators only, `@name`/`-ports`, allowlisted config environment, `devices` ownership marker and rebuild on new commit; reproductions replayed |
| 2026-11-06 | P1 review | adversarial-reviewer (21 findings) and deploy-reviewer (9 findings), all fixed; reproductions replayed in a clean clone: staged-snapshot gate, secret kinds, section continuation, shallow clone, pipe-to-shell forms, hook ownership |

## 18. Known limits (`shortcut:` ledger)

- `registry`: no `schema` key yet; the first writer (P3/P4) adds `schema = 1`
  and refuses to save over a newer schema. `list`/`card` warn and continue on a
  broken `images.toml`.

- `env`: built-in fallbacks duplicate `defaults.env` (only used without the framework).
- `log`: rotation can race a concurrent adx run (harmless, one extra generation).
