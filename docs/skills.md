# Skills: UI/UX, advanced features, code control

Third-party Claude Code skills are declared in `config/skills.manifest`.
Each one is pinned to a full commit SHA and vendored into every scaffolded
app's `.claude/skills/<name>/`, along with its MIT `LICENSE`.

`adx skills apply <dir>` installs or refreshes them and writes
`.claude/skills/SKILLS.lock` to record exactly what was installed.

## Pinned set (2026-11-05)

| Skill | Repo @ commit | Use it for |
| --- | --- | --- |
| `hallmark` | Nutlope/hallmark @ `407fa689f4a2` | Designing or auditing screens. Its build, audit, redesign and study modes target generic "AI slop" UI. It was written for web UI; translate its rules (type scale, spacing, colour restraint, hierarchy) into Compose and Material 3 tokens. This is also the project's "AntiSlop" design skill. |
| `swiftui-pro` | twostraws/SwiftUI-Agent-Skill @ `f2920b639952` | SwiftUI only. Keep it for cross-platform design parity and a future iOS template; don't apply it to Kotlin or Compose code. |
| `autoresearch` | uditgoenka/autoresearch @ `7bddd056dfd0` | Advanced features with a measurable goal (performance, size, test pass rate). It iterates modify → verify → keep or discard. |
| `ponytail`, `ponytail-audit`, `ponytail-debt`, `ponytail-gain`, `ponytail-help`, `ponytail-review` | dietrichgebert/ponytail @ `96608e078b2d` | Code control: the simplest solution that works, audits, tech-debt tracking, reviews. |

## Why skills only, without the plugins' hooks

Both autoresearch and ponytail are also published as Claude Code plugins
with automatic hooks:

- **autoresearch:** runs Node scripts before every Read, Write, Edit, Glob,
  Grep and Bash call, on every prompt, and at session start and end.
- **ponytail:** adds activation and mode hooks, plus a per-turn CSV
  "ledger" in `~/.config/ponytail/band/`. It has an optional webhook, which
  is off by default.

The decision taken on 2026-11-05 was to install **only the SKILL.md skills**.
No hooks are installed, so nothing runs on its own; Claude may still load a skill automatically when its description matches the task. Their bundled scripts can still be called by the
skill through Bash, and those calls go through Claude Code's normal
permission prompts.

To adopt a full plugin later, run
`claude plugin marketplace add <owner/repo>` and then
`claude plugin install <name>`, and review the plugin's `hooks/` first.

## Upgrading or adding a skill

1. Find the new commit:
   `gh api repos/<owner>/<repo>/commits/HEAD --jq .sha`
2. Edit its line in `config/skills.manifest`, or append a new one with the
   format `name | owner/repo | sha | path/to/skill-dir`.
3. Re-vendor into each app: `adx skills apply <app-dir>`, then review the
   diff in the app's git.
