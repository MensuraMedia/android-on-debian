# Architecture

## Overview

```text
                     Claude Code
          shell  /                \  MCP tools
                v                  v
  ┌──────── render loop ───────┐  ┌──────── live check ────────┐
  │ ./gradlew recordRoborazzi… │  │ mobile-mcp (npx, stdio)    │
  │ Roborazzi + Robolectric    │  │ adb (platform-tools)       │
  │ PNG → app/build/outputs/…  │  │ headless emulator, API 37  │
  └────────────────────────────┘  └────────────────────────────┘
          \                                  /
           ~/Android/Sdk  ·  ~/.local/opt/jdk-21  ·  Gradle wrapper
```

The render loop needs no device and takes seconds. The live check is used
only when runtime behaviour matters. Both read the same SDK and JDK.

## Design principles

| Principle | How it shows up |
| --- | --- |
| Configuration over code | All versions and paths are in `config/defaults.env`; machine overrides go in `config/local.env` |
| One capability per module | `modules/<name>/module.sh`, run in a subshell so modules can't leak state |
| Idempotent | `mod_status` gates `mod_install`, so `adx up` can be re-run safely |
| Verified downloads | Temurin, cmdline-tools, Gradle (and app wrappers), Node, rustup-init, ShellCheck and cargo-deny are SHA-256 checked; skills are pinned by commit; a bad cached download is deleted |
| No root | Everything goes under `$HOME`; nothing runs sudo, the `system` module prints the commands that need it (apt packages, `kvm` group) |
| Reversible | `shell-env` writes a marked block that `adx shell-env remove` deletes; modules may provide `mod_remove` |
| Composable | Profiles are ordered module lists, so a use case is a new profile file |
| Minimal inside each module (ponytail) | The smallest complete change: no options, wrappers or "for later" code nobody needs; deletion beats addition; known limits marked `shortcut: <limit>, <when to upgrade>` |
| Logged | Every `adx` run and message goes to `$ADX_LOG_DIR/adx.log`; long-running processes keep their previous log |
| Detect, then configure | Machine quirks are detected (`system` preflight, `emulator autoconfig`) and written to `config/local.env`, never hard-coded |
| Never sudo | Modules that need root print the exact command instead (`system`) |

### Modularity rules

The framework is strictly modular. A change that breaks one of these rules
needs a reason in the commit message.

1. **One capability, one folder.** A new tool or concern is a new
   `modules/<name>/`, never a branch inside another module. A module may grow
   verbs (`mod_<verb>`) only for its own capability (e.g. `emulator desktop`).
2. **Contract only.** The dispatcher talks to modules only through
   `mod_status` / `mod_install` / `mod_remove` / `mod_<verb>`. Modules call
   each other only through `adx_run_module <name> <action>`, never by
   sourcing each other's files.
3. **No shared mutable state.** Modules run in subshells; anything that must
   persist goes to `config/local.env` (`adx_local_set`) or a file the module owns.
4. **Configuration in one place.** Versions, paths and defaults live only in
   `config/defaults.env`; machine values only in `config/local.env`.
5. **Core stays small.** `adx` and `lib/common.sh` hold only dispatch,
   config, logging and helpers used by two or more modules. `adx env` and
   `adx logs` are core because they report on all modules.
6. **Profiles compose.** A use case is a profile (ordered module list), not
   code: `system` first, dependencies before dependents (`node` before
   `mobile-mcp`).
7. **Each module is checkable alone.** `adx <name> status` must work without
   any other module installed, and say what is missing.

## Module contract

```bash
# modules/<name>/module.sh — sourced by adx_run_module in a subshell
MOD_DESC="One-line description shown by adx list"

mod_status()  { ...; }   # print state; return 0 when installed/healthy
mod_install() { ...; }   # make mod_status succeed; must be idempotent
mod_remove()  { ...; }   # optional
mod_<verb>()  { ...; }   # optional; exposed as: adx <name> <verb> [args]
```

What a module can use:

- every variable from `config/defaults.env` and `config/local.env` (exported)
- `$ADX_ROOT`, `$MOD_NAME` and `$MOD_DIR`
- the helpers in `lib/common.sh`: `adx_log`, `adx_warn`, `adx_die` (all three
  also write to the framework log), `adx_logfile`, `adx_log_rotate`,
  `adx_log_shift`, `adx_have`, `adx_download`, `adx_sha256_check`,
  `adx_block_write`, `adx_block_remove`, `adx_block_present`,
  `adx_render_tree`, `adx_local_set`, `adx_run_module`

## How changes are made (plan → design → modular development)

Two tracks, kept separate:

- **Apps built or modified with the framework** (the compose-roborazzi apps,
  and APKs handled by the app library) follow **app best practices**: their
  own `CLAUDE.md` and pinned skills, the pipeline's definition of done
  (`adx pipeline run`), and, for third-party APK work, the engineering
  practices in [docs/app-library.md](app-library.md) §1.1. This is about the
  software being produced.

- **Any enhancement or modification to the adx console, the emulator, or
  their properties** — the framework itself — follows a fixed process, not
  ad-hoc edits:

  1. **Plan.** Agree the scope and phases first (an approved plan, as the
     phone-image manager and app library have in ROADMAP.md). No framework
     feature starts as code.
  2. **Design.** Write the design down before building — a technical document
     (e.g. [docs/phone-image-manager.md](phone-image-manager.md),
     [docs/app-library.md](app-library.md)) that states the modules, config,
     contracts and limits. The design is reviewed and only then built.
  3. **Develop under strict modular principles.** Implement to the Modularity
     rules and Module contract above and the ponytail minimalism in the
     project `CLAUDE.md`: one capability per module, contract-only between
     modules, config in `defaults.env`, core stays small, the console and
     other front ends are thin layers over `adx` CLIs. A change that breaks a
     rule carries its reason in the commit message.
  4. **Check and review.** Each phase ends with `adx check` and the
     adversarial and deployment reviews, with findings fixed or recorded,
     before it is called done.

This is why console and emulator capabilities live behind `adx` modules and
CLIs rather than inside the web server or the AVD config: the plan and design
decide the module boundaries, and development keeps them.

## Config precedence

1. `config/defaults.env` (committed)
2. `config/local.env` (git-ignored; copy `config/local.env.example`)

Both files are sourced by `adx` and by `lib/shell-env.sh`. A value you
change there reaches new shells without editing `~/.bashrc` again.

## Templating

`adx_render_tree <src> <dst>` copies a template tree and replaces
`@@VAR@@` with the value of the environment variable `VAR`, in file
contents and in paths. Unknown tokens are left as they are, and binary files
are copied untouched.

The app scaffolder exports `APP_NAME`, `APP_PACKAGE` and `APP_PACKAGE_PATH`
on top of the config variables.

## App template: compose-roborazzi

| Piece | Detail |
| --- | --- |
| Build | AGP 9.5.1 with built-in Kotlin, Kotlin 2.4.21 Compose compiler plugin, Gradle 9.8.2 wrapper, version catalog |
| UI | Compose BOM 2026.09.00, Material 3, `HomeScreen` with state passed in as a parameter |
| SDK levels | compileSdk 37, targetSdk 36, minSdk 26 |
| Screenshots | Roborazzi 1.77.0 + Robolectric 4.17 (SDK 36, native graphics, Pixel 7 qualifiers) |
| JVM flag | `--add-opens=java.base/jdk.internal.access=ALL-UNNAMED`, which Robolectric on SDK 36 needs under JDK 21 |
| Agent files | `CLAUDE.md`, `.claude/settings.json` (allowlist), `.claude/skills/` (pinned) |
