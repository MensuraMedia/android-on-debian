# App library: staged APKs, modification and promotion

Technical design for the **app library**: the place where every APK the
environment builds, imports or modifies is kept, grouped per app into the
stages **dev**, **test** and **prod**, and moved between them. It replaces the
flat `apps/` folder of the console's Assets page and is the APK half of the
"source safe": app source stays in each app's git repository; the APKs built
from it, and APKs that come from outside, live here.

Status: **Planned** (see [ROADMAP.md](../ROADMAP.md), phase L). Nothing in
this document is built yet. Written 2026-11-10.

How it must be built (modularity, best practices): §1.1.

## 1. Purpose and use cases

The console is an image console plus an agentic developer pipeline (see
[app-development-and-release.md](app-development-and-release.md) §1). The
library gives every APK in that loop a home and a history:

| # | Use case | Flow |
| --- | --- | --- |
| U1 | Own app, normal development | Claude changes source → `adx pipeline run` → the built APK lands in **dev** → a passing run lets it be promoted to **test** → a release build (signed) goes to **prod** |
| U2 | Post-APK modification | An APK in **prod** (an own release or a third-party APK under evaluation) is **opened**: decoded into a workspace → changed (by Claude or by hand) → **rebuilt** and re-signed → sent back to **test**, **dev** or both as a new artifact whose parent is the original |
| U3 | Third-party APK under evaluation | An APK from outside (download, pulled from a phone) is **imported** with its provenance, inspected (identity, permissions, ABIs, signer), installed on a test phone, compared with other versions |
| U4 | Regression and rollback | Any earlier artifact of an app can be installed again on any phone; two artifacts can be **diffed** (manifest, permissions, resources, code) |
| U5 | Claude reads the state | Every command has `--json`; Claude decides the next step from the library and pipeline records, not from screenshots |

Current test case for U2/U3: Pinterest 14.43.0 (third-party, evaluation only,
not for deployment). It exercises import of a large single APK, decoding,
rebuilding, re-signing and the side effects of a changed signer (§6.4).

### 1.1 How this must be built (binding on every phase)

The app library **is** a modification of the adx console and its properties,
so it follows the framework change process in
[docs/architecture.md](architecture.md) ("How changes are made"): this
document is its **design** step; development is then expressed through the
**strict modular principles** below — the Modularity rules and Module contract
in that same doc, and the ponytail minimalism in the project `CLAUDE.md`. Any
phase that breaks a rule here needs a reason in its commit message, and ends
with `adx check` plus adversarial and deployment review.

(The APKs this feature handles are a separate track: the *apps* follow app
best practices — the engineering practices at the end of this section. The
*process of building the console feature* follows the modular principles
first.)

**Strict modular principles (the framework rules, applied here):**

- **One capability per module.** The library is a new `modules/library/`
  (store, stages, promote, install, gc) and the APK toolchain a new
  `modules/apk-tools/` (install apktool/jadx, decode, rebuild, diff). Neither
  is a branch inside `console` or `pipeline`. They talk to other modules only
  through `adx_run_module` / the `adx` CLI, never by sourcing each other.
- **CLI first, console is a thin front end.** Every operation is an
  `adx library` / `adx apk` command with `--json`; the console's Libraries
  page only calls those commands and renders the result. No library logic
  lives in `server.py`. This is what lets Claude drive the whole loop from
  machine-readable output (U5) and keeps the UI replaceable.
- **The console itself stays modular.** `tools/console/server.py` is already
  large (~850 lines); the Libraries page must not grow it into a monolith.
  Library endpoints go in their own handler unit (e.g. `tools/console/
  library.py`, as `assets.py` is split out today), the server only routes to
  it, and the browser view is its own template/module. Decomposing the
  existing server as needed is part of this work, not a later cleanup.
- **Configuration in one place.** Every path, version, checksum and limit is
  in `config/defaults.env` (`ADX_LIBRARY_*`, `ADX_APKTOOL_*`, `ADX_JADX_*`);
  machine values in `config/local.env`. None are hard-coded in a module.
- **Minimal (ponytail).** The smallest complete thing per phase: no stage,
  flag or format nobody needs yet. Split-APK support, lineage graphs and
  export are later phases (§14), marked as limits (§13), not pre-built.
  Deletion beats addition; known limits carry `shortcut: <limit>, <when to
  upgrade>`.
- **Reversible and logged.** Modules provide `mod_remove`; every action is
  logged to `$ADX_LOG_DIR/library.log` and appended to `history.jsonl`, per
  the global logging rule.

**App-track best practices (for the APKs handled, beyond the modular
principles above):**

- **Content-addressed, immutable artifacts.** Every APK is keyed by its
  SHA-256 and never edited in place (§2, §4): the identity is the bytes, so a
  mis-named or duplicate file cannot mislead (§10).
- **Provenance and lineage are mandatory, not optional.** Where each artifact
  came from and what it was built from is recorded at intake and carried
  through modification (§4), so any build can be traced to a source or a
  parent.
- **Reproducible, deterministic builds.** Decode and rebuild are mechanical
  and tool-driven (§6.3), run the same way every time, with tool versions
  pinned to a checked minimum; no hand steps in the normal path.
- **Untrusted input is sandboxed** (§12): decode runs with no network and
  path-sanitised names, and decoded content is treated as data, never as
  instructions.
- **Signing hygiene** (§6.4): keys outside the library, passwords via file
  not command line, consequences of a changed signer surfaced on every
  modified artifact.
- **Tested like the pipeline.** Each phase ships with tests (Rust unit tests
  or shell `bats`-style checks as the rest of the repo uses), exit codes
  matching the pipeline, and is verified end to end on the real emulator
  before it is called done.

## 2. Concepts

| Term | Meaning |
| --- | --- |
| **App** | One library entry per Android package name (`com.example.app`). Created on the first artifact for that package |
| **Artifact** | One APK file, immutable once stored, identified by its SHA-256 (short id: first 8 hex digits). Never edited in place: a change produces a new artifact |
| **Stage** | `dev`, `test` or `prod`. An artifact can be in one or more stages at once (one file on disk, several stage entries) |
| **Provenance** | Where an artifact came from: `pipeline` (built from an own repo, with commit and run id), `import` (with source note and original file name), `pulled` (from a phone, with serial), `modified` (with parent artifact and workspace commit) |
| **Ownership class** | `own` (source in an own repo), `licensed` (third-party, modification allowed by its license, e.g. open source), `evaluation` (third-party, local testing only). Set on import, inherited by modifications. `evaluation` artifacts are marked *not distributable* everywhere they appear and can never be the input of a release |
| **Workspace** | A decoded copy of an artifact (apktool output) in a local git repository, where modifications are made |
| **Lineage** | The parent → child chain of artifacts created by modification and rebuild |

## 3. Storage layout

Under `ADX_LIBRARY_DIR` (new, default `${XDG_DATA_HOME:-$HOME/.local/share}/adx/library`),
mode 700, files 600, same rules as the pipeline directory:

```
library/
  <package>/
    app.json                     # app record: package, label, ownership, notes
    artifacts/
      <sha256>.apk               # the only copy of each APK
      <sha256>.json              # artifact record (§4)
    dev/  test/  prod/           # stage folders: hard links named
      <name>.apk                 # human-readable name (§3.1); same inode as artifacts/
    workspaces/
      <name>/                    # decoded workspace, a git repo (§6); same <name> as its APK
    history.jsonl                # append-only log of every library action
    .lock                        # per-package lock, held only while writing records
```

Writes take the per-package `.lock` (flock, like the pipeline) and hold it
only while updating JSON records, never during decode, build or an install.
All records are written to a temp file and `os.replace`d into place (atomic,
as `pipeline.py` does), so reads need no lock. Stored APKs are mode **400**:
the owner cannot change an artifact in place from a file manager.

### 3.1 Names: version last

Every stage file, workspace and rebuilt APK is named with the version
number as the **suffix**, so names sort by app and read the same everywhere:

```
<label>-<shortid>-v<versionName>.apk          original or built artifact
<label>-<shortid>-v<versionName>+m<N>.apk     N-th modification of that version
<label>-<shortid>-v<versionName>/             its decompiled workspace
<label>-<shortid>-v<versionName>+m<N>/        workspace opened from a modification
```

- `<label>`: the app's label from the APK, lower-case, spaces and anything
  outside `[a-z0-9._]` turned into `-` (falls back to the last package segment).
- `<shortid>`: first 8 hex digits of the SHA-256, so two different builds of
  the same version never collide.
- `+m<N>`: modification revision, counted per original version (1, 2, …);
  the APK's own `versionName`/`versionCode` are left unchanged unless the
  change itself edits them (`apktool.yml`), in which case the new version is
  used and `+m` restarts at 1.
- Examples: `pinterest-3f9e21ab-v14.43.0.apk`,
  `pinterest-3f9e21ab-v14.43.0+m2.apk`, workspace
  `pinterest-3f9e21ab-v14.43.0+m2/`.

- Stage folders are real folders the owner can open in a file manager; the
  files in them are hard links, so an artifact in two stages uses its space
  once. The owner's "production folder" is `library/<package>/prod/`. Where
  `link()` fails (EXDEV across file systems, EPERM on exFAT/some FUSE mounts)
  the library falls back to a symlink into `artifacts/` and notes it.
- The JSON records are the source of truth. `adx library verify` re-hashes
  every `artifacts/` file against its record; `--repair` rebuilds the stage
  links from the records and moves any file dropped into a stage folder by
  hand into `inbox/` (it never deletes an unknown file).

## 4. Artifact record

`artifacts/<sha256>.json`, written once at intake, extended only with stage
history and notes:

```json
{
  "sha256": "…", "size": 123456789,
  "package": "com.example.app", "label": "Example",
  "version_name": "1.4.0", "version_code": 140,
  "min_sdk": 26, "target_sdk": 36, "abis": ["arm64-v8a", "x86_64"],
  "debuggable": false,
  "signer": {"sha256": "…", "subject": "CN=…", "scheme": ["v2", "v3"]},
  "permissions": ["android.permission.INTERNET", "…"],
  "ownership": "own | licensed | evaluation",
  "provenance": {
    "kind": "pipeline | import | pulled | modified",
    "repo": "…", "commit": "…", "run_id": "…",
    "source": "…", "original_name": "…", "serial": "…",
    "parent": "<sha256>", "workspace_commit": "…", "change_note": "…"
  },
  "created": "2026-11-10T17:00:00Z",
  "stages": [{"stage": "dev", "at": "…", "by": "pipeline|console|cli", "reason": "…"}],
  "gates": [{"run_id": "…", "stages_passed": ["deploy", "smoke"],
             "apk_sha256": "…", "git_dirty": false, "at": "…"}],
  "notes": "…"
}
```

Identity comes from `aapt2 dump badging` (already used by the console's
`apk_info`). The `signer` field is a **list** (v3 key rotation means an APK
can carry more than one signer), read from `apksigner verify --verbose
--print-certs`. Both tools are in build-tools 37, already installed. `gates`
is filled by §7 when the artifact is promoted, copied out of the pipeline run
**before** the run can be pruned, so a later promotion never depends on a run
record that `ADX_PIPELINE_KEEP` has since deleted.

## 5. Operations

All operations are an `adx library` CLI first (module `library`), with
`--json` output and the exit codes of the pipeline (0 ok, 1 failed, 2 usage,
3 locked). The console calls the same CLI.

| Command | Does | Rules |
| --- | --- | --- |
| `import FILE --stage S [--ownership C] [--source TEXT]` | Verifies the file is an APK, reads identity and signer, stores it, puts it in stage S | `--ownership` required for a package not yet in the library; same SHA-256 already present → only the stage is added |
| `pull SERIAL PACKAGE --stage S` | Copies an installed app off a running emulator (`pm path`, `adb pull`) and imports it | Split installs (several `pm path` lines) are refused until §9 L4 |
| `list [PACKAGE] [--stage S]` | Apps, or one app's artifacts per stage | |
| `show ARTIFACT` | Full record, lineage, where it is installed now | ARTIFACT = short id, full SHA-256 or `PACKAGE@stage` (newest in that stage) |
| `promote ARTIFACT --to test\|prod` | Adds the stage | Gates in §7 |
| `send-back ARTIFACT --to dev[,test]` | Puts an artifact (normally from prod) into test and/or dev as the base for further work | Records reason; does not remove it from prod |
| `unstage ARTIFACT --from S` | Removes a stage entry | The file is deleted only by `gc` when no stage and no child refers to it. `--from prod` also needs `--yes --force`; Claude may not run it |
| `install ARTIFACT [--serial S]` | `adb install -r -t [-d]` on the running phone (asks which when several run, like the pipeline). `-d` is added only for an allowed version downgrade (U4) | Emulators only (aodm-reported). Compares the installed signer (`dumpsys package`) with the artifact first; a mismatch (`INSTALL_FAILED_UPDATE_INCOMPATIBLE`) needs an uninstall, which wipes that app's data, and the console asks before it — in both directions |
| `open ARTIFACT` | Creates a workspace (§6) | Refused for split APKs and for APKs over `ADX_LIBRARY_DECODE_MAX_MB` |
| `rebuild WORKSPACE --to dev\|test [--note TEXT]` | Commits the workspace, builds, aligns, signs, imports the result as a `modified` artifact | §6.2 |
| `diff A B` | Compares two artifacts: identity, permissions, signer, manifest, resources, file list, and (when both are decoded) smali | Output as text or `--json` |
| `verify [--repair]` | Re-hashes files, checks records against stage folders and links | |
| `gc [--dry-run]` | Deletes artifacts with no stage and no child, and old workspaces | Keeps any artifact whose **history** shows it was ever in prod (checked in `history.jsonl`, not just the current stage). Skips a workspace whose `git status` is dirty or that has not been rebuilt, so an uncommitted modification is never lost |
| `rm ARTIFACT --yes` | Deletes an artifact and its stage entries | Refused while it has children, unless `--with-children`. An artifact currently or ever in prod also needs `--force`; Claude may not run `rm` on a prod artifact |

Every action appends one line to `history.jsonl` and logs to
`$ADX_LOG_DIR/library.log` (tags `Lib/Import`, `Lib/Stage`, `Lib/Build`,
`Lib/Install`), per the project's logging rules.

## 6. Modification (post-APK)

### 6.1 Open

`adx library open ARTIFACT`:

1. Decodes the APK with **apktool** (`apktool d --no-debug-info` style
   options decided at implementation) into `workspaces/<name>/src/`:
   `AndroidManifest.xml`, `res/`, `smali*/`, `assets/`, `lib/`, `apktool.yml`.
2. Optionally (`--java`) also writes a read-only Java view with **jadx** into
   `workspaces/<name>/java/` for reading; changes are made in smali and
   resources, because jadx output does not rebuild.
3. Initialises a git repository in the workspace, commits the untouched
   decode as `base <name>`, and writes a `CLAUDE.md` in the workspace that
   tells Claude what the workspace is, which files to change, how to rebuild,
   and the artifact's ownership class.

Claude Code then works in the workspace folder like in any project. The git
history is the record of what was changed and why.

### 6.2 Rebuild

`adx library rebuild WORKSPACE --to dev|test`:

1. Refuses if the workspace has no changes against its base commit.
2. Commits pending changes (message from `--note`).
3. `apktool b` → unsigned APK → `zipalign -P 16 -f 4` (16 KB native-library
   alignment for API 35+ images, which 4 KB would break) → `apksigner sign
   --ks-pass file:<600-perm file>` with the **library signing key** (§6.4),
   never `pass:` on the command line (it shows in `ps`) → `apksigner verify
   --verbose`.
4. Imports the result with provenance `modified` (parent, workspace commit,
   note) into the requested stage(s). Ownership is inherited from the parent.
5. Optionally (`--pipeline`) runs the pipeline's `deploy,smoke` stages
   against the new artifact (§8).

Build logs go to `workspaces/<name>/build/<time>.log`; a failed build
leaves the workspace as it was and returns exit 1 with apktool's errors
quoted, the way the pipeline quotes Gradle errors.

### 6.3 Decompile utility (token-saving)

Decoding, indexing and rebuilding are mechanical: the utility does them, so
Claude never spends tokens running apktool by hand, reading tool output, or
browsing a decoded tree that can hold 100,000+ smali files. It works on any
APK file, inside or outside the library:

| Command | Does |
| --- | --- |
| `adx apk decompile FILE\|ARTIFACT [--java]` | Reverts the APK to its modifiable form (§6.1 steps 1–3) and writes the indexes below. Cached by SHA-256: a second decompile of the same APK reuses the workspace (`--fresh` starts over). A plain file is imported to the library first (`--stage`, `--ownership` as for `import`) |
| `adx apk recompile WORKSPACE --to dev[,test]` | The rebuild of §6.2: build, align, sign, verify, import as a `modified` artifact into dev and/or test |
| `adx apk find WORKSPACE PATTERN [--in manifest\|res\|strings\|smali\|java] [--app-only]` | Bounded search (default 50 hits, file:line plus one line of context) |
| `adx apk class WORKSPACE NAME` | Path of one class's smali file (and Java view when present), with its methods listed, without opening the rest |
| `adx apk status WORKSPACE` | Changed files against the base commit and whether it rebuilds; `--json` |

Indexes written into the workspace at decompile time, so Claude reads a few
kilobytes instead of the tree:

- `INDEX.md`, the entry point named in the workspace `CLAUDE.md`: identity
  (package, version, SDKs, ABIs, signer), permissions, components
  (activities with the launcher one marked, services, receivers, providers,
  exported flags), network security config, native libraries, size of
  each top-level package, and which packages are the app's own code versus
  known libraries (`androidx`, `kotlin`, `com.google`, `okhttp3`, …, from a
  list in the module).
- `index/classes.tsv` (class → file → method count), `index/strings.tsv`
  (resource name → value, default locale only), `index/layouts.tsv`
  (layout → view ids).
- `.claudeignore`-style guidance in the workspace `CLAUDE.md`: read
  `INDEX.md` first; use `adx apk find`/`class` instead of listing folders;
  never read library packages or `res/values-*` locales unless the task
  names them.

After a change Claude runs one command, `adx apk recompile … --to test
--pipeline`, and reads the short result record (artifact id, pipeline
verdict), the same way it reads `adx pipeline last`.

### 6.4 Signing, and a like-for-like baseline

A modified APK can never carry the original signature: the signature is a
cryptographic hash of the APK's contents signed with the developer's private
key, which the library does not hold, so any change invalidates it and the
APK must be re-signed with a different key. No tool or flag preserves it.
The library signs with its own keys, kept **outside** the library under
`ADX_LIBRARY_KEYS_DIR` (default `${XDG_DATA_HOME:-$HOME/.local/share}/adx/keys`),
created with `keytool` (from the JDK) on first rebuild:

| Key | File (mode 600, dir 700) | Used for |
| --- | --- | --- |
| Library dev key | `$ADX_LIBRARY_KEYS_DIR/library-dev.p12`, password in `library-dev.pass` (600) | All rebuilt artifacts |
| App release keys | Per own app, never stored by the library (see app-development-and-release.md §6) | Only the future release stage |

**The re-signed baseline (for a 1-to-1 comparison).** Because a re-signed
build can behave differently in signer-bound code paths, comparing the
modified APK against the *store original* is not apples-to-apples. So every
`decompile` also produces a **baseline**: the unmodified APK re-signed with
the same library dev key, imported as its own artifact
(`…-v<version>+m0`, provenance `modified` with an empty change set). The
comparison flow and `adx apk diff --vs-baseline` pair the modified artifact
against this baseline, not the store original, so the only difference between
the two is the change itself — both are self-signed with the same key and
both are affected equally by any signer-bound behaviour.

Consequences the library must show next to every `modified` artifact, and
which behaviour is comparable:

- **Comparable (signer-independent):** the app's own network calls, the feed
  and its contents, UI, and local functions. These do not look at the APK
  certificate, so the modified build and the re-signed baseline behave
  identically except for the change. Email/password login works too, because
  it authenticates against the app's own account system, not the certificate.
- **Not comparable (signer-bound), flag, do not read as a difference the
  change caused:** Google sign-in and Google APIs keyed to the certificate
  fingerprint, Play Integrity / server-side attestation, licence checks, and
  an app's own tamper checks. `INDEX.md` lists which of these the app uses
  (from its manifest and libraries) so they are marked off-limits for the
  comparison. If the app gates its *API* behind attestation, its servers can
  refuse any re-signed build and the feed will not load — that feature is then
  untestable on a modified build, on either side.
- It cannot be installed **over** the original (signer differs,
  `INSTALL_FAILED_UPDATE_INCOMPATIBLE`): install needs an uninstall first,
  which deletes the app's data on that phone. The console asks before that.
- An `evaluation` artifact stays local: it can be staged, installed on local
  emulators and diffed, never released or exported.

### 6.5 Testing a modified feed

To confirm a feed change works and to compare before/after on the same
account (the feed is live data, so the phone stays **online** for this; do
not use offline mode):

1. Install the re-signed **baseline**, sign in with **email/password** (not
   Google — that path is signer-bound), capture the feed. Then install the
   **modified** artifact the same way on the same image (or a duplicate of
   one baseline save), one at a time, and capture again.
2. To read the feed itself rather than just the screen, one legitimate
   decompile edit — adding a `networkSecurityConfig` that trusts a
   user-installed CA — lets the phone's traffic be routed through an HTTPS
   proxy (mitmproxy) on the host, so the feed JSON can be read and diffed
   directly, baseline vs. modified. This edit is itself tracked as a
   modification in the workspace.
3. Features the manifest shows are signer-bound (step 6.4) are recorded as
   not-comparable rather than chased.

## 7. Stage gates

| Move | Gate |
| --- | --- |
| → dev | None (intake) |
| dev → test | `own`: a pipeline run **for this exact artifact** (`run.apk_sha256` == the artifact) passed build, check, test, deploy, smoke, with `git_dirty=false`. Others: a `deploy,smoke` run for this artifact passed |
| test → prod | `own`: built by the (future) release stage, release-signed, `debuggable=false`, versionCode higher than the current prod artifact. `licensed`/`evaluation`: a passing `deploy,smoke` run and an explicit `--reason`, recorded |
| prod → test/dev (`send-back`) | None; reason recorded |

At promote time the gate **copies** the run's evidence (`run_id`,
`stages_passed`, `apk_sha256`, `git_dirty`) into the artifact's `gates`
field (§4), so the decision never depends on a run that
`ADX_PIPELINE_KEEP` later prunes. A requested stage that was `skipped`
(not `passed`) does not satisfy a gate.

`--force` skips a gate, records that it did, and is never used by Claude
without the owner saying so (the template's `CLAUDE.md` says this).

## 8. Pipeline integration

- `adx pipeline run <App>`: when the run **finishes** (not merely when deploy
  passes), the built APK is imported into **dev** with provenance `pipeline`
  (repo, commit, run id) and the run's final status. An APK whose smoke stage
  failed is in dev but does not satisfy the dev → test gate.
- New input form `adx pipeline run --artifact ID [--stages deploy,smoke]`:
  runs deploy and smoke against a library artifact (own or not), so modified
  APKs are tested the same way as built ones. These runs have no gradlew and
  no app folder, so they are filed under `$ADX_PIPELINE_DIR/lib-<package>/`.
  The pass result, matched by `apk_sha256`, is what the dev → test gate reads.
- The future **release** stage writes into **prod** (with the release
  signer); until then nothing reaches prod for an own app except by `--force`.

## 9. Console: Libraries page

The Assets page's "Apps" section becomes a **Libraries** page:

- Left: list of apps (label, package, ownership badge, newest version per
  stage).
- Main: three columns **Dev | Test | Production**. Each card: label,
  version, short id, signer (short), provenance line (e.g. "built from
  HelloAdx a1b2c3, run 20261110-…", "modified from 3f9e…: network
  security config for traffic inspection", "imported, evaluation"), and where it is installed now.
- Card actions: Install on phone · Promote · Send back (Test / Dev / both) ·
  Open for modification · Diff with… · Show history · Delete. Rebuild appears
  on cards that have an open workspace.
- Upload goes through `import` (asks stage and ownership). The existing
  Assets `apps/` files are imported once by a migration (§10).
- Lineage view: a small tree for one app showing parents and children.

## 10. Migration

On first run, `adx library migrate` imports every APK in
`ADX_CONSOLE_ASSETS_DIR/apps/` into **dev** (not test: migration passes no
gate; the stage entry records `by: migrated`) with ownership `evaluation`
and source "assets library", stores each mode 400, and leaves the old files
in place (the Assets page stops listing APKs). The duplicate and mis-named APK issue found
on 2026-11-10 (`uptodown-com.brave.browser.apk` is Uptodown) resolves itself:
artifacts are keyed by content and shown by their real package and label.

## 11. Tools and module

New module `apk-tools` (installed by the default profile), versions and
checksums in `config/defaults.env`:

| Tool | Use | Source |
| --- | --- | --- |
| apktool (jar) | decode / build | GitHub release, SHA-256 pinned |
| jadx | read-only Java view | GitHub release, SHA-256 pinned (optional) |
| aapt2, zipalign, apksigner | identity, align, sign, verify | build-tools 37 (already installed) |

apktool and jadx are pinned to a **minimum** version with a known fix for
their decode path-traversal CVEs; an older binary is refused.

New config: `ADX_LIBRARY_DIR`, `ADX_LIBRARY_KEYS_DIR`,
`ADX_LIBRARY_DECODE_MAX_MB` (default 400, checked against the APK size),
`ADX_LIBRARY_KEEP_WORKSPACES` (default 10), `ADX_APKTOOL_VERSION`/`_SHA256`
(a minimum), `ADX_JADX_VERSION`/`_SHA256`.

## 12. Security

An imported APK is untrusted input, and everything inside it — strings, asset
files, resource names — is attacker-controlled data, never instructions.

- **Decode is sandboxed.** apktool and jadx have had path-traversal and
  similar decode CVEs, so decoding runs as an unprivileged step with **no
  network** (`bwrap --unshare-net`, `--frame-path` kept inside the workspace)
  and a pinned minimum tool version. Nothing from a workspace is executed on
  the host: rebuild runs only apktool, zipalign and apksigner.
- **Path safety.** The package name is checked against the regex already in
  `assets.py` and the label is reduced to `[a-z0-9._-]` before either is used
  in a directory name or filename, so a crafted label cannot escape the
  library tree.
- **Prompt-injection hygiene.** A decoded tree can contain a nested
  `CLAUDE.md`, `.claude/` or `.mcp.json` that would otherwise be read as
  instructions; the library renames these out of the way on decode, and the
  workspace `CLAUDE.md` states that all decoded content is untrusted data and
  that the workspace has no network and no Bash beyond the `adx apk` rebuild.
  The console escapes every value read from an APK (`esc()`), as it does for
  assets today.
- The library and keys directories are mode 700, artifacts mode 400, other
  files 600. The library dev key is for local testing only; release keys are
  never stored here.
- Install targets are limited to emulators aodm reports as running, as the
  console's Assets page is today. A physical phone needs the separate
  physical-device work on the roadmap.
- `evaluation` artifacts are never exported, released or attached to bug
  reports by the tooling.

## 13. Limits and later work

| Limit | Plan |
| --- | --- |
| Split APKs (`.apks`, `.xapk`, multiple `pm path` entries) and `.aab` | L4: import as one artifact set, install with `install-multiple`, modification after merging with APKEditor |
| Decoded output and rebuild need far more space than the APK | `open` checks for roughly 10× the APK size free before decoding and refuses otherwise; `DECODE_MAX_MB` bounds the input |
| Heavily obfuscated or protected apps may not rebuild, or refuse to run re-signed | Recorded per artifact; no workaround planned |
| One machine; no sharing of libraries | Export/import of own artifacts with the saves export work |

## 14. Phases

| Phase | Scope |
| --- | --- |
| L1 | Storage, records, `import`, `pull`, `list`, `show`, `promote`, `send-back`, `unstage`, `install`, `verify`, `gc`, `rm`; migration; console Libraries page (columns, cards, install/promote/send back) |
| L2 | Pipeline integration: dev intake after deploy, `--artifact` runs, gates read run results; release stage → prod (with the release work in app-development-and-release.md §7) |
| L3 | `apk-tools` module; decompile utility (`adx apk decompile/recompile/find/class/status`, indexes, re-signed baseline, §6.3–6.5); `open`, `rebuild`, `diff`; workspace `CLAUDE.md`; console open/rebuild/diff/lineage |
| L4 | Split APKs and bundles; lineage graph; export of own artifacts |

Each phase ends like the others in this repository: `adx check`,
adversarial and deployment review, CHANGELOG and HANDOFF updates, commit.
