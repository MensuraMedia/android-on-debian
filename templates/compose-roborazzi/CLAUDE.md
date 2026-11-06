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

## UI workflow

1. Edit composables under `app/src/main/kotlin/`.
2. Every screen and important state gets a `@Test` in
   `app/src/test/kotlin/.../*ScreenshotTest.kt` calling `captureRoboImage(...)`.
3. Run `./gradlew recordRoborazziDebug`, then read the PNGs in
   `app/build/outputs/roborazzi/` to check the result. Iterate.
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
