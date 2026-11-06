# Troubleshooting

| Symptom | Cause | Fix |
| --- | --- | --- |
| `Toolchain installation … does not provide the required capabilities: [JAVA_COMPILER]` | `JAVA_HOME` points at a JRE (for example apt's `openjdk-21-jre`) | `adx jdk install`, then open a new shell so `JAVA_HOME=~/.local/opt/jdk-21` |
| Robolectric: `Failed to interact with raw FileDescriptor internals` | JDK 17+ module encapsulation with Robolectric on SDK 36 | The template already passes `--add-opens=java.base/jdk.internal.access=ALL-UNNAMED`; add it to older apps |
| `adx emulator status` shows `kvm: not accessible` | User isn't in the `kvm` group and the session ACL is missing | `sudo usermod -aG kvm $USER`, then log out and back in |
| Emulator black screen | GPU driver or mode | `ADX_EMU_GPU=software` in `config/local.env`, or `adx emulator start --window` to inspect |
| `boot timed out` | Cold boot is slow, or there isn't enough RAM | Raise `ADX_EMU_BOOT_TIMEOUT`; check `~/Downloads/adx/emulator-<avd>.log` |
| mobile-mcp tools missing in Claude | Claude Code loads MCP servers at startup | Restart Claude Code; check with `claude mcp get mobile-mcp` |
| `sdkmanager is deprecated` warning | Google is moving to the new `android` CLI | Harmless for now; the `sdk` module will migrate to `android sdk` (see HANDOFF.md) |
| AGP installs build-tools 36.0.0 by itself | AGP 9.5 defaults to its own build-tools | Expected; add `build-tools;36.0.0` to `ADX_SDK_EXTRA_PACKAGES` to pre-install it |
| Roborazzi PNG unchanged after an edit | The Gradle build cache hit | `./gradlew recordRoborazziDebug --rerun-tasks` |
