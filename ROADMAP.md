# Roadmap

What android-on-debian (AOD, `adx`) does today, what is being built, and what
comes later. Released detail is in [CHANGELOG.md](CHANGELOG.md); current state
and open decisions in [HANDOFF.md](HANDOFF.md); the full design of the phone
image manager in the approved plan (summarised in §2). Updated 2026-11-06.

Status keys: **Shipped** (released and tagged) · **Done, unreleased** (on a
branch, not yet tagged) · **In progress** · **Planned** (approved, scheduled
in a phase) · **Future** (agreed direction, not scheduled) · **Decision
needed** (waiting for the owner).

## 1. Existing features (shipped)

### v0.1.0 (2026-11-05): framework foundation
| Area | Features |
| --- | --- |
| Core | `adx list`, `doctor`, `up [profile]`, `<module> <action>`; module contract; config in `defaults.env` + git-ignored `local.env`; `@@VAR@@` templating; managed `~/.bashrc` block |
| Toolchain modules | `jdk` (Temurin 21, SHA-256), `shell-env`, `sdk` (cmdline-tools, platform 37, build-tools 37), `emulator` (AVD `adx_api37`, headless start/stop, screenshot, accel), `gradle` (wrapper bootstrap), `studio` (dormant) |
| Claude Code | `mobile-mcp` 1.0.16 (user scope), `claude-permissions` allowlist template, pinned `skills` (hallmark, swiftui-pro, autoresearch, ponytail set) |
| Apps | `compose-roborazzi` template (AGP 9.5.1, Kotlin 2.4.21, Compose BOM, Roborazzi, Robolectric); `adx app-template new` |
| Profiles | `default`, `render-only` |
| Docs / CI | README, architecture, skills, Android Studio, troubleshooting, emulator research; ShellCheck CI |

### v0.2.0 (2026-11-06): logging, preflight, hardening, Claude rules
| Area | Features |
| --- | --- |
| Logging | Framework log `~/.local/state/adx/adx.log` (START/END, exit codes, every message, rotation); emulator logs keep previous runs; `adx logs`, `adx env` |
| New modules | `system` (preflight: packages, CPU virtualisation, KVM, RAM, disk; prints the sudo command), `node` (user-level Node.js, SHA-256), `claude-rules` (global rules + `adversarial-reviewer` agent) |
| Emulator | `autoconfig` for hybrid GPUs (software/Lavapipe + NVIDIA offload), `desktop` menu entry, `--notify`, pinned console port, adb reattach, kill on failed start, status hints |
| Supply chain | cmdline-tools and app Gradle wrappers checksum-pinned; corrupt cached downloads deleted; partial skills/Node installs repaired |
| Reliability | `adx up` re-checks modules after install; `adx doctor [profile]` exit code ignores optional modules; `~/.local/bin` on PATH for modules |
| App template | `CHANGELOG.md`, `HANDOFF.md`, `AppLog` (debug-only info), committed screenshot baselines, design-and-review rules |
| Docs | `environment.md`, `dependencies.md`, `install-issues.md` (38 issues), modularity + ponytail principles |
| Process | Three adversarial reviews (installer, app/template, docs/Claude), fixes applied |

### Example app: HelloAdx (separate repo, local)
Weather app with live Open-Meteo data and device location, Canvas night-lake
scene, ViewModel, accessibility labels, debug-only logging, 11 tests with
committed screenshot baselines. Own HANDOFF lists its open items.

## 2. Pending features

### Done, unreleased (branch `feat/aodm`, will ship in v0.3.0)
- `diagram-design` skill pinned; project diagram profile **`aod-flat-dark`**
  (flat dark, system fonts, WCAG-checked) installed by the skills module and
  selected by `.diagram-design` (framework + app template).
- `docs/diagrams/aodm-architecture` (first diagram), `docs/claude-integration.md`
  (every skill, MCP server, agent, rule, permission), `docs/research/aodm-spike.md`
  (what the emulator really allows), this roadmap.
- Fixes: `adx emulator stop` stopped every emulator; `screenshot` failed with
  two devices.

### In progress: phone image manager `aodm` (Rust), v0.3.0
Phases from the approved plan; each ends with `adx check`, reviewer agents,
CHANGELOG/HANDOFF updates and a commit.

| Phase | Scope | Status |
| --- | --- | --- |
| P0 | Spike on the real emulator (settings, flags, identity fields, copies, locks, calls/SMS) | **Done** |
| P1 | Pipeline foundation: `adx check` gate (shell, Rust, docs, process, security), `devtools` module (ShellCheck, cargo-deny, pre-commit hook), `deploy-reviewer` agent, `review-change` workflow, CI hardening; `rust` module; Cargo workspace; `env`, `log`, `ini` + tests | **Done**: reviewed (adversarial: 21 findings, deployment: 9 findings), all fixed and regression-tested |
| P2 | Inventory: AVD scan, `/proc` process detection, registry, version card; `aodm list/card/env`; `devices` module (build + install) | **Done**: reviewed (code: 13, deployment: 10 findings), all fixed and reproductions replayed |
| P3 | Lifecycle: port allocation (16 slots), start/stop/restart, RAM/CPU budget, adb + emulator console clients, health | Planned |
| P4 | Full-copy saves (create, verify, restore, open as new, prune, pin), clone, rename, delete | Planned |
| P5 | Settings profiles (hardware / launch / runtime, dry-run diff, per-image report), groups, selectors; identity policies (`private`, `standard`, `fixed`, uniqueness check, masked audit); security locks | Planned |
| P6 | Build templates (built-in `barebones` default, `pixel8-api37`, `small-phone`, `tablet`; capture, validate, drift, batch create, provisioning); bulk app deploy, shell broadcast, screenshots, logcat | Planned |
| P7 | Spin-offs: standalone phones (linked / portable), `aod-phone run/control` mechanics panel, switchboard calls/SMS between phones, `adopt` | Planned |
| P8 | Terminal UI: dashboard, details, version card, templates, identity, locks, results; confirmation modals | Planned |
| P9 | `adx release`, `adx upgrade` + migrations + rollback, `adx new-module`; technical document `docs/phone-image-manager.md`; diagrams (saves flow, identity classes, spin-off/switchboard, change pipeline, template lifecycle); end-to-end check; **release v0.3.0** | Planned |

The ten multi-device features delivered across P2–P8: inventory dashboard;
create from templates in batches; lifecycle for one/group/all; full-copy saves;
clone/rename/delete; groups and tags; bulk settings profiles; bulk app deploy;
broadcast and capture; health and version card.

### Decision needed (owner)
| Item | Detail |
| --- | --- |
| Permission allowlist | `Bash(adx:*)` auto-allows actions that rewrite global rules/permissions; `mobile_set_location` auto-allowed; `adb emu geo fix` not allowed; merge can't tighten rules; `adb shell` chaining past ask rules |
| LinearGenetics mirror | `MensuraMedia` has read-only access to `LinearGenetics/android-on-debian`; grant write so releases push to both |
| HelloAdx remote | Create `MensuraMedia/HelloAdx` (blocked by auto mode) |
| swiftui-pro in Android apps | Keep vendored, or only in a future iOS template |

## 3. Future features (not scheduled)

| Feature | Notes |
| --- | --- |
| Desktop GUI `aod-gui` (P10) | eframe/egui on `aod-core`; pure Rust (no GTK `-dev` packages, no sudo) |
| Export/import of saves between machines | Archive format for full-copy saves; system image revision check |
| `android` CLI migration | Replace deprecated `sdkmanager`/`avdmanager` in `sdk`, `emulator` and `aodm` |
| Compose Preview scanner | Render every `@Preview` without hand-written tests |
| Physical devices | `physical-device` module (udev rules, scrcpy) |
| Device matrix in the cloud | `ftl` module (Firebase Test Lab via gcloud) |
| CI profile + Actions template for apps | `verifyRoborazziDebug` on every PR |
| Non-interactive shell environment | Env block in `~/.profile` or before the `.bashrc` interactive guard |
| Skills housekeeping | `skills apply` removes skills dropped from the manifest |
| App template hardening | R8/minify for release, adaptive icon, reject Kotlin keywords in package names, fail on unrendered `@@TOKENS@@` |
| Identity fields the emulator fixes today | IMEI, serial, ICCID/IMSI, MAC: revisit if a supported emulator mechanism appears (never faked) |
| Upstream report | Hybrid-GPU first-boot tcmalloc crash (emulator 37.2.13) |
