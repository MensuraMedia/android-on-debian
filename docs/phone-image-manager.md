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
| `aod-core` | P1: `env`, `log`, `ini`, `error` | Library, no terminal I/O; shared by every front end |
| `aodm` | P1: `--version`, `env` | CLI (clap); TUI from P8 |
| `aod-phone` | P7 | Standalone phone runtime + mechanics panel |
| `aod-gui` | future | eframe front end |

### Built in P1

- **`env`**: one source of truth. Per key: process environment (under `adx`
  this already equals the config) → `config/defaults.env` + `config/local.env`
  sourced through bash (when the framework is found via `ADX_ROOT`, or the
  root baked in at build time) → built-in fallbacks. `aodm env` shows each
  value and its source.
- **`log`**: appends to `$ADX_LOG_DIR/adx.log` in adx's exact format
  (`YYYY-mm-dd HH:MM:SS LEVEL [aodm #run] msg`), one write per line, size
  rotation and generation shift identical to adx; never fails a command.
- **`ini`**: lossless `config.ini` editor: byte-identical round trip on the
  real AVD file, `set` changes one value in place (keeps spacing, order,
  comments, trailing newline), last occurrence wins, atomic save.
- **`error`**: typed errors (`Io`, `Parse`, `Config`; more per phase) so front
  ends can react.

## 3. Data on disk *(P2–P4)*
## 4. Saves *(P4)*
## 5. Running many emulators *(P3)*
## 6. Settings profiles *(P5)*
## 7. Templates *(P6)*
## 8. Identity policies *(P5)*
## 9. Security locks *(P5)*
## 10. Spin-offs, phone mechanics, switchboard *(P7)*
## 11. The ten multi-device features *(P2–P8)*
## 12. Version card *(P2)*
## 13. Safety *(every phase)*

## 14. Dependencies and best practices *(P1)*

| Crate | Where | Why |
| --- | --- | --- |
| `clap` (derive, default features off) | aodm | CLI with nested help |
| `anyhow` | aodm | Error context in the binary |
| `thiserror` | aod-core | Typed errors |
| `jiff` | aod-core | Local-time log timestamps (sound across threads) |
| `tempfile` | dev | Test fixtures |

Bar for a new dependency: it replaces more code than it costs, has a stated
reason here and in [dependencies.md](dependencies.md), passes `cargo deny`
(licences allowlist: MIT, Apache-2.0, Unicode-3.0), and comes from crates.io.

## 15. Testing *(every phase)*

P1: 12 unit tests (config precedence, real framework config sourced, log
format and rotation, lossless ini round trip / set / atomic save). Run:
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

## 18. Known limits (`shortcut:` ledger)

- `env`: built-in fallbacks duplicate `defaults.env` (only used without the framework).
- `log`: rotation can race a concurrent adx run (harmless, one extra generation).
