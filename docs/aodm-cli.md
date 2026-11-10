# aodm command library: managing images, the manager and its settings

The complete command-line interface of the android-on-debian phone image
manager (`aodm`) and the deployment pipeline around it (`adx`). It is a
specification: every command is listed with its status, **done** (in the
current build, see `aodm --help`) or the roadmap phase that builds it
([ROADMAP.md](../ROADMAP.md)). Internals (algorithms, files on disk, safety
rules) are in [phone-image-manager.md](phone-image-manager.md); this document is
the user-facing contract.

Contents:
1. Scope: three things to manage
2. The twenty management and architecture features
3. CLI conventions (grammar, selectors, global flags, output, exit codes)
4. Command reference: images
5. Command reference: saves, groups, settings, templates, identity, locks
6. Command reference: apps, device operations, system images, spin-offs
7. Command reference: administering the manager
8. Settings reference (every key)
9. The deployment pipeline (`adx`)
10. How the library stays complete and correct
11. Build order

## 1. Scope: three things to manage

| Domain | What | Main commands |
| --- | --- | --- |
| **OS images** | Emulator phone images (AVDs): their lifecycle, data, configuration, identity, apps | `aodm list/start/stop/save/clone/…`, `settings`, `template`, `identity`, `lock`, `app` |
| **The manager** | aodm itself: its health, version, logs, state on disk, storage, audit trail, upgrades | `aodm doctor`, `card`, `logs`, `disk`, `gc`, `audit`, `completion`; `adx devices`, `adx upgrade` |
| **Application settings** | How aodm and adx behave: paths, limits, emulator defaults | `aodm config get/set/unset/list/validate/path`, `aodm env` |

## 2. The twenty management and architecture features

The plan's ten multi-device features, extended to the twenty capabilities a
fleet manager for OS images is expected to have (virtual-machine managers,
device farms and container tools all converge on this set). Each row names the
commands that deliver it.

| # | Feature | Commands | Status |
| --- | --- | --- | --- |
| 1 | **Inventory and status**: what exists, what runs, how big | `list`, `show`, `card` | done |
| 2 | **Lifecycle** for one, a group or all, with budget checks | `start`, `stop`, `restart`, `health` | done |
| 3 | **Provisioning** from templates, in batches | `create --template T --count N`, `template …` | P6 |
| 4 | **Clone, rename, delete** | `clone`, `rename`, `delete` | done |
| 5 | **Backup and restore** (full-copy saves, integrity-checked) | `save create/list/verify/restore/open/pin/delete/prune` | done |
| 6 | **Grouping and selection** (groups, tags, selectors) | `group …`, `tag …`, `NAME… / --all / --group G / --tag K=V / --running` | done |
| 7 | **Configuration management** (profiles, plan/apply, drift) | `settings plan/apply/show/export`, `profile …` | P5b |
| 8 | **Identity policies** (private/standard/fixed) | `identity show/apply/check/regenerate` | P5b |
| 9 | **Policy enforcement** (security locks) | `lock show/set/unlock` | P5b |
| 10 | **Application deployment** | `app install/uninstall/launch/list/clear` | P6 |
| 11 | **Remote execution and capture** | `shell`, `shot`, `record`, `logcat`, `push`, `pull` | P6 |
| 12 | **Health monitoring and watch** | `health`, `watch` | `health` done; `watch` P8 |
| 13 | **Resource management** (ports, RAM/CPU budget, disk quotas, retention) | `start` checks, `disk`, `ADX_AOD_*` limits | done |
| 14 | **Audit trail** (who changed what, when, why) | `audit`, `logs` | done (entries from P5 on) |
| 15 | **Import, export and portability** | `export`, `import`, `spinoff`, `adopt` | P7 |
| 16 | **System image catalogue** (install, remove, check updates) | `sysimg list/add/remove/check` | P6 |
| 17 | **Automation and scripting** (machine output, dry runs, stable exit codes) | `--json`, `--dry-run`, exit codes, `completion` | done |
| 18 | **Self-administration** (doctor, config, version, logs, completion) | `doctor`, `config …`, `card`, `env`, `logs`, `completion` | done |
| 19 | **Upgrades and migrations with rollback** | `adx upgrade [--rollback]`, `adx release` | P9 |
| 20 | **Storage management and garbage collection** | `disk`, `gc`, `move` | done (`move` P6) |

Architecture features that cut across all twenty (section 3): one core
library under every front end (CLI, TUI, GUI), typed errors, structured
logging, locking, idempotent commands, sandboxing.

## 3. CLI conventions

### Grammar

```
aodm [GLOBAL FLAGS] <command> [<subcommand>] [TARGETS] [FLAGS]
```

- Images are the default noun: `aodm start pixel-a` is the same as
  `aodm image start pixel-a` (the `image` group lists them together in `--help`).
- Every other domain is a noun group with verbs: `aodm save restore`,
  `aodm group add`, `aodm config set`.
- Verbs mean the same everywhere: `list`, `show`, `create`, `delete`,
  `apply`, `plan`, `verify`, `export`, `import`.

### Targets (selectors)

| Form | Means |
| --- | --- |
| `NAME…` | these images (unknown names are an error, nothing runs) |
| `--all` | every image |
| `--group G` | images in group G |
| `--running` | narrows to running images (alone: every running image) |
| `--tag K=V` | images whose registry tag K equals V (combines with `--group`) |

The same selector works on every command that takes TARGETS. Commands that
change images report one line per image and, after doing the rest, exit 4 if
some images failed; if all of them failed, with their shared code (all
refused 3, all timed out 5) or 1. In `--json`, each per-image result has
`image`, `ok`, `message` and `code`. No selector at all is a usage
error (exit 2).

### Global flags

| Flag | Effect | Status |
| --- | --- | --- |
| `--help`, `-h` | help for any command or subcommand | done |
| `--version`, `-V` | version and git revision | done |
| `--yes` | skip typed confirmations (required without a terminal) | done |
| `--json` | machine-readable output on stdout (section "Output") | done |
| `--dry-run` | print what would change, change nothing; other commands refuse it (exit 2) rather than run for real | done: `delete`, `wipe`, `save restore/delete/prune`, `gc`, `config set/unset`; `settings` P5b |
| `-q`, `--quiet` / `-v`, `--verbose` | no progress on stderr / also every log line on stderr | done |
| `--no-color` | plain output; aodm prints no colour today, the flag is accepted so scripts can always pass it | done |
| `--sandbox DIR` | run against `DIR/{avd,aod,saves,android}` and log to `DIR/log` instead of the real homes (tests, demos); images whose folder resolves outside DIR are ignored; `config set/unset` are refused; the SDK stays the real one (read only) | done |

### Output

- **stdout** carries results (tables, one line per image); **stderr** carries
  progress (`aodm: …`), warnings and errors. Piping stdout never captures
  progress. A closed pipe (`aodm list | head -1`) is not an error.
- **`--json`**: one JSON document per command, `{"schema": 1, "command":
  "...", "ok": true, "results": [...], "errors": [...]}`; per-image results
  have `image`, `ok`, `message` and command-specific fields. The schema
  version changes only on incompatible changes (documented in CHANGELOG).
- Tables have a header row, are aligned, and never wrap; empty sets say so
  (`no running images`).

### Exit codes

| Code | Meaning | Status |
| --- | --- | --- |
| 0 | success (also: nothing to do) | done |
| 1 | failure | done |
| 2 | usage error (unknown flag, missing argument) | done (clap) |
| 3 | refused on purpose (running image, budget, lock, not confirmed) | done |
| 4 | partial: some targets failed, the rest succeeded | done |
| 5 | timeout | done |

### Confirmations and safety

- Destructive commands ask you to type a word (the image name, `wipe`,
  `delete`, `prune`) or take `--yes`; without a terminal they refuse unless
  `--yes` is given. The prompt names what is destroyed (folder, size).
- Every change to an image holds the image's own lock (the emulator's
  `multiinstance.lock`), so a running image is refused and can't start
  mid-change.
- `--dry-run` is available wherever a command changes several things.
- adx's own image (`ADX_AVD_NAME`) is never renamed or deleted by aodm.

### Logging

Every command logs to `$ADX_LOG_DIR/adx.log` with its process id
(`[aodm #PID]`): what it did, to which image, how long it took, every
failure and every fallback. Read it with `aodm logs` or
`grep 'aodm #' ~/.local/state/adx/adx.log`. Secrets (the console token) are
never logged.

### Idempotency and concurrency

Repeating a command is safe: `start` on a running image reports it running,
`stop` on a stopped one reports it stopped, `apply` changes only what
differs. Concurrent aodm runs coordinate through locks: `ports.lock`
(starts), `images.lock` (registry), each image's `multiinstance.lock`
(changes). Interrupted runs leave only hidden, pid-named work folders, which
the next command removes.

## 4. Command reference: images

| Command | Does | Key flags | Status |
| --- | --- | --- | --- |
| `aodm list [--group G] [--tag K=V]` | table: state, port, API, ABI, tag, revision, device, RAM, size, saves, groups | `--json` | done |
| `aodm show NAME` | everything about one image: paths, config keys, registry entry, saves, last start (locks and identity P5b) | `--json` | done |
| `aodm start TARGETS` | boot (port and budget checked), wait for Android | `--window`, `--cold`, `--wipe`, `--yes` | done |
| `aodm stop TARGETS` | console `kill` (saves quick-boot), then SIGTERM; never SIGKILL | | done |
| `aodm restart TARGETS` | stop, then start what stopped | as `start` | done |
| `aodm health [TARGETS]` | process, adb, boot, console, log errors; exit 1 if any needs attention | `--json` | done |
| `aodm watch [TARGETS]` | live health and resource view, refreshes until Ctrl-C | `--interval S` | P8 |
| `aodm create NAME --template T` | build a new image from a template | `--count N` (NAME-01…), `--group G` | P6 |
| `aodm clone SRC NEW` | full copy under a new name (first boot cold) | | done |
| `aodm rename OLD NEW` | rename; saves and registry entry move along | | done |
| `aodm delete NAME` | remove image and registry entry; saves kept | `--yes` | done |
| `aodm move NAME DIR` | move an image's folder to another disk (rewrites `path=`) | | P6 |
| `aodm wipe NAME` | factory-reset user data (same as `start --wipe`, without booting): removes user data, cache, encryption key and snapshots; keeps config and identity | `--yes`, `--dry-run` | done |
| `aodm net NAME [online\|offline]` | network from the next start: `offline` restricts the emulator's user-mode network (`restrict=on`), so the phone reaches neither the internet nor the host at 10.0.2.2, with nothing changed inside the phone; adb, the emulator console and the window keep working. Without a mode: the setting, and how a running phone was started. Clones of an offline phone are offline | `--json` | done |
| `aodm image …` | the commands of this table, grouped (`aodm image start` = `aodm start`) | | done |
| `aodm tui` | full-screen dashboard (also: `aodm` with no arguments on a terminal) | | P8 |

## 5. Command reference: saves, groups, settings, templates, identity, locks

### Saves (backup and restore) — done

| Command | Does |
| --- | --- |
| `aodm save create IMAGE [--label L]` | full copy of a stopped image with SHA-256 manifest |
| `aodm save list [IMAGE]` | saves with size, pin, system image and emulator revision |
| `aodm save show IMAGE ID` | the manifest |
| `aodm save verify IMAGE ID` | re-hash; reports missing, changed, unexpected files |
| `aodm save restore IMAGE ID [--backup-first] [--yes]` | replace the image's state (verified first, atomic swap) |
| `aodm save open IMAGE ID --as NEW` | the save becomes a new image |
| `aodm save pin IMAGE ID [--off]` | keep forever, exempt from the limit |
| `aodm save delete IMAGE ID [--yes]` | remove one save |
| `aodm save prune IMAGE [--keep N] [--yes]` | remove the oldest unpinned saves |

IDs: exact, a unique prefix (e.g. the date), or `latest`.

### Groups and tags — done

| Command | Does |
| --- | --- |
| `aodm group list` | groups with member counts |
| `aodm group add G NAME…` / `group remove G NAME…` | membership |
| `aodm group delete G` | remove the group (images stay) |
| `aodm tag set NAME K=V…` / `tag unset NAME K…` | free-form labels for `--tag` selection |

### Settings profiles (configuration management) — P5b

| Command | Does |
| --- | --- |
| `aodm profile list` / `profile show P` | built-in and user profiles |
| `aodm profile new P [--from IMAGE]` / `profile validate P` | create (optionally from an image's current values), check |
| `aodm settings show NAME` | current hardware, launch and runtime values |
| `aodm settings plan P TARGETS` | from → to diff per image, nothing changed |
| `aodm settings apply P TARGETS [--restart] [--dry-run]` | apply, read back, report Changed / Unchanged / Skipped (why) / Failed |
| `aodm settings drift P TARGETS` | exit 1 if any image differs from the profile (CI-friendly) |
| `aodm settings export NAME > p.toml` | an image's settings as a profile |

Three classes: **hardware** (`config.ini`, stopped images only; never shrinks
data without `--wipe`), **launch** (gpu, timezone, locale, headless; used at
next start), **runtime** (dark mode, font scale, animations, screen timeout,
stay awake, location, battery, network; over adb while running).

### Templates (provisioning) — P6

| Command | Does |
| --- | --- |
| `aodm template list` / `show T` | built-in (`barebones`, `pixel8-api37`, `small-phone`, `tablet`) and user templates, versioned `T@N` |
| `aodm template copy T NEW` / `edit T` / `delete T` | user templates (built-ins can't be deleted, only overridden) |
| `aodm template validate T` | schema, system image present, profile and identity policy valid |
| `aodm template capture IMAGE T [--golden]` | a template from an image (recipe, or based on a save) |
| `aodm template diff IMAGE` | drift of an image from the template it was built from |
| `aodm template export T` / `import FILE` | share templates |

### Identity policies — P5b

| Command | Does |
| --- | --- |
| `aodm identity show NAME` | controllable, regenerate-on-wipe and fixed fields (values masked) |
| `aodm identity apply TARGETS --policy private\|standard\|fixed` | set phone number, time zone, locale, Android ID, console id, DNS |
| `aodm identity check [TARGETS]` | exit 1 on duplicates between images or host leaks (time zone, locale) |
| `aodm identity regenerate NAME` | new random identity (also done by clone/open) |

### Security locks (policy enforcement) — P5b

| Command | Does |
| --- | --- |
| `aodm lock show [NAME]` | locks per image and what each enforces |
| `aodm lock set NAME LOCK…` | `camera`, `microphone`, `gps`, `sensors`, `network`, `app-install`, `settings`, `identity`, `delete`, `restore`, `wipe`, `adb-shell`, `screen-capture` |
| `aodm unlock NAME LOCK --reason "…"` | the only way to lift a lock; logged with the reason |

## 6. Command reference: apps, device operations, system images, spin-offs

### Apps (deployment) — P6

| Command | Does |
| --- | --- |
| `aodm app install APK TARGETS` | install (or update) on every target, in parallel |
| `aodm app uninstall PKG TARGETS` / `app clear PKG TARGETS` | remove / clear data |
| `aodm app launch PKG TARGETS` | start the launcher activity |
| `aodm app list NAME [--user-only]` | installed packages with versions |

### Device operations (remote execution and capture) — P6

| Command | Does |
| --- | --- |
| `aodm shell TARGETS -- CMD…` | run a shell command on each target; output prefixed by image |
| `aodm shot TARGETS [--out DIR]` | screenshots, one PNG per image |
| `aodm record NAME [--seconds S]` | screen recording |
| `aodm logcat NAME [-- FILTER…]` | device log of one image |
| `aodm push NAME SRC DST` / `pull NAME SRC DST` | files |
| `aodm geo NAME LON LAT` | move the GPS (console `geo fix`) |
| `aodm console NAME -- CMD` *(P7)* | raw emulator console command (`gsm`, `sms`, `power`, `sensor`…) |

### System images — P6

| Command | Does |
| --- | --- |
| `aodm sysimg list` | installed system images with revisions and the images using them |
| `aodm sysimg add API TAG [ABI]` | install through `adx sdk` (asks first) |
| `aodm sysimg remove ID` | refused while an image uses it |
| `aodm sysimg check` | newer revisions available; images whose save snapshots a revision change would invalidate |

### Spin-offs and portability — P7

| Command | Does |
| --- | --- |
| `aodm spinoff IMAGE --form linked\|portable [--name N]` | a standalone phone in `$ADX_AOD_HOME/phones/` with menu entry |
| `aodm adopt DIR` | bring a spun-off phone back under the manager |
| `aodm export NAME FILE` / `import FILE [--as NEW]` | an image (or save) as one archive with manifest |
| `aodm call FROM TO` / `sms FROM TO TEXT` | switchboard between phones |
| `aod-phone run DIR` / `aod-phone control DIR` | run a spun-off phone; mechanics panel |

## 7. Command reference: administering the manager

| Command | Does | Status |
| --- | --- | --- |
| `aodm card` | version card: aodm/adx versions, host, toolchain, emulation, images | done |
| `aodm env` | every setting with its value and source (process, config, fallback) | done |
| `aodm doctor` | self-check: binary matches the framework, config valid and absolute paths, SDK and emulator present, KVM, locks not stale, registry readable and schema known, saves dir on a POSIX filesystem, free space, leftover work folders; exit 1 on problems | done |
| `aodm config list` | the settings aodm reads (as `env`, grouped) | done |
| `aodm config get KEY` | one value (script-friendly) | done |
| `aodm config set KEY VALUE` | write to `config/local.env` (checked, other lines kept, symlink and permissions kept, audited); a data path whose old folder holds data needs `--yes` (move the data first) | done |
| `aodm config unset KEY` | back to the default | done |
| `aodm config validate` | check every key (types, ranges, absolute paths, port slots) | done |
| `aodm config path` | which files are read, in which order | done |
| `aodm logs [-n N] [--follow] [--image NAME [--logcat]] [--level warn]` | aodm's lines from `adx.log` (all rotated generations), or an image's emulator log, or with `--logcat` its logcat (`logcat-NAME.log`, saved from every start, `-v threadtime`, this run's lines only); `--follow` stops at Ctrl-C or, like `tail -f`, at the next line after the reader closes | done |
| `aodm audit [--since DATE] [--image NAME]` | changes only (save, restore, open, pin, delete, clone, rename, wipe, group/tag, config set/unset, gc; later apply, unlock…), with when, who and what; recorded from P5 on | done |
| `aodm disk` | space per image, per image's saves, free per filesystem, leftovers | done |
| `aodm gc [--dry-run]` | remove leftover work folders and temp files of dead runs, empty per-image save folders, and adx's rotated logs (`adx.log.N`, `emulator-*.log.N`, `logcat-*.log.N`) beyond `ADX_LOG_KEEP`; each item is re-checked just before removal; no prompt (it only removes aodm's own garbage), audited | done |
| `aodm completion bash\|zsh\|fish` | shell completion script | done |
| `aodm help [COMMAND]` / `--help` | help | done |
| `adx devices install` / `status` / `remove` | build and install, check, uninstall aodm | done |
| `adx upgrade [vX.Y.Z]` / `adx upgrade --rollback` | update the framework and aodm, run migrations, roll back on failure | P9 |

The manager never needs root: everything it writes is under the user's home
(the paths in section 8).

## 8. Settings reference

Precedence (highest first): `config/local.env`, `config/defaults.env`, then
the process environment for keys left unset there, then built-in fallbacks.
`aodm env` shows the winner and its source. `aodm config set` writes
`local.env`.

| Key | Default | Meaning |
| --- | --- | --- |
| `ANDROID_HOME` | `~/Android/Sdk` | Android SDK (emulator, platform-tools, system images) |
| `ANDROID_USER_HOME` | `~/.android` | Android user directory (`path.rel=` is relative to it) |
| `ANDROID_EMULATOR_HOME` | (unset) | emulator home, if different |
| `ANDROID_AVD_HOME` | `<user home>/avd` | where images live (absolute) |
| `ADX_AOD_HOME` | `~/.local/share/aod` | registry, locks, templates, phones (absolute) |
| `ADX_AOD_SAVES_DIR` | `$ADX_AOD_HOME/saves` | saves (absolute, POSIX filesystem) |
| `ADX_AOD_MAX_SAVES` | `3` | unpinned saves per image |
| `ADX_AOD_BASE_PORT` | `5554` | first console port (even, 5554–5584) |
| `ADX_AOD_MAX_INSTANCES` | `3` | images running at once |
| `ADX_AOD_RAM_RESERVE_MB` | `2048` | host RAM kept free when starting |
| `ADX_AVD_NAME` | `adx_api37` | adx's own image (protected) |
| `ADX_EMU_PORT` | `5554` | its reserved port |
| `ADX_EMU_GPU` | `host` | emulator `-gpu` mode: `host`, `auto`, `auto-no-window`, `software` (Lavapipe), `angle_indirect`, `guest` (the older software renderers are accepted too) |
| `ADX_EMU_HEADLESS` | `true` | no window unless `--window` |
| `ADX_EMU_ENV` | (empty) | `VAR=value` pairs for the emulator process |
| `ADX_EMU_BOOT_TIMEOUT` | `240` | seconds |
| `ADX_LOG_DIR` | `~/.local/state/adx` | `adx.log`, emulator logs and logcat per image |
| `ADX_LOG_MAX_KB` / `ADX_LOG_KEEP` | `5120` / `5` | log rotation |

Per-image settings live in the registry (`$ADX_AOD_HOME/images.toml`:
groups, tags, pinned port, launch overrides, template, origin, locks) and are
changed through the commands above, never by hand while aodm runs.

## 9. The deployment pipeline (`adx`)

Every change to aodm or the framework goes through the same commands:

| Step | Command | Does | Status |
| --- | --- | --- | --- |
| 1. Check | `adx check [--staged] [--full]` | shell, Rust (fmt, clippy, tests incl. fake-SDK integration, cargo deny), docs (links, documented commands exist), process (CHANGELOG/HANDOFF), security (secrets, checksums, pipe-to-shell, owner approval) | done |
| 2. Review | `adx review [--base REF]` | runs the review workflow: adversarial and deploy reviewers per changed segment, findings verified | P9 (today: the agents directly) |
| 3. Release | `adx release X.Y.Z [--dry-run]` | gate `--full`, CHANGELOG version, Cargo version, annotated tag, push to every mirror, local backup | P9 |
| 4. Deploy | `adx upgrade [vX.Y.Z]` | fetch, check out, numbered idempotent migrations, `adx up`, `adx doctor`, automatic rollback on failure | P9 |
| 5. Roll back | `adx upgrade --rollback` | back to the recorded previous tag, `adx up` | P9 |
| Install aodm | `adx devices install` | locked release build, atomic install to `~/.local/bin` | done |
| New module | `adx new-module NAME` | scaffold with contract stubs, logging, docs row | P9 |

The pre-commit hook (`adx devtools hook`) runs `adx check --staged`; CI runs
`adx check` and `aodm health`.

## 10. How the library stays complete and correct

- **One source of truth:** commands are defined once (clap); `--help` and
  completion scripts are generated from it.
- **Gate:** `adx check` fails when a command documented here as done is
  missing from `aodm --help`, or when an `aodm` command exists that this
  document doesn't list (done: the `docs_match_commands` test in
  `crates/aodm/src/main.rs`, run by `adx check`; a row is done when its
  Status cell says `done`, or its section heading ends in `— done`).
- **Contract tests:** every command's `--help` renders (done); the `--json`
  envelope and the exit codes for usage, refused and failed runs are asserted
  against a sandbox (done, `crates/aodm/tests/cli.rs`); per-command JSON
  schemas P9.
- **Integration tests:** every image-changing command runs against a fake
  SDK in a temp directory (done for saves, clone, rename, delete; lifecycle
  with a fake emulator in P9).
- **Reviews:** each phase is reviewed by the adversarial and deploy reviewers
  before it is committed; findings and fixes are recorded in
  phone-image-manager.md §17.

## 11. Build order

| Phase | Adds to the library |
| --- | --- |
| P2–P4 (done) | `list`, `card`, `env`, `start`, `stop`, `restart`, `health`, `save …`, `clone`, `rename`, `delete` |
| **P5a** (done) | conventions: `--json`, `--dry-run`, `--quiet/--verbose`, `--no-color`, `--sandbox`, global `--yes`, exit codes 3–5, `image` group; `show`, `wipe`, `group`, `tag`, `save show`; manager administration: `doctor`, `config`, `logs`, `audit`, `disk`, `gc`, `completion`; the docs/CLI gate |
| **P5b** | `profile`, `settings`, `identity`, `lock`/`unlock` |
| P6 | `create --template`, `template …`, `move`, `app …`, `shell`, `shot`, `record`, `logcat`, `push`, `pull`, `geo`, `sysimg …` |
| P7 | `spinoff`, `adopt`, `export`, `import`, `console`, `call`, `sms`, `aod-phone` |
| P8 | `tui`, `watch` |
| P9 | `adx review`, `adx release`, `adx upgrade [--rollback]`, `adx new-module`; fake-emulator lifecycle tests; v0.3.0 |
