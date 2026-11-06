# Global rules (all projects)

Installed by android-on-debian (`adx claude-rules install`) from
`config/claude-global-rules.md`. Edit that file, not this block.

These apply to every project. A project's own CLAUDE.md may add to them but
should not drop them.

## Changelog and handoff: always kept current

Every project has a `CHANGELOG.md` and a `HANDOFF.md` at its root. Create them
if missing, the first time you change anything in a project.

- **CHANGELOG.md**: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
  format with an `## [Unreleased]` section (Added / Changed / Fixed / Removed).
  Add an entry in the same piece of work as every user-visible change, fix,
  config change or dependency change. Do it right away, not "at the end".
- **HANDOFF.md**: the state of the project for whoever picks it up next (human
  or agent). Keep these sections current: date of the last update, **State**
  (what works and what's verified), **How to run/test**, **Known issues**,
  **Next steps**, **Open questions**. Update it before ending any session that
  changed code, config or environment, and whenever a task finishes or a
  decision is made. Replace stale text rather than appending forever.
- Before reporting work as done, check both files reflect it.

## Advanced logging: built in from the start

Every project gets structured, filterable logging, not ad-hoc prints:

- One logging entry point per project (a small wrapper or the language's
  standard logger), with a consistent tag/prefix per app and per area
  (e.g. `App/Net`, `App/DB`, `App/UI`), and standard levels (debug, info,
  warn, error).
- Log at the boundaries: every network call (method, URL, status, size,
  duration), every external process, file or device I/O, permission and
  config decisions, fallbacks taken, state transitions, and every caught
  error with its exception. Time slow operations.
- Never log secrets, tokens, passwords or personal data beyond what is needed
  for debugging.
- Scripts and CLIs: log to stderr and to a log file under a predictable path
  (documented in the README/HANDOFF), with timestamps.
- Document how to read the logs (the exact filter command) in the project's
  CLAUDE.md or README.
- When debugging, read the logs first and add logging where it is missing.

## Android projects (android-on-debian / adx)

- Framework: `@@ADX_ROOT@@` (`adx`); apps live in `@@ADX_APPS_DIR@@`. Environment reference: its
  `docs/environment.md`.
- Before debugging the emulator, adb or mobile-mcp, run `adx emulator status`
  and read `adx logs` / `adx logs emulator` (logs in `@@ADX_LOG_DIR@@`).
- Start the emulator with `adx emulator start [--window]`, never a bare
  `emulator` command, so config, logging and crash detection apply.
- Move the emulator's GPS with `adb emu geo fix <lon> <lat>`; mobile-mcp's
  `mobile_set_location` does not move it.
