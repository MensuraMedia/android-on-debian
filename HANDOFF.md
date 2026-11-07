# Handoff — updated 2026-11-07 (v0.2.0 released; branch `feat/aodm` at P5a)

State of the framework for whoever (human or agent) picks it up next.
History of every installation problem: [docs/install-issues.md](docs/install-issues.md).
Full environment reference: [docs/environment.md](docs/environment.md).
Feature status (shipped / pending / future): [ROADMAP.md](ROADMAP.md).

## State

| Area | Status |
| --- | --- |
| Framework | 15 modules (`./adx list`); `./adx up` idempotent and `./adx doctor` exit 0 on the hybrid-GPU laptop (Mint 22.2). `studio` is optional and not in any profile |
| Render loop (Roborazzi) | Verified; app baselines now committed in `app/src/test/screenshots/` |
| Live loop (emulator + adb) | Verified: `adx emulator start --window`, install, launch, location via `adb emu geo fix` |
| mobile-mcp | Verified from a live Claude Code session (list devices/elements, tap, screenshot) |
| Logging | `~/.local/state/adx/adx.log` (+ emulator logs, previous runs kept); `adx logs`, `adx env` |
| Claude integration | Global rules + `adversarial-reviewer` agent via `claude-rules`; mobile-mcp pinned 1.0.16 |
| Example app | `~/projects/android-apps/HelloAdx`: weather app with live Open-Meteo data (own HANDOFF) |
| Review | Three adversarial reviews (installer, app/template, docs/Claude) on 2026-11-06; fixes are in 0.2.0, open items below |
| Phone image manager | Branch `feat/aodm` (P5a): `aodm list`, `show`, `card`, `env`, `start/stop/restart/health` (names, `--all`, `--group`, `--tag`, `--running`), `save create/list/show/verify/restore/open/pin/delete/prune`, `clone`, `rename`, `delete`, `wipe`, `group`, `tag`, and manager commands `doctor`, `config`, `logs`, `audit`, `disk`, `gc`, `completion`; global `--json`, `--dry-run`, `-q/-v`, `--sandbox`, `--yes`; exit codes 0–5 (docs/aodm-cli.md §3); installed by `adx devices install`; 77 tests incl. fake-SDK and sandboxed-CLI integration tests and the docs/CLI gate; `wipe` verified on a throwaway image (marker gone, first boot 26 s); lifecycle and saves verified on the real emulator (three images on 5554/5556/5558). Saves in `~/.local/share/aod/saves`. Logs: `grep 'aodm #' ~/.local/state/adx/adx.log`, emulator output in `emulator-<image>.log` |
| Desktop window | `aod-gui` (menu: Phone Images; or run `aod-gui`): live thumbnail per phone, Start / Stop… / Save / Clone… through `aodm` (no `--yes`: anything that would ask is refused). Seen working against three running phones; reviewed (code: 15, deployment: 10 findings). Logs: `grep 'gui:' ~/.local/state/adx/adx.log` |
| Push | `feat/aodm` is local only: the GitHub token lacks the `workflow` scope (the branch changes `.github/workflows/`). Fix: `gh auth refresh -h github.com -s workflow`, then `git push -u origin feat/aodm` |
| Repos | `MensuraMedia/android-on-debian` (public, pushed). `LinearGenetics/android-on-debian` (public, empty): the `MensuraMedia` account has read-only access, so it is not pushed yet |

## This machine (hybrid-GPU laptop)

- `config/local.env`: `ADX_EMU_ENV` (NVIDIA offload) and `ADX_EMU_GPU="software"`;
  `-gpu host` crashed twice within minutes (GL `null ctx`).
- User is in the `kvm` group but this login predates it: log out and back in once.
- Menu entry installed (`adx emulator desktop`).

## Known issues

- Window `aod-gui` (P10a), recorded rather than fixed:
  - Screenshots are full raw frames (~10 MB each through adb, ~1.5 s) taken one
    phone at a time every 5 s; with many awake phones each card updates more slowly.
  - Headless machines (CI, servers) build the window too; no `ADX_DEVICES_GUI` switch yet.
  - Only RGBA/RGBX screencap formats are read; other formats show "waiting for screen".
  - Thumbnails are not counted by `aodm disk`; `gc` removes orphans only while at least one image exists.
  - Busy state clears when `aodm` returns; the card's new state shows on the next list (≤3 s).

- `feat/aodm` isn't pushed: the token lacks the `workflow` scope (see State).
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
