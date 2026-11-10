# Handoff — updated 2026-11-09 (v0.2.0 released; `feat/aodm` merged into `main`; adx console)

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
| Web console | `adx console start` (menu: adx console, own icon in menu/panel/Alt+Tab) → http://127.0.0.1:8766, Python stdlib only. Images: aodm Launch/Stop, Save, Duplicate (clone), Rename, plus per-phone Screenshot, Record (≤180 s) and Captures gallery (`~/.local/share/adx/assets/captures/<image>/`); system images. Assets: upload folders (wallpapers, photos & videos, sounds, APKs) + top 10 phone features. Sidebar: Internet switch (airplane mode, ping-checked). Apps (Render/Run/Open, screenshot gallery), Modules (Install, doctor), Skills, Activity, Settings. Every page and action verified in headless Chromium and against the API 37 emulator (phone state reset afterwards); **not yet adversarially reviewed**. Server log `~/.local/state/adx/console.log`, job logs `~/.local/state/adx/console/` |
| Security | Posture measured and written up in [docs/security.md](docs/security.md) (2026-11-09): no inbound path from the internet (NAT, loopback-only ports, ufw on); test-device weaknesses accepted by decision; one open finding in LinSmartPhone (separate repo, not fixed here) |
| Desktop window | `aod-gui` (menu: Phone Images; or run `aod-gui`): live thumbnail per phone, Start / Stop… / Save / Clone… through `aodm` (no `--yes`: anything that would ask is refused). Seen working against three running phones; reviewed (code: 15, deployment: 10 findings). Logs: `grep 'gui:' ~/.local/state/adx/adx.log` |
| Push | `main` pushed to `MensuraMedia/android-on-debian` (last: `8894b30`); `feat/aodm` merged (`4b5f242`) and can be deleted |
| Repos | `MensuraMedia/android-on-debian` (public, pushed). `LinearGenetics/android-on-debian` (public, empty): the `MensuraMedia` account has read-only access, so it is not pushed yet |

## This machine (hybrid-GPU laptop)

- `config/local.env` has no overrides now: the emulator runs `-gpu host` and
  was stable all through 2026-11-09. (On 2026-11-06 `-gpu host` crashed twice
  with GL `null ctx`; if that returns, `adx emulator autoconfig` sets
  `ADX_EMU_GPU=software`.)
- User is in the `kvm` group but this login predates it: log out and back in once.
- Menu entries installed: Android Emulator (`adx emulator desktop`), Phone
  Images (`aod-gui`), adx console.
- `adx_api37` baseline save: `20261110-012304Z-baseline` (8.7 GB, pinned,
  verified). `aodm save restore adx_api37 20261110-012304Z-baseline` returns
  the phone to it.
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
- Phone logs (logcat: app logs, crashes, ANRs) aren't kept anywhere; only the
  emulator process log per image, `adx.log` and console job logs are.
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

0. adx console follow-ups: adversarial + deployment review; per-phone logcat
   capture (`logs/<image>/`, crash/ANR extracts, viewer on each card) was
   offered on 2026-11-09 and awaits the owner; optional security controls
   from docs/security.md (filtering Private DNS, offline/filtered network
   mode per image, `hw.audioInput=no`) only if the owner asks.
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
