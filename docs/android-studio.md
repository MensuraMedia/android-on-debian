# Android Studio (dormant)

Android Studio 2026.2.2.9 ("Rabbit 2 Patch 1") is installed locally but is
**not part of the default workflow**. The framework doesn't depend on it:
the JDK, SDK and emulator are all provisioned by `adx`. It stays available
for the jobs it does best, such as the layout inspector, the profiler and
interactive debugging.

| Item | Value |
| --- | --- |
| Location | `~/.local/opt/android-studio` (user-owned, so in-IDE patch updates need no root) |
| Source | Official tarball from developer.android.com/studio, SHA-256 `11ecbf0e…d8d8ea`, verified 2026-11-05 |
| Launcher | none on PATH and no menu entry (deliberately); `adx studio launch` |
| SDK | point it at `~/Android/Sdk`, the one shared with `adx` |

## When you need it

1. Run `adx studio launch`.
2. In the setup wizard choose **Custom**, set the SDK location to
   `~/Android/Sdk`, and skip any component downloads that are already there.
3. Optional: use Tools → Create Desktop Entry for a menu entry.
4. Optional: install the Claude Code JetBrains plugin, which shows diffs in
   the IDE.

The full install guide, with system checks and best practices, is in the
Claude doc *Installing Android Studio on Linux Mint (Debian-based)*.
