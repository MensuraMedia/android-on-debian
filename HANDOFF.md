# Handoff — updated 2026-11-10 (v0.2.0 released; adx console; offline phones, per-phone logcat; app pipeline; app dev & release requirements; app library design; framework change process; emulator multi-display requested)

State of the framework for whoever (human or agent) picks it up next.
History of every installation problem: [docs/install-issues.md](docs/install-issues.md).
Full environment reference: [docs/environment.md](docs/environment.md).
Feature status (shipped / pending / future): [ROADMAP.md](ROADMAP.md).

## State

| Area | Status |
| --- | --- |
| Framework | 17 modules (`./adx list`, all ok on 2026-11-09); `./adx up` idempotent and `./adx doctor` exit 0 on the hybrid-GPU laptop (Mint 22.3). `studio` is optional and not in any profile. New AVDs get `hw.keyboard=yes` (`ADX_AVD_HW_KEYBOARD`) so the host keyboard types into the emulator |
| Render loop (Roborazzi) | Verified; app baselines now committed in `app/src/test/screenshots/` |
| Live loop (emulator + adb) | Verified: `adx emulator start --window`, install, launch, location via `adb emu geo fix` |
| mobile-mcp | Verified from a live Claude Code session (list devices/elements, tap, screenshot) |
| Logging | `~/.local/state/adx/adx.log` (+ emulator logs, previous runs kept); `adx logs`, `adx env` |
| Claude integration | Global rules + `adversarial-reviewer` agent via `claude-rules`; mobile-mcp pinned 1.0.16 |
| Example app | `~/projects/android-apps/HelloAdx`: weather app with live Open-Meteo data (own HANDOFF) |
| Review | Three adversarial reviews (installer, app/template, docs/Claude) on 2026-11-06; fixes are in 0.2.0, open items below |
| Phone image manager | On `main` (merged from `feat/aodm`, P5a): `aodm list`, `show`, `card`, `env`, `start/stop/restart/health` (names, `--all`, `--group`, `--tag`, `--running`), `save create/list/show/verify/restore/open/pin/delete/prune`, `clone`, `rename`, `delete`, `wipe`, `group`, `tag`, and manager commands `doctor`, `config`, `logs`, `audit`, `disk`, `gc`, `completion`; global `--json`, `--dry-run`, `-q/-v`, `--sandbox`, `--yes`; exit codes 0–5 (docs/aodm-cli.md §3); installed by `adx devices install`; 77 tests incl. fake-SDK and sandboxed-CLI integration tests and the docs/CLI gate; `wipe` verified on a throwaway image (marker gone, first boot 26 s); lifecycle and saves verified on the real emulator (three images on 5554/5556/5558). Saves in `~/.local/share/aod/saves`. Logs: `grep 'aodm #' ~/.local/state/adx/adx.log`, emulator output in `emulator-<image>.log` |
| Web console | `adx console start` (menu: adx console, own icon in menu/panel/Alt+Tab) → http://127.0.0.2:8766 (out of the phones' reach), Python stdlib only. Images: aodm Launch/Stop, Save, Duplicate (clone), Rename, plus per-phone Screenshot, Record (≤180 s) and Captures gallery (`~/.local/share/adx/assets/captures/<image>/`); system images. Assets: upload folders (wallpapers, photos & videos, sounds, APKs) + top 10 phone features. Sidebar: Internet switch (airplane mode, ping-checked). Per phone: Go offline / Go online and a Logcat viewer (below). Apps (pipeline: New app, Run pipeline, Runs, Record screenshots, Open folder; pipeline skills), Modules (Install, doctor), Activity, Settings. Every page and action verified in headless Chromium and against the API 37 emulator (phone state reset afterwards); Whole console **not yet adversarially reviewed** (only the offline/logcat change was, 2026-11-10). Server log `~/.local/state/adx/console.log`, job logs `~/.local/state/adx/console/` |
| App dev & release docs | [docs/app-development-and-release.md](docs/app-development-and-release.md) (2026-11-10): machine/toolchain/project requirements, pipeline, definition of done, release and store requirements, manual release checklist |
| App library (design) | [docs/app-library.md](docs/app-library.md) (2026-11-10, **design only, nothing built**): per-app APK libraries in dev/test/prod stages, provenance/lineage/ownership, version-last names (`<label>-<id>-v<ver>[+mN]`), `adx library`/`adx apk` CLIs with a token-saving decompile/recompile utility and workspace indexes, a **re-signed like-for-like baseline** and feed-inspection recipe for 1-to-1 comparison, stage gates, pipeline integration, console **Libraries** page replacing the Assets "Apps" list. One adversarial review (9 findings) applied. Build governance in §1.1. Phases L1–L4 in ROADMAP.md |
| Framework change process | [docs/architecture.md](docs/architecture.md) "How changes are made": **two tracks** — apps follow app best practices; any enhancement to the adx console, the emulator or their properties follows plan → design (a technical doc) → strict modular development (modules, contract-only, config in `defaults.env`, thin front ends over `adx` CLIs) → `adx check` + adversarial/deployment review per phase |
| App pipeline | `adx pipeline run <App>` (build → lint → test → deploy → smoke) with run records in `~/.local/share/adx/pipeline/<App>/`; console Apps page (Run pipeline, Runs, New app, pipeline skills). Verified 2026-11-10: MeridianWeather and HelloAdx pass all stages; a compile error, a screenshot change and a launch crash each fail the right stage with the evidence; a fresh template app records its references on the first run and verifies on the second; console flow in headless Chromium |
| Console offline | The console needs no internet. Its window (Brave/Chrome `--app`, profile `~/.local/state/adx/console-window`) runs with background networking off and a dead proxy for non-loopback traffic; verified 2026-11-10: no outside connections in 60 s (before: Brave reached a CDN). Skills-page GitHub links don't open there |
| Offline phones | `aodm net NAME offline\|online` (registry `offline`), console **Go offline** on the card. Applies at start, in every start path (aodm, aod-gui, console, `adx emulator start`): restricted user-mode network + netsim Wi-Fi off. Verified 2026-11-10 on API 37 / emulator 37.2.13: ping, TCP out and 10.0.2.2 blocked, adb works, back online the network returns; round trip through the console UI with restart |
| Per-phone logcat | Every start saves `~/.local/state/adx/logcat-NAME.log` (previous starts `.1`…`.5`, `ADX_LOG_KEEP`): `adx logs logcat`, `aodm logs --image NAME --logcat`, console **Logcat** (run picker, filter). Verified on both start paths |
| Security | Posture measured and written up in [docs/security.md](docs/security.md) (2026-11-09): no inbound path from the internet (NAT, loopback-only ports, ufw on); test-device weaknesses accepted by decision; one open finding in LinSmartPhone (separate repo, not fixed here) |
| Desktop window | `aod-gui` (menu: Phone Images; or run `aod-gui`): live thumbnail per phone, Start / Stop… / Save / Clone… through `aodm` (no `--yes`: anything that would ask is refused). Seen working against three running phones; reviewed (code: 15, deployment: 10 findings). Logs: `grep 'gui:' ~/.local/state/adx/adx.log` |
| Push | `main` pushed to `MensuraMedia/android-on-debian` (last: `14cfa9a`, 2026-11-10; local `main` pulled to it). Since 0.2.0: `a46c1be` app pipeline + offline phones + per-phone logcat + app dev/release docs, `360d7cb` console APK identity fix, `14cfa9a` app-library design + framework change process + emulator multi-display plan. Working tree clean. `feat/aodm` merged (PR #1, `4b5f242`) and can be deleted on GitHub |
| Repos | `MensuraMedia/android-on-debian` (public, pushed). `LinearGenetics/android-on-debian` (public, empty): the `MensuraMedia` account has read-only access, so it is not pushed yet |

## This machine (hybrid-GPU laptop)

- `config/local.env` has no overrides now: the emulator runs `-gpu host` and
  was stable all through 2026-11-09. (On 2026-11-06 `-gpu host` crashed twice
  with GL `null ctx`; if that returns, `adx emulator autoconfig` sets
  `ADX_EMU_GPU=software`.)
- User is in the `kvm` group but this login predates it: log out and back in once.
- Menu entries installed: Android Emulator (`adx emulator desktop`), Phone
  Images (`aod-gui`), adx console.
- `./adx up` run on 2026-11-10 at `14cfa9a`: aodm/aod-gui 0.3.0-dev
  (`v0.2.0-28-g14cfa9a`) installed, console module installed, `adx doctor`
  clean. Console started at http://127.0.0.2:8766.
- Images: only `adx_api37` (stopped). The `throwaway` and `throwaway-clone`
  test images and throwaway's save were deleted on 2026-11-10.
- **No saves exist.** An earlier note here listed an `adx_api37` baseline save
  `20261110-012304Z-baseline`; on 2026-11-10 22:20 it was not in
  `~/.local/share/aod/saves` and `adx.log` has no record of it being created or
  deleted. Make a new one with `aodm save create adx_api37` if needed.
- Local backups (tar.gz + `backup-log.md`, build output excluded):
  `~/backups/android-on-debian/` and `~/backups/HelloAdx/` (2026-11-10, `14cfa9a`).
- Emulator GPS location was left at Paris (48.85, 2.35) by a console test.

## Known issues

- Window `aod-gui` (P10a), recorded rather than fixed:
  - Screenshots are full raw frames (~10 MB each through adb, ~1.5 s) taken one
    phone at a time every 5 s; with many awake phones each card updates more slowly.
  - Headless machines (CI, servers) build the window too; no `ADX_DEVICES_GUI` switch yet.
  - Only RGBA/RGBX screencap formats are read; other formats show "waiting for screen".
  - Thumbnails are not counted by `aodm disk`; `gc` removes orphans only while at least one image exists.
  - Busy state clears when `aodm` returns; the card's new state shows on the next list (≤3 s).

- adx console, recorded rather than fixed:
  - Not yet adversarially reviewed (installer, server, page).
  - Chromium hands the window manager only a 16×16 favicon; the panel and
    Alt+Tab use the menu entry's theme icon (matched by `StartupWMClass`), so
    they look right, but a WM that only uses the window's own icon would scale up 16 px.
  - Screen recordings stop at 180 s (Android's `screenrecord` limit); one
    recording per phone at a time; a recording in progress is lost if the
    console server stops.
  - Set wallpaper opens the phone's own preview (Photos' SetWallpaperActivity);
    the user confirms on the phone: Android has no adb command to set one.
  - Recording state and demo-mode state live in the server's memory only.
- Logcat capture, recorded rather than fixed: it starts with the phone, so a
  phone already running when it is started otherwise (e.g. by a bare
  `emulator`) has none; an adb server restart ends it until the next start;
  no size cap per run (a chatty day can reach tens of MB; the console viewer
  reads the whole file); no crash/ANR extracts yet.
- Assets APK installer: plain `.apk` files only (no split APKs, `.apks`,
  `.xapk` or `.aab`; install those with `adb install-multiple` or
  bundletool). APKs from mirror sites can carry another app than their
  file name says: the page now shows the real app name and package.
- App pipeline, recorded rather than fixed:
  - one phone per run, and only emulators (no physical phones yet);
  - smoke checks only that the app launches and stays up: no instrumented
    or UI tests on the phone;
  - debug builds only, no release stage (docs/app-development-and-release.md §7);
  - an interrupted run leaves Gradle's daemon (by design) and may leave a
    `gradlew` child until it ends;
  - HelloAdx and MeridianWeather keep their screenshot references in
    `app/build/outputs/roborazzi` (not committed), so `gradle clean` makes
    the next run re-record them.
- `aodm stop` can report "stopped" while the exiting emulator still holds the
  image lock for a moment (the review's likely cause: /proc cmdline empties
  before the process releases its files). Start waits up to 10 s for it; a
  clone/save/wipe issued right after a stop could still be refused once.
  Proper fix: have stop wait for the lock too.
- Offline mode leaves non-IP channels open by design: Bluetooth/BLE between
  emulators (netsim), SMS and calls between emulators, the shared clipboard.
- Offline mode changes take effect at the next start (the card offers a
  restart). It relies on the emulator's `-feature -WiFiPacketStream` and
  `restrict=on` options: re-check after an emulator update (docs/security.md
  shows how).
- The emulator's side toolbar can stay behind on another workspace when the
  phone window moves; dragging the phone window re-docks it.
- `config/claude-global-rules.md` gained an `ANDROID_SERIAL` note (several
  emulators break plain `adb`); it reaches `~/.claude/CLAUDE.md` only after the
  owner runs `adx claude-rules install`.
- `aodm stop --all` / `--running` include `adx_api37` (the emulator
  mobile-mcp uses): by design, `--all` means all; name images to spare it.
- Lifecycle orchestration (start/stop) has no automated fake-emulator test
  yet (real-emulator checks only); saves/clone/rename/delete have one.
- Clones and opened saves keep the source's Android ID until P5b (identity policies).
- `adx emulator stop && adx emulator start` back to back: stop printed
  "emulator stopped" while the emulator was still exiting, so start said
  "already running" and nothing came up (seen 2026-11-06). Wait for the
  process to go (or use `aodm restart`, which waits) until stop waits itself.
- A headless emulator's screen sleeps, and Doze then cuts app network and
  location (HelloAdx showed "Unable to resolve host" while `adb shell ping`
  worked). Workaround: `adb shell svc power stayon true` (set on adx_api37 on
  2026-11-06). Candidate fix: runtime setting "stay awake" (P5) or `adx
  emulator start`.
- `aodm config unset ADX_EMU_GPU` (or `ADX_EMU_ENV`) on this hybrid-GPU
  laptop is undone by the next `adx up`/`adx emulator install` (adx's
  autoconfig adds the key when it's missing); use `config set` instead. aodm
  says so when you unset them.
- Exit code 3 means "refused on purpose"; P5a moved real failures (tool
  errors, console auth, emulator died) to exit 1, but some refusal messages
  remain broad (e.g. a damaged save refusing a restore is 3).
- aodm work folders count as dead when `/proc/PID` is gone: two hosts sharing
  a saves folder over NFS, or aodm in another PID namespace, could sweep a
  live run's copy (docs/phone-image-manager.md §18).
- The `deploy-reviewer` agent isn't loaded in sessions started before it was
  installed; such sessions run it through a general agent with its file.

## Next steps

0. **Emulator secondary displays** (owner request 2026-11-10, clarified): the
   emulator's own multi-display feature — Extended Controls → Displays, "Add
   secondary display (Max 3)" — NOT Android overlay displays. The owner wants
   the framework to support configuring these. Mechanism and plan:
   - Emulator supports up to 3 secondary displays via the launch flag
     `-multidisplay <id> <w> <h> <dpi> [<id> <w> <h> <dpi> …]` (applied at
     start). The Extended Controls panel also adds/removes them live over
     gRPC, but that is not persisted and not scriptable through aodm today.
   - Fits the framework the same way `offline` does: a registry field per
     image (e.g. `displays = [{w,h,dpi}, …]`), a `Launch` field, the
     `-multidisplay` args built in BOTH `tools/aodm/crates/aod-core/src/emu.rs`
     (`args`) and `modules/emulator/module.sh` (next to the offline flags),
     `aodm` subcommand + registry round-trip, and a console card control.
     Applies at start, like offline → needs a restart.
   - This is an **emulator-property change**, so it follows the framework
     process (architecture.md "How changes are made"): plan → a short design
     (extend docs/phone-image-manager.md or a new doc) → strict modular
     development → `adx check` + reviews. **To be done in the Opus 5.5 session.**
   - NB: on 2026-11-10 an overlay-display experiment (`settings put global
     overlay_display_devices …`) was tried and **reverted**; it is the wrong
     layer (Android OS overlays, not emulator displays). The phone is back to
     a single 1080x2400 display, setting cleared.

0b. **App library** (owner request 2026-11-10): design in
   [docs/app-library.md](docs/app-library.md), phases L1–L4 in ROADMAP.md.
   Start with L1 (storage, import/promote/send-back/install, console
   Libraries page, migration of the Assets `apps/` folder). Test case:
   Pinterest 14.43.0 on `adx_api37` (installed via Uptodown on the phone,
   evaluation only, not for deployment). The design doc has had one adversarial
   review (9 findings) applied: gate evidence copied into artifact records
   before pipeline runs are pruned, per-package locks with atomic writes,
   prod protected from gc/rm, sandboxed untrusted-APK decode, signer-change
   and install-flag handling, and a re-signed baseline for 1-to-1 comparison.
0. App pipeline, next pieces (owner to choose): "Fix with Claude" (console
   starts headless Claude Code on a failed run, deferred by decision);
   instrumented/UI tests on the phone (connectedAndroidTest) and a scripted
   mobile-mcp walkthrough as a stage; release build + signing; a pipeline
   section in HelloAdx's and MeridianWeather's own CLAUDE.md (they predate
   the template change).
1. adx console follow-ups: adversarial + deployment review of the whole
   console; logcat crash/ANR extracts if wanted; remaining optional security
   controls from docs/security.md (filtering Private DNS, a *filtered*
   network mode, `hw.audioInput=no`) only if the owner asks.
1. **Phone image manager (Rust)** on branch `feat/aodm`: plan approved; P0
   (spike) and P1 (gate, devtools, rust, agents, workflow, CI, workspace core)
   done and reviewed (30 findings fixed); P2 (inventory, process detection,
   registry, version card, `devices` module) done and reviewed (23 findings
   fixed); P3 (ports, start/stop/restart/health, budget, adb + console
   clients) done, reviewed (25 findings fixed) and verified end to end on
   the real emulator; P4 (saves, clone, rename, delete) done, reviewed (24
   findings: 2 critical, all fixed or recorded) and verified end to end;
   P5a (CLI conventions and manager administration of
   [docs/aodm-cli.md](docs/aodm-cli.md)) done and reviewed (25 findings,
   fixed or recorded); next P5b: settings profiles (incl. "stay awake" for
   the Doze issue), identity policies and security locks. Status per phase: ROADMAP.md; design: docs/phone-image-manager.md.
2. Decide the permission-allowlist items in Open questions, then apply them in
   `templates/claude/settings.json` and let `claude-permissions` tighten rules.
3. Push to `LinearGenetics/android-on-debian` once the account has write access
   (`git push origin main --tags` pushes to both mirrors).
4. Non-interactive shells: also write the env block to `~/.profile` or before
   the `.bashrc` interactive guard (install-issues #9).
5. `skills apply`: delete skills removed from the manifest (left vendored today).
6. Migrate `sdk`/`emulator` from the deprecated `sdkmanager`/`avdmanager` to
   the `android` CLI once its syntax has been checked.
7. Compose Preview scanner so every `@Preview` renders without a hand-written test.
8. Optional modules: `physical-device` (udev, scrcpy), `ftl` (Firebase Test Lab);
   a `ci` profile + GitHub Actions template (`verifyRoborazziDebug`).
9. Template: R8/minify for release, adaptive app icon, reject Kotlin keywords in
   package names, fail on unrendered `@@TOKENS@@`.

## Approvals recorded

- 2026-11-06: owner requested the `adversarial-reviewer` and `deploy-reviewer`
  agents ("create adversarial agent…", "ensure agents scrutinize change
  deployments"); both are global (`~/.claude/agents/`), read-only by
  instruction (their Bash tool is not technically restricted).

## Decisions

- 2026-11-10: the console is an AI (Claude)-integrated environment, not an
  IDE: Claude Code plans, designs and codes; the environment assembles,
  checks, tests and deploys to the emulator for review and iteration.
  Skills belong to the app pipeline (applied at app creation). First
  version: Claude stays in the owner's Claude Code sessions and reads the
  pipeline's results (no console-launched Claude yet); deploys go to the
  running phone, asking which if several run.

- 2026-11-10: the owner asked for per-phone logcat capture and an offline
  option per phone that needs no network configuration on the phone itself.
  Offline is set outside the phone (emulator flags), stored in aodm's
  registry, so every front end shares it; logcat sits next to the emulator
  log in `ADX_LOG_DIR` and rotates the same way.

- 2026-11-09: the adx console (web GUI) is the OS/phone image manager the
  owner uses day to day; it owns the new app icon. `aod-gui` keeps its
  generic `phone` icon. LineageOS/phone-conversion features stay in
  LinSmartPhone (`~/projects/android-conversion`) and are not part of this repo.
- 2026-11-09: security: document the posture; don't change anything already
  sufficient. The LinSmartPhone token exposure is recorded in
  docs/security.md, not fixed from here.
- 2026-11-07: the planned aodm manager (P8 terminal dashboard, P10 desktop
  window) is the manager to use. The P10a "Phone Images" window is its
  prototype: its thumbnail card grid becomes the manager's main view, built
  on `crates/aod-gui`, not a second app.

## Open questions

- **Permission allowlist (security, needs the owner's decision).** The docs
  review found: `Bash(adx:*)`/`Bash(./adx:*)` auto-allow every adx action,
  including `adx claude-rules install`, `adx claude-permissions apply` and
  `adx gradle run …`, so an agent could rewrite global instructions or
  permissions without a prompt; `mobile_set_location` is auto-allowed although
  it hangs location; `adb emu geo fix` and `adb -s …` are not allowed;
  `claude-permissions apply` can only add rules; `adb shell <allowed> \; rm …`
  can chain past ask rules on the device. Left unchanged on purpose.
- Keep `swiftui-pro` vendored into Android apps, or only in a future iOS template?
- `ADX_APPS_DIR` inside or outside this repo (kept separate: one git repo per app)?
