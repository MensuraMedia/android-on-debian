# @@APP_NAME@@

Jetpack Compose app (`@@APP_PACKAGE@@`), scaffolded by android-on-debian (`adx`).
UI is built CLI-first: render screens with Roborazzi, check behaviour on the
emulator through mobile-mcp only when needed.

## Commands

| Task | Command |
| --- | --- |
| Render all screenshot tests to PNG | `./gradlew recordRoborazziDebug` |
| Fail on visual diffs vs recorded PNGs | `./gradlew verifyRoborazziDebug` |
| Unit tests | `./gradlew testDebugUnitTest` |
| Build debug APK | `./gradlew assembleDebug` |
| Install on running emulator/device | `./gradlew installDebug` |
| Launch the app | `adb shell am start -n @@APP_PACKAGE@@/.MainActivity` |
| Start / stop headless emulator | `adx emulator start` / `adx emulator stop` |
| Crash logs | `adb logcat -d *:E` |
| Several emulators running | `export ANDROID_SERIAL=emulator-5554` first (serials: `aodm health`), else plain `adb` fails |

## Pipeline: check, test and deploy

The environment builds, checks, tests and deploys the app for you. After every
change, run it and read the result:

```sh
adx pipeline run @@APP_NAME@@      # build, lint, unit + screenshot tests, install on the phone, smoke-test the launch
adx pipeline last @@APP_NAME@@     # the latest result again (add --json for run.json)
```

- Stages: `build` (assembleDebug), `check` (lintDebug; errors fail it),
  `test` (verifyRoborazziDebug, or recordRoborazziDebug while there are no
  references), `deploy` (install on the running phone; several running: pass
  `--serial emulator-NNNN`; none: it starts one), `smoke` (launch, still
  running after a few seconds, no crash or ANR in logcat).
- On failure the summary names the stage, the compiler or lint errors, the
  failing tests, or the crash stack. The run folder (printed as `folder`)
  holds `<stage>.log`, `lint.xml`, `diff-*.png` (screenshot changes: look at
  them; if the change is intended, re-record with
  `./gradlew recordRoborazziDebug`), `screen.png` (the phone after launch)
  and `logcat.txt`.
- Only some stages: `--stages build,test`. The owner sees the same runs on
  the console's Apps page.

## UI workflow

1. Edit composables under `app/src/main/kotlin/`.
2. Every screen and important state gets a `@Test` in
   `app/src/test/kotlin/.../*ScreenshotTest.kt` calling `captureRoboImage(...)`.
3. Run `./gradlew recordRoborazziDebug`, then read the PNGs in
   `app/src/test/screenshots/` (committed baselines) to check the result. Iterate.
4. For taps, navigation and runtime behaviour: `adx emulator start`,
   `./gradlew installDebug`, launch, then use the mobile-mcp tools
   (screenshot, tap, type, list elements).

## Conventions

- Kotlin sources in `src/main/kotlin`; versions only in `gradle/libs.versions.toml`.
- Composables take state as parameters (e.g. `initialCount`) so tests can
  render any state without the emulator.
- Do not commit `local.properties`, keystores or signing passwords.

## Logging

All logging goes through `AppLog` (`app/src/main/kotlin/.../AppLog.kt`), tagged
`@@APP_NAME@@/<area>`. Log every network call (URL, status, size, duration),
permission/config decisions, fallbacks, state changes and caught errors; use
`AppLog.timed` for slow work. Never log secrets. Read logs with
`adb logcat -d | grep '@@APP_NAME@@/'`, and read them first when debugging.

## Changelog and handoff (mandatory)

- `CHANGELOG.md` (Keep a Changelog): add an `[Unreleased]` entry in the same
  piece of work as every change.
- `HANDOFF.md`: keep State, How to run/test, Known issues, Next steps and Open
  questions current; update before ending any session that changed something.

## Design and review

- Keep the code modular (one screen or concern per file/package, state passed
  in as parameters) and minimal: use the ponytail skill (`/ponytail`) for the
  smallest complete change; `/ponytail-audit` to find bloat.
- **Diagrams:** use the `diagram-design` skill. Every diagram is **flat dark**
  (profile `aod-flat-dark`, selected by the `.diagram-design` marker): solid
  fills, hairlines, one mint accent, no gradients/shadows/glow, system fonts.
  Source HTML in `docs/diagrams/`, exported SVG next to it, embedded in docs.
- Before calling a change done, run the `adversarial-reviewer` agent on what
  you touched, and record what it finds in `HANDOFF.md` if you don't fix it.
