# Handoff — 2026-11-05

The state of the project at the end of the first session, written for
whoever (human or agent) picks it up next.

## State

| Area | Status |
| --- | --- |
| Framework (`adx`, 10 modules, 2 profiles) | Done; `./adx list` shows all OK on this machine |
| Render loop (Roborazzi) | Verified end to end with `~/projects/android-apps/HelloAdx` |
| Live loop (emulator + adb) | Verified: headless boot, install, launch, screenshot |
| mobile-mcp | Registered (user scope), stdio handshake verified; **not yet used from a live Claude session** (needs a Claude Code restart) |
| Skills | 9 skills pinned and vendored into HelloAdx; plugin hooks deliberately excluded |
| Android Studio | Installed at `~/.local/opt/android-studio`, dormant by request |
| Repo | MIT licensed, pushed to `MensuraMedia/android-on-debian` (private) |

## Machine-specific facts

- Linux Mint 22.3 (Ubuntu 24.04 base), Cinnamon on X11, kernel 7.0.0-38,
  i7-13620H, Intel UHD graphics only, 31 GiB RAM.
- `/dev/kvm` is reachable through a logind ACL, but the user is **not in the
  `kvm` group**. Run `sudo usermod -aG kvm $USER` for a permanent fix.
- sudo needs a password, so all installs avoid root.
- The project folder was renamed from `android-studio` to
  `android-on-debian`. A compatibility symlink
  `~/projects/android-studio → android-on-debian` was left for the running
  session; delete it once Claude Code has been restarted from the new path.

## Next steps

1. Restart Claude Code in an app folder and do a live mobile-mcp session:
   take a screenshot, list elements, tap "Press me", and check the counter
   reads 1.
2. Optional sudo items, all documented in the Studio install doc:
   - `kvm` group membership.
   - The 32-bit libraries (`libncurses6:i386`, `lib32z1`), only needed by
     Android Studio.
   - `fs.inotify.max_user_watches=524288`.
3. Migrate the `sdk` and `emulator` modules from the deprecated
   `sdkmanager`/`avdmanager` to the new `android` CLI (`android sdk`,
   `android emulator`) once its syntax has been checked.
4. Add a Compose Preview scanner (ComposablePreviewScanner +
   `roborazzi-compose-preview-scanner-support`) so every `@Preview` renders
   without a hand-written test.
5. Add an optional `physical-device` module (udev rules, scrcpy 5.x from
   GitHub) and an `ftl` module (Firebase Test Lab via `gcloud`).
6. Consider a `ci` profile plus a GitHub Actions workflow template for apps
   (`render-only` with `verifyRoborazziDebug`).

## Open questions

- Should `ADX_APPS_DIR` (`~/projects/android-apps`) live inside this repo
  instead? It's kept separate for now, so each app has its own git history.
- The SwiftUI skill is iOS-only. Keep it vendored into Android apps, or move
  it to a future iOS template?
