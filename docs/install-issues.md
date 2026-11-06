# Installation and configuration issue log

Every problem hit while installing, configuring and first using this
framework, with cause, resolution and status. Add new issues here as they
happen (same columns), and keep the status current. Quick symptom → fix
lookups are in [troubleshooting.md](troubleshooting.md).

Status values:
- **Fixed**: the framework now prevents it.
- **Auto**: detected and configured automatically, no user action.
- **Guided**: detected; adx prints the exact command (needs sudo or a re-login).
- **Documented**: known workaround, nothing to automate.
- **Open**: not resolved yet.

Machines: **A** = desktop, Mint 22.3, Intel UHD only.
**B** = ASUS laptop, Mint 22.2, Intel Iris Xe + NVIDIA RTX 3050 Ti (PRIME on-demand).

| # | When / machine | Phase | Symptom | Cause | Resolution | Status |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 2026-11-05 A | Build | `Toolchain … does not provide the required capabilities: [JAVA_COMPILER]` | `JAVA_HOME` pointed at apt's JRE (`openjdk-21-jre`), no `javac` | `jdk` module installs a full Temurin JDK at user level; `shell-env` exports `JAVA_HOME` | Fixed |
| 2 | 2026-11-05 A | Tests | Robolectric: `Failed to interact with raw FileDescriptor internals` | JDK 17+ module encapsulation with Robolectric on SDK 36 | Template passes `--add-opens=java.base/jdk.internal.access=ALL-UNNAMED` | Fixed |
| 3 | 2026-11-05 A, B | Emulator | `/dev/kvm` works in the desktop but not over SSH/cron; user not in `kvm` group | Access came only from the logind seat ACL | `system` preflight explains which (ACL vs group, and "in group but this login predates it") and prints `sudo usermod -aG kvm $USER` | Guided |
| 4 | 2026-11-05 A | Setup | Project folder renamed `android-studio` → `android-on-debian` broke the running session | Paths baked into the session | Compatibility symlink, removed after restart; `ADX_ROOT` is always derived from the script location | Fixed |
| 5 | 2026-11-05 B | Install | `adx up` stopped at `mobile-mcp: Node.js/npx required` | Node.js not installed; apt Node too old on Mint | New `node` module installs Node.js LTS at user level (SHA-256 from SHASUMS256.txt), before `mobile-mcp` in the profile | Fixed |
| 6 | 2026-11-05 B | Emulator | Segfault ~40 s into every **first** cold boot (exit 139, `libtcmalloc_minimal.so.4` → `_dl_update_slotinfo`) | Hybrid GPU: GL via Mesa while the emulator picked NVIDIA's Vulkan device | `ADX_EMU_ENV` NVIDIA offload; `adx emulator autoconfig` (run by `emulator install`) writes it to `local.env` on hybrid machines | Auto |
| 7 | 2026-11-05 B | Emulator | `adb devices` stuck at `unauthorized` | Crashed first boot left user data half-initialised | `adx emulator start` detects it after 60 s and prints the `--wipe` command; `--wipe` added | Guided |
| 8 | 2026-11-05 B | Emulator | `adx emulator start` waited the full timeout after the emulator had died | Used `adb wait-for-device` | Polls the process and fails within seconds | Fixed |
| 9 | 2026-11-05 A, B | Shell | `adx`, `adb`, `ANDROID_HOME` missing in scripts and `bash -c` | Mint/Ubuntu `~/.bashrc` returns early for non-interactive shells, before the adx block | Use absolute paths (the menu entry does); in scripts `. <repo>/lib/shell-env.sh` | Documented |
| 10 | 2026-11-05 B | Testing | `adb shell input tap X Y` missed buttons that worked before | System-bar insets differ between boots | Read coordinates from the element list each time (mobile-mcp / `uiautomator dump`) | Documented |
| 11 | 2026-11-05 A | Claude | mobile-mcp tools absent after registering | Claude Code loads MCP servers at startup | Restart Claude Code or `/mcp` → reconnect; README says so | Documented |
| 12 | 2026-11-06 B | Emulator | Emulator window/process vanished a few minutes after boot (twice, headless and windowed); `installDebug` failed with `device offline` | `-gpu host` on the hybrid laptop: gfxstream `context was preempted` / `null ctx`, then exit | `ADX_EMU_GPU=software` (Lavapipe); `autoconfig` sets it on hybrid GPUs; `status` hints when hybrid + `host` | Auto |
| 13 | 2026-11-06 B | Logs | The crash log was overwritten by the next start | Emulator log truncated on every start | Previous runs kept as `emulator-<avd>.log.1…5` (`adx_log_shift`) | Fixed |
| 14 | 2026-11-06 B | Logs | No record of what adx had run or printed | adx logged only to the terminal | Framework log `~/.local/state/adx/adx.log` (START/END, exit codes, every message; rotated); `adx logs` | Fixed |
| 15 | 2026-11-06 B | Emulator | Emulator alive but adb listed no device | adb server lost the emulator | `status` reports it; `start` restarts the adb server and reattaches instead of launching a second emulator | Fixed |
| 16 | 2026-11-06 B | Emulator | `swiftshader_indirect` used as the software fallback | Deprecated in emulator 36.4.9; `software` (Lavapipe) replaced it | Config and docs use `software` | Fixed |
| 17 | 2026-11-06 B | Claude | `mobile_set_location` did not move the emulator GPS; the app's location request then hung | It injects a `mobilecli` agent into the app process instead of setting the GPS | Use `adb emu geo fix <lon> <lat>`; in global rules and environment docs | Documented |
| 18 | 2026-11-06 B | App | New location not picked up after `geo fix` | Coarse (approximate) location is cached and blurred by Android for minutes | Expected; documented in the app HANDOFF | Documented |
| 19 | 2026-11-06 B | App | Location fix took ~12 s | Fused provider silent for its whole timeout before GPS was asked | Providers raced in parallel (44 ms) | Fixed |
| 20 | 2026-11-06 B | Template | Kotlin "Unclosed comment" | `HelloAdx/*:V` in a KDoc opens a nested comment | Template `AppLog` doc uses `grep '<App>/'` instead | Fixed |
| 21 | 2026-11-06 B | Desktop | Menu entry `Exec` with `\|\|` and quotes rejected by `desktop-file-validate` | Desktop Entry spec forbids shell syntax | `adx emulator desktop` writes plain `Exec=…/adx emulator start --window --notify`; failures notify from adx | Fixed |
| 22 | 2026-11-06 B | Security | cmdline-tools zip was not checksum-verified (rule says every download is) | No pinned hash | `ADX_CMDLINE_TOOLS_SHA256` pinned (derived from a download matching Google's published SHA-1) | Fixed |
| 23 | 2026-11-06 B | Claude | Global rules and agents existed only on one machine | Hand-written `~/.claude` files | `claude-rules` module installs the rules block and agents from the repo | Fixed |
| 24 | 2026-11-06 B | Install | Missing system packages surfaced late, one at a time | No preflight | `system` module (first in every profile) checks commands, CPU virtualisation, KVM, RAM, disk, and prints one `apt-get install` line | Guided |
| 25 | 2026-11-05 A | Tooling | `sdkmanager is deprecated` warning | Google moving to the `android` CLI | Harmless; migration is a HANDOFF next step | Open |
| 26 | 2026-11-05 A | Build | AGP installs build-tools 36.0.0 on first build | AGP 9.5 default | Expected; pre-install via `ADX_SDK_EXTRA_PACKAGES` if wanted | Documented |
| 27 | 2026-11-06 B | Repo | Creating the HelloAdx GitHub repo was blocked by the agent's auto-mode safety check | Creating a new remote is an outward-facing action | User approves `gh repo create` once (or adds a permission rule) | Open |
| 28 | 2026-11-06 review | Emulator | Hybrid-GPU detection true on every NVIDIA machine | Name match `ati` hit "compatible"/"Corporation" | Detect by PCI vendor IDs from sysfs (no `lspci` needed) | Fixed |
| 29 | 2026-11-06 review | Install | Corrupt cached download failed on every rerun | `wget -c` keeps a full-size bad file | Checksum failure deletes the file and says "rerun" | Fixed |
| 30 | 2026-11-06 review | Install | Half-fetched skills cache / interrupted Node extract never repaired | Guards checked only that a directory existed | Skills check the checked-out SHA; Node extracts to a temp dir and checks `npx` | Fixed |
| 31 | 2026-11-06 review | Install | `adx up` died at mobile-mcp right after installing Node | New `~/.local/bin` not on PATH in the same login | `adx` adds `~/.local/bin` to PATH for its modules | Fixed |
| 32 | 2026-11-06 review | Install | `adx up` said "done" although a module was still broken | Status not re-checked after install | Re-check; `adx up` fails listing incomplete modules | Fixed |
| 33 | 2026-11-06 review | Install | Raw `claude: command not found` (127) | Claude CLI not checked | `mobile-mcp` dies with an install hint; `system` warns | Guided |
| 34 | 2026-11-06 review | Emulator | Boot timed out with a phone attached | `adb shell` without `-s` ("more than one device") | Console port pinned (`ADX_EMU_PORT`), all adb calls use `-s emulator-<port>` | Fixed |
| 35 | 2026-11-06 review | Emulator | A failed start left a half-booted emulator that the next start reported as running | `adx_die` didn't kill the child | EXIT trap kills it on failure | Fixed |
| 36 | 2026-11-06 review | Health | `adx doctor` always exit 1 | Optional `studio` counted | Only profile modules count | Fixed |
| 37 | 2026-11-06 review | Claude | Permission allowlist lets an agent run any adx action (incl. rewriting global rules/permissions) | `Bash(adx:*)` wildcard | Owner decision pending (HANDOFF open questions) | Open |
| 38 | 2026-11-06 review | Repo | `LinearGenetics/android-on-debian` mirror not updated | `MensuraMedia` account has read-only access there | Grant write access, then `git push origin main --tags` (pushes both) | Open |
