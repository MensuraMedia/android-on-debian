# Troubleshooting

| Symptom | Cause | Fix |
| --- | --- | --- |
| `Toolchain installation … does not provide the required capabilities: [JAVA_COMPILER]` | `JAVA_HOME` points at a JRE (for example apt's `openjdk-21-jre`) | `adx jdk install`, then open a new shell so `JAVA_HOME=~/.local/opt/jdk-21` |
| Robolectric: `Failed to interact with raw FileDescriptor internals` | JDK 17+ module encapsulation with Robolectric on SDK 36 | The template already passes `--add-opens=java.base/jdk.internal.access=ALL-UNNAMED`; add it to older apps |
| `adx emulator status` shows `kvm: not accessible` | User isn't in the `kvm` group and the session ACL is missing | `sudo usermod -aG kvm $USER`, then log out of the desktop session and back in (restarting the terminal is not enough) |
| KVM works in the desktop but not over SSH, cron or systemd | `/dev/kvm` access comes only from a logind ACL (`getfacl /dev/kvm` shows `user:<you>:rw-`), which exists for the local graphical seat only | Same fix: join the `kvm` group; check with `id \| grep kvm` after logging back in |
| Emulator segfaults ~40 s into the **first** cold boot of an AVD (exit 139; `coredumpctl info` shows SIGSEGV in `libtcmalloc_minimal.so.4` → `_dl_update_slotinfo`) | Hybrid Intel + NVIDIA laptop (PRIME on-demand): GL loads through Mesa while the emulator picks NVIDIA's Vulkan device; the emulator's bundled tcmalloc crashes in glibc TLS setup. Warm boots are unaffected | Set `ADX_EMU_ENV="__NV_PRIME_RENDER_OFFLOAD=1 __GLX_VENDOR_LIBRARY_NAME=nvidia __VK_LAYER_NV_optimus=NVIDIA_only"` in `config/local.env`, then `adx emulator start --wipe`. It applies to the emulator process only, not to your shells |
| `adb devices` shows `emulator-5554 unauthorized` and never changes | A crashed first boot left the AVD's user data half-initialised, so the guest never trusts `~/.android/adbkey.pub` (the emulator still sends it) | `adx emulator stop && adx emulator start --wipe`. `adx emulator start` now detects this after 60 s and prints the hint |
| `adx up` stops at `mobile-mcp: Node.js/npx required` | Node.js isn't installed; Mint/Ubuntu apt Node is often too old | User-level, no sudo: download `node-<ver>-linux-x64.tar.xz` and `SHASUMS256.txt` from nodejs.org, verify with `sha256sum -c`, unpack to `~/.local/opt/`, symlink `node`, `npm`, `npx` into `~/.local/bin`, then rerun `./adx up` |
| `adx`, `adb` or `ANDROID_HOME` missing in scripts or `bash -c` even after `adx shell-env install` | Mint/Ubuntu `~/.bashrc` returns early for non-interactive shells, before the adx block | In scripts, `. <repo>/lib/shell-env.sh` directly, or set `ANDROID_HOME`/`PATH` explicitly |
| `adb shell input tap X Y` misses a button that worked before | System-bar insets differ between boots, so the same composable moves on screen | Read the coordinates from `adb shell uiautomator dump` (or mobile-mcp's element list) every time; never hard-code them |
| Emulator black screen | GPU driver or mode | `ADX_EMU_GPU=software` in `config/local.env`, or `adx emulator start --window` to inspect |
| `boot timed out` | Cold boot is slow, or there isn't enough RAM | Raise `ADX_EMU_BOOT_TIMEOUT`; check `~/Downloads/adx/emulator-<avd>.log` |
| `emulator exited during boot` | The emulator process crashed (see the segfault row) | Check the log and `coredumpctl list`; retry, or `adx emulator start --wipe` |
| mobile-mcp tools missing in Claude | Claude Code loads MCP servers at startup | Restart Claude Code; check with `claude mcp get mobile-mcp` |
| `sdkmanager is deprecated` warning | Google is moving to the new `android` CLI | Harmless for now; the `sdk` module will migrate to `android sdk` (see HANDOFF.md) |
| AGP installs build-tools 36.0.0 by itself | AGP 9.5 defaults to its own build-tools | Expected; add `build-tools;36.0.0` to `ADX_SDK_EXTRA_PACKAGES` to pre-install it |
| Roborazzi PNG unchanged after an edit | The Gradle build cache hit | `./gradlew recordRoborazziDebug --rerun-tasks` |
