# Dependency catalogue

Every dependency of the framework and of the apps it scaffolds: what it is
for, which version is pinned, who installs it, and whether it is required.
Versions live in `config/defaults.env` (framework) and the app's
`gradle/libs.versions.toml` (apps). Installed versions on a machine:
`adx env` and `adx doctor`. Verified 2026-11-06.

Legend: **R** required · **O** optional · **D** desktop convenience.
"Installer" is the module that installs it; `system` only checks and prints
the `sudo` command, because adx never runs sudo.

## 1. Hardware and host

| Item | Need | Why | Check |
| --- | --- | --- | --- |
| x86_64 CPU with VT-x/AMD-V (+EPT) | R | KVM acceleration for the emulator; no nested VM | `adx system status` |
| `/dev/kvm` read/write | R | Emulator acceleration. Seat ACL works in the desktop session; the `kvm` group is needed for SSH/cron | `adx system status` |
| RAM | R 8 GiB, 16 GiB recommended | API 37 AVD takes ≥ 4 GiB guest RAM, plus Gradle daemon | `adx system status` |
| Disk in `$HOME` | R ~15 GiB | SDK ≈ 4.2 GiB, Gradle cache ≈ 1.6 GiB, JDK 0.35 GiB, AVD data | `adx system status` |
| GPU | O | `-gpu host` uses it; hybrid Intel/AMD + NVIDIA laptops get `software` (Lavapipe) via `adx emulator autoconfig` | `adx env` |
| Network (HTTPS) | R for install | nodejs.org, api.adoptium.net, dl.google.com, services.gradle.org, Maven Central/Google Maven, registry.npmjs.org, github.com (skills) | — |

## 2. System packages (apt, need sudo; checked by `system`)

| Command | Package | Need | Used by |
| --- | --- | --- | --- |
| `bash` ≥ 5 | bash | R | everything |
| `curl`, `wget` | curl, wget | R | API queries (Adoptium), resumable downloads |
| `unzip` | unzip | R | cmdline-tools, Gradle distributions |
| `tar`, `xz` | tar, xz-utils | R | JDK (`.tar.gz`), Node (`.tar.xz`) |
| `sha256sum` | coreutils | R | verified downloads |
| `git` | git | R | apps, skills (pinned commits) |
| `jq` | jq | R | Adoptium API, Claude settings merge |
| `perl` | perl | R | template rendering (`adx_render_tree`) |
| `awk`, `pgrep` | gawk/mawk, procps | R | status parsing, emulator process detection |
| `lspci` | pciutils | O | hybrid-GPU detection (`emulator autoconfig`, `adx env`) |
| `notify-send` | libnotify-bin | D | menu-entry notifications |
| `desktop-file-validate`, `update-desktop-database` | desktop-file-utils | D | `adx emulator desktop` |
| `getfacl` | acl | O | diagnosing KVM ACL access |
| `rsync` | rsync | O | backups |
| emulator runtime libs | libgl1, libnss3, libpulse0, libvulkan1, mesa-vulkan-drivers, libx11-6, libxkbfile1 | R (emulator) | Present on any desktop install; on a minimal/server install add them |
| 32-bit libs | libncurses6:i386, lib32z1 | O | Android Studio only |
| `shellcheck` | shellcheck | O (dev) | linting the framework (CI runs it) |

One-shot install of everything above:

```bash
sudo apt-get install -y bash curl wget unzip tar xz-utils coreutils git jq perl gawk procps \
  pciutils libnotify-bin desktop-file-utils acl rsync shellcheck \
  libgl1 libnss3 libpulse0 libvulkan1 mesa-vulkan-drivers libx11-6 libxkbfile1
sudo usermod -aG kvm "$USER"     # then log out and back in
```

## 3. User-level toolchain (no sudo; installed by `adx up`)

| Component | Pinned | Location | Installer | Need | Source / verification |
| --- | --- | --- | --- | --- | --- |
| Eclipse Temurin JDK | 21 (latest GA, 21.0.13 now) | `~/.local/opt/jdk-21` | `jdk` | R | api.adoptium.net, SHA-256 from API |
| Android cmdline-tools | build 16111833 (23.0.0) | `~/Android/Sdk/cmdline-tools/latest` | `sdk` | R | dl.google.com, SHA-256 pinned |
| platform-tools (adb) | latest (37.0.1) | `~/Android/Sdk/platform-tools` | `sdk` | R | sdkmanager |
| platforms;android-37.0 | 37.0 | `~/Android/Sdk/platforms` | `sdk` | R | sdkmanager |
| build-tools | 37.0.0 (+36.0.0 pulled by AGP) | `~/Android/Sdk/build-tools` | `sdk` / AGP | R | sdkmanager |
| Android Emulator | latest (37.2.13) | `~/Android/Sdk/emulator` | `emulator` | R (live loop) | sdkmanager |
| System image | android-37.0 google_apis_playstore x86_64 (rev 8; ≥ 5 needed for Play sign-in since 2026-06) | `~/Android/Sdk/system-images` | `emulator` | R (live loop) | sdkmanager |
| AVD `adx_api37` | pixel_8 profile | `~/.android/avd` | `emulator` | R (live loop) | avdmanager |
| Gradle (bootstrap only) | 9.8.2 | Gradle cache | `gradle` | R (new apps) | services.gradle.org, SHA-256 |
| Node.js | 26.13.0 LTS (min major 20) | `~/.local/opt/node-v…`, links in `~/.local/bin` | `node` | R (mobile-mcp) | nodejs.org, SHA-256 from SHASUMS256.txt |
| Claude Code CLI | user-installed (2.1.x) | `~/.local/bin/claude` | — (prerequisite) | R (agent workflow) | claude.com |
| Rust toolchain | 1.95.0 via rustup 1.29.3 (rustup-init SHA-256 pinned) | `~/.cargo`, `~/.rustup` (not on PATH; modules use full paths) | `rust` | R (aodm) | static.rust-lang.org |
| ShellCheck | 0.11.0 (static binary) | `~/.local/opt/shellcheck-0.11.0`, link in `~/.local/bin` | `devtools` | R (adx check) | GitHub release, SHA-256 pinned |
| cargo-deny | 0.20.2 (musl binary) | `~/.local/opt/cargo-deny-0.20.2`, link in `~/.local/bin` | `devtools` | R (adx check) | GitHub release, SHA-256 pinned |
| Android Studio | dormant | `~/.local/opt/android-studio` | `studio` (manual) | O | developer.android.com |

## 4. Claude Code integration

| Component | Pinned | Location | Installer |
| --- | --- | --- | --- |
| mobile-mcp MCP server | `@mobilenext/mobile-mcp@1.0.16` via `npx` | user scope in `~/.claude.json` | `mobile-mcp` |
| Global rules | `config/claude-global-rules.md` | `~/.claude/CLAUDE.md` (marked block) | `claude-rules` |
| Agents | `templates/claude/agents/*.md` (adversarial-reviewer) | `~/.claude/agents/` | `claude-rules` |
| Permission allowlist | `templates/claude/settings.json` | `<project>/.claude/settings.json` | `claude-permissions` |
| Skills | commit-pinned in `config/skills.manifest`: hallmark, swiftui-pro, autoresearch, ponytail (+ audit, debt, gain, help, review) | `<project>/.claude/skills/` + `SKILLS.lock` | `skills` |

## 5. App dependencies (template `compose-roborazzi`, per app)

| Library / plugin | Version | Scope | Purpose |
| --- | --- | --- | --- |
| Android Gradle Plugin | 9.5.1 | build | Android build |
| Kotlin + Compose compiler plugin | 2.4.21 | build | Kotlin, Compose |
| Compose BOM | 2026.09.00 | impl | ui, material3, tooling (versions from BOM) |
| androidx.activity:activity-compose | 1.13.0 | impl | `setContent`, permission launcher, edge-to-edge |
| Roborazzi (+ compose, junit-rule) and Gradle plugin | 1.77.0 | test | JVM screenshot rendering |
| Robolectric | 4.17 (renders SDK 36) | test | Android runtime on the JVM |
| JUnit 4, androidx.test.ext:junit | 4.13.2, 1.3.0 | test | test runner |
| SDK levels | compile 37, target 36, min 26 | — | `ADX_COMPILE_SDK`, `ADX_TARGET_SDK`, `ADX_MIN_SDK` |

Platform APIs the example app uses instead of extra libraries:
`HttpURLConnection` (HTTP), `org.json` (JSON), `LocationManager` +
`Geocoder` (location), `java.time` (dates), `android.util.Log` via `AppLog`
(logging), kotlinx-coroutines (transitive from Compose).

## 5b. aodm crates (tools/aodm, `Cargo.lock` committed)

| Crate | Version | Scope | Purpose |
| --- | --- | --- | --- |
| clap | 4.6.7 (derive, default features off) | aodm | CLI |
| anyhow | 1.0.104 | aodm | error context |
| thiserror | 2.0.21 | aod-core | typed errors |
| jiff | 0.2.39 | aod-core | log timestamps |
| tempfile | 3 | dev | test fixtures |

Policy (`tools/aodm/deny.toml`, checked by `adx check`): crates.io only, no wildcards, licences MIT / Apache-2.0 / Unicode-3.0, yanked crates denied.

## 6. External services

| Service | Used by | Auth | Notes |
| --- | --- | --- | --- |
| Open-Meteo forecast API | HelloAdx | none | Free, no key; HTTPS |
| Android `Geocoder` (Google Play services backend) | HelloAdx | none | Needs a Play/Google APIs image and network |
| GitHub | framework repo (public, two mirrors), app repos, skills | `gh` / git credentials | Push needs write access to both mirrors |

## 7. What a clean machine needs before `./adx up`

1. The apt packages in §2 (`adx system status` prints the exact command).
2. `sudo usermod -aG kvm "$USER"`, then a full logout/login.
3. Claude Code installed (`claude --version`).
4. Then `./adx up`; `node` installs Node.js if missing; restart Claude Code
   so it loads mobile-mcp.
