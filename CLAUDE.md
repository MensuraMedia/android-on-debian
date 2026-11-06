# android-on-debian

A modular bash framework (`adx`) that provisions a CLI-first Android
toolchain on Debian-family Linux and scaffolds Compose + Roborazzi apps.
Read `README.md`, `docs/architecture.md` and `HANDOFF.md` first.

## Rules

- Each capability lives in `modules/<name>/module.sh` and follows the module
  contract (`MOD_DESC`, `mod_status`, `mod_install`, optional `mod_<verb>`).
  Never hard-code versions or paths in modules; add them to
  `config/defaults.env`.
- Installs must be idempotent, must not need root, and must verify downloads
  (SHA-256, or a pinned commit for git sources).
- Scripts run under `set -euo pipefail`: write `if …; then …; fi` rather
  than `test && cmd` as the last statement of a function or loop.
- Record every user-visible change in `CHANGELOG.md` under `[Unreleased]`,
  and update `HANDOFF.md` when the state or next steps change.
- Test changes with `bash -n`, `./adx list`, and, for template changes,
  `adx app-template new` followed by `./gradlew recordRoborazziDebug`.

## Commands

| Task | Command |
| --- | --- |
| Status of everything | `./adx list` / `./adx doctor` |
| Provision | `./adx up [profile]` |
| New app | `./adx app-template new <Name> <package> [dir]` |
| Emulator | `./adx emulator start|stop|screenshot|accel` |
