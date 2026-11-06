# Environment reference

Everything the framework installs, where it lives, how the pieces connect, where
the logs are, and how to keep it working. For *why* it is built this way see
[architecture.md](architecture.md); for symptoms and fixes see
[troubleshooting.md](troubleshooting.md).

Related: [claude-integration.md](claude-integration.md) (skills, MCP servers, agents, rules),
[dependencies.md](dependencies.md) (everything required, with versions) and
[install-issues.md](install-issues.md) (every installation problem met so far).

Generate a current snapshot of any machine with:

```bash
adx env          # OS, GPUs, KVM, toolchain versions, paths, local overrides
adx doctor       # status of every module (non-zero exit if any fail)
```

Paste both into bug reports and handoffs.

## 1. Layout on disk

| Path | What | Created by |
| --- | --- | --- |
| `~/projects/linear-genetics/android-on-debian/` | This framework (`adx`, modules, templates, docs) | git clone |
| `…/config/defaults.env` | Pinned versions and defaults (committed) | repo |
| `…/config/local.env` | This machine's overrides (git-ignored) | you |
| `…/config/claude-global-rules.md` | Global Claude Code rules (source) | repo |
| `~/projects/android-apps/<App>/` | Apps, one git repo each (`ADX_APPS_DIR`) | `adx app-template new` |
| `~/.local/opt/jdk-21/` | Temurin JDK 21 for Gradle/AGP | `jdk` module |
| `~/Android/Sdk/` | Android SDK: cmdline-tools, platform-tools, platforms, build-tools, emulator, system images (`ANDROID_HOME`) | `sdk`, `emulator` modules |
| `~/.android/avd/adx_api37.avd/` | The emulator's virtual device (user data, snapshots) | `emulator` module |
| `~/.local/opt/node/`, `~/.local/bin/{node,npm,npx}` | Node.js (for mobile-mcp) | manual, see troubleshooting |
| `~/.local/opt/android-studio/` | Android Studio, dormant | `studio` module (manual) |
| `~/.local/state/adx/` | **All framework logs** (`ADX_LOG_DIR`) | `adx` |
| `~/Downloads/adx/` | Downloads cache and screenshots (`ADX_DOWNLOADS`) | `adx` |
| `~/.bashrc` block `adx:shell-env` | Sources `lib/shell-env.sh`: `ANDROID_HOME`, `JAVA_HOME`, `PATH` | `shell-env` module |
| `~/.claude.json` (user scope) | mobile-mcp MCP server registration | `mobile-mcp` module |
| `~/.claude/CLAUDE.md` block `adx:claude-rules` | Global rules: changelog, handoff, logging, Android notes | `claude-rules` module |
| `~/.local/share/applications/adx-emulator.desktop` | Menu entry: Programming → Android Emulator | `adx emulator desktop` |
| `~/projects-backups/` | Local backups: mirror folders + dated `.tar.gz` | manual (see §8) |
| `~/.claude/agents/adversarial-reviewer.md` | Read-only adversarial review agent | `claude-rules` module |

## 2. Configuration

Precedence: `config/defaults.env` → `config/local.env` (both sourced by `adx`
and by `lib/shell-env.sh`). Environment variables set in your shell do **not**
override them; put overrides in `local.env`.

Key settings:

| Variable | Default | Notes |
| --- | --- | --- |
| `ADX_AVD_NAME` | `adx_api37` | AVD used by every `adx emulator` action |
| `ADX_AVD_IMAGE` | `system-images;android-37.0;google_apis_playstore;x86_64` | Play Store image (has Geocoder, fused location) |
| `ADX_EMU_GPU` | `host` | `host`, `auto`, or `software` (Lavapipe; `swiftshader_indirect` is deprecated) |
| `ADX_EMU_HEADLESS` | `true` | `adx emulator start --window` overrides per start |
| `ADX_EMU_ENV` | empty | Extra env for the emulator process only (PRIME offload) |
| `ADX_EMU_BOOT_TIMEOUT` | `240` | Seconds |
| `ADX_LOG_DIR` | `~/.local/state/adx` | Framework and emulator logs |
| `ADX_LOG_MAX_KB` / `ADX_LOG_KEEP` | `5120` / `5` | `adx.log` rotation size and number of old logs kept |
| `ADX_MCP_SCOPE` | `user` | mobile-mcp available in every project |

### This machine (hybrid-GPU laptop, `config/local.env`)

```bash
# Without NVIDIA offload the emulator segfaults in libtcmalloc on first cold boot.
ADX_EMU_ENV="__NV_PRIME_RENDER_OFFLOAD=1 __GLX_VENDOR_LIBRARY_NAME=nvidia __VK_LAYER_NV_optimus=NVIDIA_only"
# With -gpu host the emulator died within minutes (GL "null ctx"). software = Lavapipe, stable.
ADX_EMU_GPU="software"
```

`adx emulator status` prints a hint when it detects hybrid GPUs with
`ADX_EMU_GPU=host`.

## 3. Logs

Every log the toolchain writes, and how to read it:

| Log | Path | Read with |
| --- | --- | --- |
| Framework log: every `adx` run (START/END with exit code, cwd) and every message it printed, timestamped, tagged by module and run id | `~/.local/state/adx/adx.log` (rotated to `adx.log.1…5` above 5 MB) | `adx logs [-n N]` |
| Emulator output, current run | `~/.local/state/adx/emulator-<avd>.log` | `adx logs emulator [-n N]` |
| Emulator output, previous runs (kept so a crash log survives the restart) | `emulator-<avd>.log.1` … `.5` | `less` |
| App logs (apps from the template) | device logcat, tag `<AppName>/<area>` | `adb logcat -d \| grep '<AppName>/'` |
| Crashes on the device | logcat | `adb logcat -d '*:E'`, `adb logcat -b crash -d` |
| Emulator process crashes on the host | systemd-coredump | `coredumpctl list qemu-system-x86_64`, `coredumpctl info <pid>` |
| Gradle | build output | `./gradlew <task> --info`; test reports in `app/build/reports/tests/` |
| mobile-mcp / Claude Code | Claude Code's MCP log | `claude mcp get mobile-mcp`; `/mcp` inside Claude Code |

Example `adx.log`:

```
2026-11-06 00:17:06 START [adx #84237] adx emulator start --window (cwd: …)
2026-11-06 00:17:06 INFO  [emulator #84237] starting adx_api37 (gpu: software, window: true, log: …)
2026-11-06 00:17:10 INFO  [emulator #84237] booted in ~4s: emulator-5554 (pid 84259)
2026-11-06 00:17:10 END   [adx #84237] exit 0
```

Emulator logs written before 2026-11-06 are in `~/Downloads/adx/`.

## 4. Components and how they connect

```
Claude Code ──(stdio MCP)──> mobile-mcp (npx, pinned) ──> adb ──> emulator (qemu + KVM)
     │                                                     ▲
     ├─ Bash: adx, ./gradlew, adb ─────────────────────────┘
     └─ reads ~/.claude/CLAUDE.md (global rules) + <app>/CLAUDE.md + <app>/.claude/settings.json
```

- **adb** server listens on `127.0.0.1:5037`; the emulator registers as
  `emulator-5554` (console port 5554, adb port 5555).
- **mobile-mcp** talks to devices through adb, and for some actions injects a
  helper (`mobilecli`) into the app process. It does not start or stop the
  emulator; use `adx emulator start` first.
- **Gradle** runs with the wrapper in each app; JDK from `JAVA_HOME`
  (`~/.local/opt/jdk-21`), SDK from `local.properties` (`sdk.dir`, git-ignored).
- **Roborazzi** renders on the JVM through Robolectric (SDK 36); no emulator needed.

## 5. Claude Code integration

| Piece | Where | Check |
| --- | --- | --- |
| mobile-mcp server | user scope in `~/.claude.json` | `adx mobile-mcp status`, `/mcp` in Claude Code |
| Global rules (changelog, handoff, logging) | `~/.claude/CLAUDE.md`, block `adx:claude-rules`, source `config/claude-global-rules.md` | `adx claude-rules status`; update with `adx claude-rules install` |
| Per-app instructions | `<app>/CLAUDE.md` (from the template) | — |
| Permission allowlist | `<app>/.claude/settings.json` | `adx claude-permissions apply <dir>` merges the template |
| Skills | `<app>/.claude/skills/` (pinned in `SKILLS.lock`) | `adx skills status` |

Rules for using mobile-mcp reliably:

1. Start the emulator with `adx`, then call `mobile_list_available_devices`.
2. Read screens with `mobile_list_elements_on_screen`; tap by element ref.
3. After launching an app, the first screenshot may still show the splash
   screen; take another or list elements.
4. Do **not** use `mobile_set_location`: it does not move the emulator's GPS,
   and in testing it injected an agent into the app and its location requests
   hung. Use `adb emu geo fix <lon> <lat>` (longitude first).
5. Restart Claude Code (or `/mcp` → reconnect) after changing the MCP registration.

## 6. Daily workflow

```bash
adx emulator start --window       # or menu: Programming → Android Emulator
cd ~/projects/android-apps/<App>
./gradlew recordRoborazziDebug     # render screens, read the PNGs
./gradlew testDebugUnitTest        # unit + screenshot tests
./gradlew installDebug && adb shell am start -n <package>/.MainActivity
adb logcat -d | grep '<App>/'      # app logs
adx emulator stop                  # or the menu entry's "Stop Emulator" action
```

Before ending a session: update the app's `CHANGELOG.md` and `HANDOFF.md`
(global rule), commit, push, and back up (§8).

## 7. Keeping it consistent

| Check | When | Command |
| --- | --- | --- |
| Toolchain health | Start of a session, after updates | `adx doctor` |
| Emulator state, crash hints, menu entry | When anything emulator-related misbehaves | `adx emulator status`, then `adx logs emulator` |
| What happened earlier | After a failure | `adx logs -n 100` |
| Machine snapshot | New machine, bug report, handoff | `adx env` |
| Global rules in sync with the repo | After pulling framework changes | `adx claude-rules status` |
| Reinstall anything missing | New machine or after cleanup | `./adx up` (idempotent) |

Recovery playbook:

| Situation | Do |
| --- | --- |
| Emulator window gone / `adb devices` empty | `adx emulator status`. If the process is alive, `adx emulator start` reattaches adb; if not, read `adx logs emulator` (or `.1` for the previous run) and start again |
| Emulator dies repeatedly with GL errors | `adx emulator autoconfig` (sets `ADX_EMU_GPU="software"`), then restart |
| `unauthorized` device | `adx emulator stop && adx emulator start --wipe` |
| mobile-mcp tools missing | `/mcp` → reconnect, or restart Claude Code |
| App shows a stale location | Coarse location is cached by Android for minutes; `adb emu geo fix …`, wait, or grant precise location |

## 8. Git and backups

- Framework: `github.com/MensuraMedia/android-on-debian` and `github.com/LinearGenetics/android-on-debian` (both public, kept identical), branch `main`.
- Apps: one repo each, e.g. `~/projects/android-apps/HelloAdx`.
- Backups go to `~/projects-backups/`: a mirror folder per project (git
  history included) plus a dated archive, excluding build output:

```bash
B=~/projects-backups
for src in ~/projects/android-apps/HelloAdx ~/projects/linear-genetics/android-on-debian; do
  n=$(basename "$src")
  rsync -a --delete --exclude build/ --exclude .gradle/ --exclude .kotlin/ "$src/" "$B/$n/"
  tar -C "$(dirname "$src")" --exclude=build --exclude=.gradle --exclude=.kotlin \
      -czf "$B/$n-$(date +%Y%m%d-%H%M).tar.gz" "$n"
done
```

## 9. Machines this has been verified on

| Machine | OS | GPU | Notes |
| --- | --- | --- | --- |
| Desktop (first install) | Linux Mint 22.3, kernel 7.0.0-38 | Intel UHD only | `-gpu host` fine |
| ASUS laptop | Linux Mint 22.2, kernel 7.0.0-38, i7-11370H, 23 GiB | Intel Iris Xe + NVIDIA RTX 3050 Ti (PRIME on-demand, driver 580) | Needs `ADX_EMU_ENV` (first-boot segfault) and `ADX_EMU_GPU=software` (GL crashes), both set by `adx emulator autoconfig`; Node 26.13.0 at user level; KVM via logind ACL until re-login after `usermod -aG kvm` |
