# android-on-debian

A modular bash framework (`adx`) that provisions a CLI-first Android
toolchain on Debian-family Linux and scaffolds Compose + Roborazzi apps.
Read `README.md`, `docs/architecture.md` and `HANDOFF.md` first.

## Rules

- **Modular, then minimal.** Follow the modularity rules in
  `docs/architecture.md` (one capability per module, contract only, config in
  one place, small core). Inside a module, use the ponytail skill
  (`/ponytail`): the smallest complete change, no speculative options, mark
  known limits with `shortcut: <limit>, <when to upgrade>`. Audit with
  `/ponytail-audit`, list shortcuts with `/ponytail-debt` (skills are vendored
  into apps; in this repo run `adx skills apply .` first if you want them).
- **Diagrams:** use the `diagram-design` skill. Every diagram is **flat dark**
  (profile `aod-flat-dark`, selected by the `.diagram-design` marker): solid
  fills, hairlines, one mint accent, no gradients/shadows/glow, system fonts.
  Source HTML in `docs/diagrams/`, exported SVG next to it, embedded in docs.
- Before calling a change done, run the `adversarial-reviewer` agent on the
  segment you touched and fix or record what it finds.
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
| Emulator | `./adx emulator start [--window] [--wipe]`, `stop`, `status`, `autoconfig`, `desktop`, `screenshot`, `accel` |
| App pipeline | `./adx pipeline run <App> [--serial S] [--stages build,check,test,deploy,smoke]`, `last <App> [--json]`, `list <App>` |
| Logs | `./adx logs [adx\|emulator\|logcat\|dir] [-n N]` (files in `~/.local/state/adx/`) |
| Machine report | `./adx env` |
