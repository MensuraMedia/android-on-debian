# Installer (`adx up`)

How `adx up` installs the framework, what it tells you before it changes
anything, what it changes, how to undo it, and where it logs. For every path
on a provisioned machine see [environment.md](environment.md); for the module
contract see [architecture.md](architecture.md).

## 1. Running it

```bash
./adx system status        # preflight; prints the one sudo command if anything is missing
./adx changes              # what every module of the default profile changes on your system
./adx up                   # install the default profile (lists the changes, asks once)
./adx up --yes render-only # another profile, without the question
```

| Form | Effect |
| --- | --- |
| `adx up [--yes] [profile]` | Installs every module of `profiles/<profile>.profile` (default: `default`) that `adx <module> status` doesn't already report ok, in file order. One profile per run; `--` ends options |
| `adx changes [profile]` | Prints each module of the profile with its `MOD_CHANGES` line. Changes nothing |
| `ADX_YES=1` | Same as `--yes` |

Exit codes of `adx up`: `0` done (or nothing to do), `1` a module failed (the
run continues with the next module and ends with `profile incomplete: <list>`)
or a usage error, `2` you declined the prompt (nothing was changed).

Nothing needs root. Every download is checked against a pinned SHA-256 (or a
pinned git commit), and every install is idempotent: rerunning `adx up` skips
what is already ok.

## 2. The notice before installing

Before the first install `adx up` checks every module of the profile and, for
those it still has to install, prints what each one will change:

```text
adx up (default) will install: devices claude-rules console
Changes on your system (no root; undo with: adx <module> remove, where a module has it):
  devices             aodm and aod-gui in ~/.local/bin (install markers in ~/.local/state); menu entry Phone Images + icon …
  claude-rules        edits ~/.claude/CLAUDE.md: inserts or refreshes a marked adx:claude-rules block at the top (rest kept); …
  console             menu entry adx console + icon (~/.local/share/applications, icons/hicolor)
Full list of paths: docs/environment.md. Review any time: adx changes default
Continue? [Y/n]
```

- Each line is also written to `adx.log` (`up notice:`), so the log records
  what a run announced.
- The question is asked only on a terminal. `--yes`, `ADX_YES=1` or a
  non-terminal stdin (CI, a pipe) skip it and proceed.
- If every module is already ok, there is no notice and no question.
- Status checks run with stdin closed, so an answer typed early reaches the
  prompt.

The text comes from each module's `MOD_CHANGES` line (module contract). A
module that doesn't declare one shows `not declared by the module`.

## 3. What it changes

Tools are installed under `~/.local/opt`, `~/.local/bin`, `~/Android/Sdk`,
`~/.android/avd`, `~/.cargo` and `~/.rustup`. Downloads stay in
`~/Downloads/adx` (about 2.5 GB; 1.6 GB of it Android Studio) so a rerun
doesn't download again. Changes to files you may already have:

| File | Change | Module | Undo |
| --- | --- | --- | --- |
| `~/.bashrc` | Adds a marked `adx:shell-env` block (`ANDROID_HOME`, `JAVA_HOME`, `PATH`); links `~/.local/bin/adx` | `shell-env` | `adx shell-env remove` |
| `~/.claude/CLAUDE.md` | Inserts or refreshes a marked `adx:claude-rules` block at the top; the rest is kept | `claude-rules` | `adx claude-rules remove` |
| `~/.claude/agents/` | Adds `adversarial-reviewer.md` and `deploy-reviewer.md`; an edited copy of yours is kept as `.bak` | `claude-rules` | `adx claude-rules remove` |
| `~/.claude.json` | Registers the `mobile-mcp` MCP server (user scope); restart Claude Code afterwards | `mobile-mcp` | `adx mobile-mcp remove` |
| `~/.diagram-design/profiles/` | Installs the flat-dark diagram profile; an edited copy is kept as `.bak` | `skills` | delete the file |
| `~/.local/bin/{node,npm,npx}` | Replaces these links, only when Node.js 20+ is missing | `node` | relink your own Node |
| `~/.local/opt/jdk-21`, `~/.local/opt/android-studio` | Replaced by the pinned versions; an Android Studio of another build is kept as `android-studio.previous-<build>-<time>` | `jdk`, `studio` | — |
| `~/.android/avd/adx_api37.avd/config.ini` | Sets `hw.keyboard` if an AVD of that name already exists | `emulator` | edit the file |
| `~/.local/share/applications/`, `~/.local/share/icons/hicolor/` | Menu entries and icons (section 4) | `console`, `emulator`, `devices` | see section 4 |
| `config/local.env` (this repo) | Emulator GPU settings, on hybrid-GPU laptops only | `emulator` | edit the file |

## 4. Menu entries and icons

| Menu entry | Icon | Launches | Installed by | Removed by |
| --- | --- | --- | --- | --- |
| adx console | `adx-console` (blue, stack of phone images) | `adx console start` (web console window) | `console` (`adx up`) | `adx console remove` |
| Android Emulator | `adx-emulator` (green, phone with a play button in a desktop window) | `adx emulator start --window` | `adx emulator desktop` | `adx emulator desktop remove` |
| Phone Images | `aod-gui` (violet, row of phone cards) | `aod-gui` | `devices` (`adx up`) | `adx devices remove` |

- Icons are installed into the user's hicolor theme
  (`~/.local/share/icons/hicolor/<size>/apps/<name>.png`, 16–512 px, plus
  `scalable/apps/<name>.svg`) by `adx_icon_install` in `lib/common.sh`.
- What adx installed is recorded per icon in
  `~/.local/state/adx/icons/<name>.sha256`. An icon of the same name that you
  made yourself is never replaced (`ADX_FORCE=1` replaces it), and remove
  deletes only files adx installed and nobody changed since.
- The console and Phone Images windows set a window class matching their menu
  entry (`StartupWMClass`), so the panel and Alt+Tab show the same icon. The
  emulator's own window class is not matched yet.
- Sources: `tools/console/static/app-icon/`, `modules/emulator/icon/`,
  `modules/devices/icon/`. Each folder holds `NAME.svg`, an optional
  simplified `NAME-small.svg` for 16–24 px, and the rendered PNGs.
  After editing an SVG run `tools/render-icons.sh [dir…]` (needs
  `rsvg-convert`, apt `librsvg2-bin`), then reinstall the entry.

## 5. Android Studio

The default profile installs Android Studio last (`studio` module), dormant:
no menu entry and no `PATH` change; start it with `adx studio launch`
(output in `~/.local/state/adx/studio.log`). Version, URL, SHA-256 and build
number are pinned in `config/defaults.env` (`ADX_STUDIO_VERSION`,
`ADX_STUDIO_URL`, `ADX_STUDIO_SHA256`, `ADX_STUDIO_BUILD`); `adx studio status`
fails when the installed build differs, so bumping the pins upgrades it on the
next `adx up`. To leave Studio out, use a copy of the profile without `studio`
(then `adx doctor <that profile>` ignores it). Details:
[android-studio.md](android-studio.md).

## 6. Logs

| What | Where |
| --- | --- |
| Every `adx` run (START/END with exit code), every message, the `up notice:` lines | `~/.local/state/adx/adx.log` (`adx logs`) |
| Console server | `~/.local/state/adx/console.log` |
| Android Studio launched by `adx studio launch` | `~/.local/state/adx/studio.log` |
| Emulator and phone logcat | `~/.local/state/adx/emulator-<image>.log`, `logcat-<image>.log` (`adx logs emulator`, `adx logs logcat`) |
| Icon install records | `~/.local/state/adx/icons/<name>.sha256` |

## 7. Writing a module that fits

Declare `MOD_CHANGES` next to `MOD_DESC` with every change outside the
module's own install folder, especially edits to files the user may already
have, and say what happens to their own copy. Start the text with a word, not
`~` (ShellCheck SC2088). `adx changes` shows it; `adx check` runs ShellCheck on
it.
