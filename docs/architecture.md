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
| Verified downloads | Temurin and Gradle are SHA-256 checked; skills are pinned by commit |
| No root by default | Everything goes under `$HOME`; sudo only for the optional `jdk` apt fallback and the `kvm` group |
| Reversible | `shell-env` writes a marked block that `adx shell-env remove` deletes; modules may provide `mod_remove` |
| Composable | Profiles are ordered module lists, so a use case is a new profile file |

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
- the helpers in `lib/common.sh`: `adx_log`, `adx_warn`, `adx_die`,
  `adx_have`, `adx_download`, `adx_sha256_check`, `adx_block_write`,
  `adx_block_remove`, `adx_render_tree`, `adx_run_module`

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
