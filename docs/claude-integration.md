# Claude Code integration: skills, MCP servers, agents, rules

Everything this environment adds to Claude Code, where it comes from, where it
is installed, what it is for, and how to update it. Verified 2026-11-06 with
`claude mcp list`, `adx skills status`, `adx claude-rules status`.

## 1. MCP servers

| Server | Source / version | Scope | Installed by | Used for |
| --- | --- | --- | --- | --- |
| **mobile-mcp** | `npx -y @mobilenext/mobile-mcp@1.0.16` (pinned, `ADX_MOBILE_MCP_VERSION`) | user (`~/.claude.json`, all projects) | `mobile-mcp` module | Drive emulators/devices: list devices and elements, tap, type, swipe, screenshot, launch/terminate apps, logs |

Rules for mobile-mcp (also in the global rules):
- Start the emulator with `adx emulator start` first; mobile-mcp doesn't start one.
- Read screens with `mobile_list_elements_on_screen`, tap by element ref.
- Don't use `mobile_set_location`: it injects an agent into the app instead of
  moving the GPS (see install-issues #17). Use `adb emu geo fix <lon> <lat>`.
- After changing its registration: `/mcp` → reconnect, or restart Claude Code.
- Check: `adx mobile-mcp status` (also verifies the pinned version).

Account connectors that appear in `claude mcp list` but are **not part of AOD**
and not used by it: claude.ai Claude Docs, Google Drive, Google Calendar,
Gmail. Some sessions also offer a claude-artifacts server. Nothing in the
framework depends on them.

## 2. Skills

Pinned to exact commits in `config/skills.manifest`; `adx skills install`
fetches them into `~/Downloads/adx/skills-src/` (a cache), `adx skills apply <dir>`
copies them into `<dir>/.claude/skills/` and writes `SKILLS.lock`. Only the skill
folders are installed, never the upstream plugins' hooks. Claude may load a skill
automatically when its description matches the task, or you invoke it as `/<name>`.

| Skill | Source @ commit | Purpose here | Installed into |
| --- | --- | --- | --- |
| `ponytail` (+ `ponytail-audit`, `-debt`, `-gain`, `-help`, `-review`) | dietrichgebert/ponytail @ 96608e0 | Code discipline: smallest complete change, audits, `shortcut:` debt ledger, reviews. Project rule: modular, then minimal | apps (template), framework (`adx skills apply .`) |
| `hallmark` | Nutlope/hallmark @ 407fa68 | UI/UX quality gates (anti-"AI slop"); web-oriented, principles applied to Compose | apps |
| `diagram-design` | cathrynlavery/diagram-design @ f24d98b | Documentation diagrams (HTML + exported SVG, accessible). Project style **flat dark** via profile `aod-flat-dark` | apps, framework |
| `autoresearch` | uditgoenka/autoresearch @ 7bddd05 | Autonomous modify → verify → keep/discard loops toward a metric | apps |
| `swiftui-pro` | twostraws/SwiftUI-Agent-Skill @ f2920b6 | SwiftUI best practices; reference only (iOS), not for Compose | apps (open question: drop from Android apps?) |

Skill profiles kept in the repo:

| Profile | File | Installed to | Selected by |
| --- | --- | --- | --- |
| `aod-flat-dark` (diagram-design) | `config/skill-profiles/diagram-design/aod-flat-dark.md` | `~/.diagram-design/profiles/` (by `adx skills install`) | `.diagram-design` marker (`profile: aod-flat-dark`) in the framework root and every new app |

Update a skill: change its commit in the manifest, `adx skills install`, then
`adx skills apply <dir>` for each project; read the new `SKILL.md` before
pinning (third-party text becomes instructions to Claude).

## 3. Agents

| Agent | File (source → installed) | Purpose | Tools |
| --- | --- | --- | --- |
| `adversarial-reviewer` | `templates/claude/agents/adversarial-reviewer.md` → `~/.claude/agents/` | Read-only scrutiny of a segment (installation, build, configuration, design, security, docs); ranked findings with evidence and minimal fixes | Read, Grep, Glob, Bash (read-only by instruction) |
| `deploy-reviewer` *(planned, P1)* | same pattern | Scrutinises how a change lands: upgrade, migration, rollback, clean install, CI parity, blast radius | read-only |

Installed and checked by the `claude-rules` module (`adx claude-rules install|status`);
an edited copy in `~/.claude/agents/` is kept as `.bak` before replacement.

## 4. Global rules

`config/claude-global-rules.md` is installed as a marked block
(`<!-- >>> adx:claude-rules >>> -->`) in `~/.claude/CLAUDE.md`, with machine paths
filled in. It applies to every project on the machine:
- every project keeps `CHANGELOG.md` and `HANDOFF.md` current;
- structured, filterable logging with a documented read command, no secrets;
- Android notes: use `adx` for the emulator, read `adx logs` first, `adb emu geo fix`.

Per-project rules live in each `CLAUDE.md` (framework and app template): module
contract, ponytail, reviewer before "done", flat dark diagrams, logging.

## 5. Permissions

`templates/claude/settings.json` is merged into each project's
`.claude/settings.json` by `adx claude-permissions apply <dir>`: 44 allow rules
(gradle, adx, adb read/install/logcat, read-only mobile-mcp and input tools) and
12 ask rules (on-device `adb shell rm`, `adb push`, `adb root`, uninstall, clear
data, clipboard, remote/cloud devices).

**Open security decision (owner):** `Bash(adx:*)` auto-allows every adx action,
including ones that rewrite global rules and permissions; `mobile_set_location`
is auto-allowed; `adb emu geo fix` is not; the merge can't tighten rules. See
HANDOFF open questions. Not changed without the owner's approval.

## 6. Workflows

| Workflow | Status | Purpose |
| --- | --- | --- |
| `review-change` | planned (P1) | Map changed paths to segments, run reviewer agents in parallel, verify findings, gate releases |

## 7. Check everything

```bash
claude mcp list                 # MCP servers and health
adx mobile-mcp status           # pinned version registered
adx skills status               # skills cached + profiles installed
adx claude-rules status         # global rules block + agents up to date
ls <project>/.claude/skills     # vendored skills; SKILLS.lock lists commits
```
